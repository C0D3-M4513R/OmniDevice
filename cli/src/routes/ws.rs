use serde_with::serde_as;

#[rocket::get("/ws")]
pub async fn ws_impl(device_list: &rocket::State<crate::DeviceList>, ws: rocket_ws::WebSocket) -> rocket_ws::Channel<'static> {
    #[derive(Clone, serde_derive::Serialize, serde_derive::Deserialize)]
    #[serde(from="String", into="String")]
    struct DeviceConfig{
        uuid: Vec<String>,
        sampling_rate: Option<u32>,
        format: Option<String>,
    }
    impl From<String> for DeviceConfig {
        fn from(value: String) -> Self {
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
                        config.uuid.push(last_2nd.to_string());
                    } else {
                        if let Ok(v) = last_2nd.parse::<u32>() {
                            config.sampling_rate = Some(v);
                            config.format = Some(last.to_string());
                        } else {
                            config.uuid.push(last.to_string());
                            config.uuid.push(last_2nd.to_string());
                        }
                    }
                },
                (Some(last), None) => {
                    config.uuid.push(last.to_string());
                },
                (None, Some(last_2nd)) => {
                    panic!("didn't get last element, but got second last?");
                },
                (None, None) => {}
            }
            config.uuid.extend(vec.into_iter().map(|v|v.to_string()));
            config
        }
    }
    impl From<DeviceConfig> for String {
        fn from(value: DeviceConfig) -> Self {
            let mut out = value.uuid.join(" ");
            if let Some(sampling_rate) = value.sampling_rate {
                out.push_str(&format!(" {sampling_rate}"));
            }
            if let Some(format) = value.format {
                out.push(' ');
                out.push_str(&format);
            }
            out
        }
    }
    #[serde_as]
    #[derive(serde_derive::Serialize, serde_derive::Deserialize)]
    #[serde(untagged)]
    enum Messages{
        DeviceConfig(#[serde_as(as = "serde_with::json::JsonString")] DeviceConfig),
        Hello(String),
    }
    use rocket::futures::{SinkExt, StreamExt};
    ws.channel(move |mut stream|Box::pin(async move {
        loop{
            tokio::select! {
                Some(message) = stream.next() => {
                    let message = match message {
                        Err(err) => {
                            eprintln!("error whilst receiving websocket message: {}", err);
                            return Err(err)
                        }
                        Ok(v) => v,
                    };
                    match message {
                        rocket_ws::Message::Text(text) => {
                            let config = DeviceConfig::from(text.clone());
                            let mut set:std::collections::HashSet<_> = config.uuid.iter().collect();
                            for device in device_list.list_send() {
                                let id = match device.id().await {
                                    Some(id) => id,
                                    None => continue,
                                };
                                set.remove(id.serial());
                            }

                            if set.is_empty() {
                                for device in device_list.list() {
                                    let id = match device.id().await {
                                        Some(id) => id,
                                        None => continue,
                                    };
                                    match device.start_capture().await {
                                        Ok(_) => {
                                            println!("Started capture for device: {}", id.serial());
                                        }
                                        Err(err) => {
                                            eprintln!("Error starting capture for device {}: {}", id.serial(), err);
                                        }
                                    }
                                }
                            }
                            println!("Received message: {text}");
                        },
                        rocket_ws::Message::Pong(pong) => {
                            println!("Received pong: {pong:?}");
                        },
                        rocket_ws::Message::Ping(ping) => {
                            stream.send(rocket_ws::Message::Pong(ping)).await
                                .map_err(|err| {
                                    eprintln!("error sending pong: {err}");
                                    err
                                })?;
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
    }))
}