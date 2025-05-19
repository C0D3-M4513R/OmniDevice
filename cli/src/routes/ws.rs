use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;
use serde_with::serde_as;
use tokio::sync::RwLock;
use crate::device::messages::MeasureData;
use crate::device::SendDevice;
use crate::MAX_MESSAGE_BUF;

#[derive(Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
struct WSMeasurement {
    devices: Vec<String>,
    data: Vec<WSMeasurementData>,
}

#[derive(Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
struct WSMeasurementData{
    timestamp: f64,
    value: Vec<u16>,
}

#[rocket::get("/ws")]
pub async fn ws_impl(shutdown: rocket::Shutdown, device_list: &rocket::State<Arc<RwLock<crate::DeviceList>>>, ws: rocket_ws::WebSocket) -> rocket_ws::Channel<'static> {
    #[derive(Clone, serde_derive::Serialize, serde_derive::Deserialize)]
    struct DownsampleRequest{
        command: String,
        tmin: chrono::DateTime<chrono::FixedOffset>,
        tmax: chrono::DateTime<chrono::FixedOffset>,
        desired_number_of_samples: u128
    }
    #[derive(Clone)]
    struct DeviceConfig<'a>{
        uuid: Vec<&'a str>,
        sampling_rate: Option<u32>,
        format: Option<&'a str>,
    }
    impl<'a> From<&'a str> for DeviceConfig<'a> {
        fn from(value: &'a str) -> Self {
            let mut config = DeviceConfig{
                uuid: Vec::new(),
                sampling_rate: None,
                format: None,
            };
            let mut vec: Vec<_> = value.rsplit(" ").collect();
            let last = vec.pop();
            let last_2nd = vec.pop();
            match (last, last_2nd) {
                (Some(last), Some(last_2nd)) => {
                    if let Ok(v) = last.parse::<u32>() {
                        config.sampling_rate = Some(v);
                        config.uuid.push(last_2nd);
                    } else {
                        if let Ok(v) = last_2nd.parse::<u32>() {
                            config.sampling_rate = Some(v);
                            config.format = Some(last);
                        } else {
                            config.uuid.push(last);
                            config.uuid.push(last_2nd);
                        }
                    }
                },
                (Some(last), None) => {
                    config.uuid.push(last);
                },
                (None, Some(_)) => {
                    panic!("didn't get last element, but got second last?");
                },
                (None, None) => {}
            }
            config.uuid.append(&mut vec);
            config
        }
    }
    impl From<DeviceConfig<'_>> for String {
        fn from(value: DeviceConfig) -> Self {
            let mut out = value.uuid.join(" ");
            if let Some(sampling_rate) = value.sampling_rate {
                out.push(' ');
                out.push_str(&sampling_rate.to_string());
            }
            if let Some(format) = value.format {
                out.push(' ');
                out.push_str(format);
            }
            out
        }
    }
    struct Measure<T>{
        uuids: Vec<String>,
        rx: T,
    }
    #[serde_as]
    #[derive(serde_derive::Serialize, serde_derive::Deserialize)]
    #[serde(untagged)]
    enum Messages{
        DownsampleRequest(DownsampleRequest),
    }
    use rocket::futures::{SinkExt, StreamExt};
    let device_list = device_list.inner().clone();
    ws.channel(move |mut stream|Box::pin(async move {
        let mut timer:Option<tokio::time::Interval> = None;
        let mut rx:Option<Measure<tokio::sync::mpsc::Receiver<(usize, MeasureData)>>> = None;
        let mut js = tokio::task::JoinSet::new();
        let mut result = None;
        let mut shutdown = shutdown;
        let mut measure_data = Vec::with_capacity(MAX_MESSAGE_BUF as usize);
        macro_rules! merge_err {
            ($err:expr, $reason:expr) => {
                let err = $err;
                match result {
                    Some(Err(err_old)) => {
                        result = Some(Err(rocket_ws::result::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, anyhow::format_err!("{err_old:?}\n{}: {err:?}", $reason)))));
                    },
                    None | Some(Ok(())) => {
                        result = Some(Err(rocket_ws::result::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, err))));
                    },
                }
            };
        }
        macro_rules! error {
            ($expr:expr, $err:ident, $reason:expr) => {
                match $expr {
                    Ok(v) => v,
                    Err($err) => {
                        let reason = $reason;
                        let err = $err;
                        merge_err!(err, &reason);
                        #[allow(unused_assignments)]
                        {
                            rx = None;
                        }
                        if let Err(err) = stream.close(Some(rocket_ws::frame::CloseFrame {
                            code: rocket_ws::frame::CloseCode::Invalid,
                            reason: (&reason).into(),
                        })).await {
                            eprintln!("Failed to close stream, because of '{reason}': {err}");
                            merge_err!(err, "Failed to close stream");
                        }
                        break;
                    }
                }
            };
        }
        loop{
            tokio::select! {
                _ = &mut shutdown => {
                    println!("Shutdown requested");
                    #[allow(unused_assignments)]
                    {
                        rx = None;
                    }
                    if let Err(err) = stream.close(Some(rocket_ws::frame::CloseFrame {
                        code: rocket_ws::frame::CloseCode::Restart,
                        reason: "The server is Shutting Down".into(),
                    })).await {
                        eprintln!("Failed to close stream: {err}");
                        merge_err!(err, "Failed to close stream");
                    }
                    break;
                }
                Some(_) = async { match &mut timer {
                    Some(timer) => Some(timer.tick().await),
                    None => None,
                }}, if timer.is_some() && rx.is_some() => {
                    let rx = match &rx {
                        Some(v) => v,
                        None => continue,
                    };
                    let message = WSMeasurement{
                        devices: rx.uuids.clone(),
                        data: core::mem::replace(&mut measure_data, Vec::with_capacity(MAX_MESSAGE_BUF as usize)),
                    };
                    let string = error!(serde_json::to_string(&message).map_err(|err|rocket_ws::result::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, err))), err, format!("error serializing message {message:?}: {err}"));
                    error!(stream.send(rocket_ws::Message::Text(string.clone())).await, err, format!("error sending message {string}: {err}"));
                },
                Some(message) = async{ match &mut rx {
                    Some(measure) => Some(Measure{
                        uuids: measure.uuids.clone(),
                        rx: (measure.rx.recv().await, &measure.rx)
                    }),
                    None => None
                }}, if rx.is_some() => {
                    let (message, _) = message.rx;
                    let (_, message) = match message{
                        Some(v) => v,
                        None => continue,
                    };

                    if message.counter() % 50 != 0 { continue; }
                    let timestamp = std::time::UNIX_EPOCH.elapsed().unwrap_or(Duration::ZERO) //Get the current time
                        .saturating_mul(1000) //new Data(number) uses milliseconds. Multiply first, then convert to floating-point, to not unnecessarily lose precision
                        .as_secs_f64(); //and then convert to floating-point
                    measure_data.extend(message.data().iter().map(|value|WSMeasurementData{
                            timestamp,
                            value: vec![*value],
                    }));
                },
                Some(message) = stream.next() => {
                    let message = match message {
                        Err(err) => {
                            eprintln!("error whilst receiving websocket message: {}", err);
                            result = Some(Err(err));
                            break
                        }
                        Ok(v) => v,
                    };
                    match message {
                        rocket_ws::Message::Text(text) => {
                            match serde_json::from_str::<Messages>(text.as_str()) {
                                Ok(Messages::DownsampleRequest(rq)) => {
                                    if rq.command != "get_downsampled_in_range" {
                                        error!(Err(rocket_ws::result::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, "Unknown command"))), err, format!("Unknown command: {}", rq.command));
                                    }
                                    //Todo: Implement downsampling
                                },
                                Err(_) => {
                                    let config = DeviceConfig::from(text.as_str());
                                    let mut set:std::collections::HashSet<_> = config.uuid.iter().copied().collect();
                                    let device_list = device_list.read().await;
                                    let mut subscribed_devices = Vec::new();
                                    for device in device_list.list() {
                                        let id = match device.id().await {
                                            Some(id) => id,
                                            None => continue,
                                        };
                                        set.remove(id.serial().as_str());
                                        subscribed_devices.push(device);
                                    }

                                    //Todo: Support multiple devices
                                    if subscribed_devices.len() > 1 {
                                        if let Err(err) = stream.close(Some(rocket_ws::frame::CloseFrame {
                                            code: rocket_ws::frame::CloseCode::Invalid,
                                            reason: format!("Subscribed to too many devices. Currently only one device is supported. You tried to subscribe to {} devices", subscribed_devices.len()).into(),
                                        })).await {
                                            eprintln!("error closing websocket: {}", err);
                                            result = Some(Err(err));
                                            break;
                                        }
                                    }

                                    if !set.is_empty() {
                                        if let Err(err) = stream.close(Some(rocket_ws::frame::CloseFrame {
                                            code: rocket_ws::frame::CloseCode::Invalid,
                                            reason: format!("Device(s) not found: {}", set.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ")).into(),
                                        })).await {
                                            eprintln!("error closing websocket: {}", err);
                                            result = Some(Err(err));
                                            break;
                                        }
                                    } else {
                                        let (tx, rx_) = tokio::sync::mpsc::channel(MAX_MESSAGE_BUF as usize*8);
                                        let mut devices = Vec::new();
                                        for (i, device) in subscribed_devices.into_iter().enumerate() {
                                            let id = match device.id().await {
                                                Some(id) => id,
                                                None => {
                                                    result = Some(match stream.close(Some(rocket_ws::frame::CloseFrame {
                                                        code: rocket_ws::frame::CloseCode::Invalid,
                                                        reason: Cow::Borrowed("Device only had an id spuriously?"),
                                                    })).await {
                                                        Ok(_) => Ok(()),
                                                        Err(err) => {
                                                            eprintln!("error closing websocket: {}", err);
                                                            Err(err)
                                                        }
                                                    });
                                                    break;
                                                },
                                            };
                                            println!("Subscribing to device: {}", id.serial());

                                            devices.push(id.serial().clone());
                                            let tx = tx.clone();
                                            {
                                                let device = SendDevice::from(device);
                                                js.spawn(async move {
                                                    let mut rx = device.rx_queue().resubscribe();
                                                    loop {
                                                        match rx.recv().await {
                                                            Ok(message) => {
                                                                if let Err(err) = tx.send((i, message)).await {
                                                                    eprintln!("error sending message: {}", err);
                                                                    break;
                                                                }
                                                            }
                                                            Err(tokio::sync::broadcast::error::RecvError::Lagged(num)) => {
                                                                eprintln!("Lagged {num} messages");
                                                                continue;
                                                            }
                                                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                                                println!("Device disconnected");
                                                                break;
                                                            }
                                                        }
                                                    }
                                                });
                                            }
                                            match tokio::task::block_in_place(||device.start_capture()) {
                                                Ok(_) => {},
                                                Err(err) => {
                                                    eprintln!("error starting capture: {err}");
                                                    let err = rocket_ws::result::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, err));
                                                    error!(Err(err), err, format!("error starting capture: {err}"));
                                                }
                                            }
                                        }
                                        rx = Some(Measure{
                                            uuids: devices,
                                            rx: rx_
                                        });
                                        let mut interval = tokio::time::interval(Duration::from_millis(500));
                                        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                                        timer = Some(interval);
                                    }
                                }
                            }

                            println!("Received message: {text}");
                        },
                        rocket_ws::Message::Pong(pong) => {
                            println!("Received pong: {pong:?}");
                        },
                        rocket_ws::Message::Ping(ping) => {
                            match stream.send(rocket_ws::Message::Pong(ping)).await {
                                Ok(()) => {
                                    println!("Sent pong");
                                },
                                Err(err) => {
                                    eprintln!("error sending pong: {err}");
                                    result = Some(Err(err.into()));
                                    break;
                                }
                            }
                        },
                        rocket_ws::Message::Close(_) => {
                            println!("Websocket connection closed");
                            return Ok(());
                        },
                        _ => {},
                    }

                }
            }
        }
        #[allow(unused_assignments)]
        {
            rx = None; // dropping the receiver should make the tasks in the join-set stop, as soon as they have a new message themselves.
        }
        js.abort_all();
        while let Some(join) = js.join_next().await {
            match join {
                Ok(()) => {},
                Err(err) => {
                    if err.is_cancelled() { continue; }
                    eprintln!("task panicked: {err}");
                    match result {
                        Some(Err(err_old)) => {
                            result = Some(Err(rocket_ws::result::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, anyhow::format_err!("{err_old:?}\nTask panicked: {err:?}")))));
                        },
                        None | Some(Ok(())) => {
                            result = Some(Err(rocket_ws::result::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, err))));
                        },
                    }
                    break;
                }
            }
        }
        result.unwrap_or(Ok(()))
    }))
}