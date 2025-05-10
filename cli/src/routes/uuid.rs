#[derive(serde_derive::Serialize, serde_derive::Deserialize)]
struct Devices{
    devices: Vec<Device>,
    colors: Vec<Data>,
}
#[derive(serde_derive::Serialize, serde_derive::Deserialize)]
struct Device{
    #[serde(rename = "UUID")]
    uuid: String,
}
#[derive(serde_derive::Serialize, serde_derive::Deserialize)]
struct Data {
    color: Color
}
#[derive(serde_derive::Serialize, serde_derive::Deserialize)]
struct Color{
    r: u32,
    g: u32,
    b: u32,
}
#[rocket::get("/UUID")]
pub async fn get_devices(device_list: &rocket::State<crate::DeviceList>) -> Result<String, String> {
    let send_list = device_list.list_send();
    let mut devices = Vec::new();
    let mut colors = Vec::new();
    for device in send_list {
        match device.id().await {
            Some(id) => {
                devices.push(Device{
                    uuid: id.serial().to_string(),
                });
                let rgb = device.rgb().await;
                colors.push(Data {
                    color: Color{
                        r: u32::from(rgb.r()),
                        g: u32::from(rgb.g()),
                        b: u32::from(rgb.b()),
                    }
                })
            },
            None => continue,
        }
    }
    let devices = Devices{
        devices,
        colors,
    };
    serde_json::to_string(&devices)
        .map_err(|v|v.to_string())
}