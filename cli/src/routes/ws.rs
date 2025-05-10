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
            let mut iteration = 0;
            for i in value.rsplit(" ") {
                if iteration <= 1 {
                    if let Ok(v) = i.parse::<u32>() {
                        config.sampling_rate = Some(v);
                    } else {
                        if iteration == 0 {
                            config.format = Some(i.to_string());
                        }
                    }
                } else {
                    config.uuid.push(i.to_string());
                }
                iteration += 1;
            }
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