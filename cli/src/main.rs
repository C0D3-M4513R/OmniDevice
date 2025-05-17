use std::sync::Arc;
use std::time::Duration;
use clap::Parser;
use tokio::sync::RwLock;
use crate::device::DeviceList;
use crate::options::Options;

mod options;
mod webserver;
mod device;
mod routes;

const USB_VID: u16 = 0x2e8a;
const USB_PID: u16 = 0x000a;
const MAX_MESSAGE_SIZE: u16 = 2_u16.pow(12);
const MAX_MESSAGE_BUF: u32 = 2_u32.pow(15);

#[rocket::main]
async fn main() {
    let options = options::Options::parse();
    if options.version() {
        match option_env!("GIT_VERSION") {
            Some(version) => println!("Running Version: {}", version),
            None => println!("Running Version: {}-development", env!("CARGO_PKG_VERSION")),
        }
    }
    let mut device_list = device::DeviceList::new();
    if options.search() {
        match device_list.scan_for_new_devices().await {
            Ok(()) => {
                println!("Found devices:");
                tokio::time::sleep(Duration::from_secs(1)).await;
                for device in device_list.list_send() {
                    let serializable_device = device.serializable_device().await;
                    match serde_json::to_string(&serializable_device) {
                        Ok(json) => println!("{json}"),
                        Err(err) => {
                            let descriptor = device.descriptor();
                            eprintln!("Error serializing device ID to JSON for descriptor {descriptor:?}: {err}", )
                        },
                    }
                }
            }
            Err(err) => eprintln!("Error scanning for devices: {}", err),
        }
    }
    if options.websocket() {
        if options.port() == 0 {
            eprintln!("Port must be greater than 0");
            return;
        }
        if let Err(err) = run_websocket(options.clone(), device_list).await {
            eprintln!("Error starting websocket server: {}", err);
            return;
        }
    }
}

async fn run_websocket(option: Options, device_list: DeviceList) -> Result<rocket::Rocket<rocket::Ignite>, rocket::Error>{
    let rocket = rocket::build();
    let figment = rocket.figment().clone()
                .merge((rocket::Config::PORT, option.port()));
    rocket
        .configure(figment)
        .manage(Arc::new(RwLock::new(device_list)))
        .mount("/", rocket::routes![
            routes::get_devices,
            routes::help,
            routes::ws_impl,
        ])
        .launch()
        .await
}
