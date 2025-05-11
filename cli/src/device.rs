mod messages;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::MissedTickBehavior;
use crate::{MAX_MESSAGE_SIZE, USB_PID, USB_VID};

pub(super) struct DeviceList {
    list: Vec<Device>
}
impl DeviceList {
    pub fn new() -> Self{
        DeviceList{
            list: Vec::new()
        }
    }
    pub async fn scan_for_new_devices(&mut self) -> anyhow::Result<()>{
        let device_list = match rusb::DeviceList::new() {
            Ok(v) => v,
            Err(err) => anyhow::bail!("Failed to get device list: {err}")
        };
        self.list.reserve(device_list.len());
        for device in device_list.iter() {
            let descriptor = match device.device_descriptor() {
                Ok(v) => v,
                Err(err) => {
                    eprintln!("Failed to get device descriptor: {err}");
                    continue;
                }
            };
            if descriptor.vendor_id() != USB_VID || descriptor.product_id() != USB_PID {
                continue;
            }
            match tokio::task::block_in_place(||Device::new(device, descriptor)) {
                Ok(device) => {
                    self.list.push(device);
                }
                Err(err) => {
                    eprintln!("Failed to create device: {err}");
                }
            };
        }

        Ok(())
    }
    pub fn list(&self) -> &Vec<Device> {
        &self.list
    }
    pub fn list_send(&self) -> Vec<SendDevice> {
        self.list.iter().map(SendDevice::from).collect()
    }
}

#[derive(serde_derive::Serialize, serde_derive::Deserialize)]
pub struct SerializableDevice {
    descriptor: String,
    id: Option<messages::Id>,
    meta_data: Option<messages::MetaData>,
    rgb: messages::SetRGB,
}
pub struct SendDevice {
    descriptor: Arc<rusb::DeviceDescriptor>,
    id: Arc<Mutex<Option<messages::Id>>>,
    meta_data: Arc<Mutex<Option<messages::MetaData>>>,
    rgb: Arc<Mutex<messages::SetRGB>>,
}
impl SendDevice {
    pub async fn id(&self) -> Option<messages::Id> {
        self.id.lock().await.clone()
    }
    pub async fn meta_data(&self) -> Option<messages::MetaData> {
        self.meta_data.lock().await.clone()
    }
    pub async fn rgb(&self) -> messages::SetRGB {
        self.rgb.lock().await.clone()
    }
    pub fn descriptor(&self) -> &rusb::DeviceDescriptor {
        &self.descriptor
    }
    pub async fn serializable_device(&self) -> SerializableDevice {
        SerializableDevice{
            descriptor: format!("{:?}", self.descriptor),
            id: self.id().await,
            meta_data: self.meta_data().await,
            rgb: self.rgb().await,
        }
    }
}
impl From<&Device> for SendDevice {
    fn from(device: &Device) -> Self {
        let descriptor = device.descriptor.clone();
        let id = device.id.clone();
        let meta_data = device.meta_data.clone();
        let rgb = device.rgb.clone();
        SendDevice{
            descriptor,
            id,
            meta_data,
            rgb,
        }
    }
}

struct Device{
    device: rusb::Device<rusb::GlobalContext>,
    device_handle: Arc<rusb::DeviceHandle<rusb::GlobalContext>>,
    descriptor: Arc<rusb::DeviceDescriptor>,
    id: Arc<Mutex<Option<messages::Id>>>,
    meta_data: Arc<Mutex<Option<messages::MetaData>>>,
    rgb: Arc<Mutex<messages::SetRGB>>,
    rx_queue: tokio::sync::mpsc::Receiver<messages::RxMessage>,
    tx_close: tokio::sync::oneshot::Sender<()>,
    jh: tokio::task::JoinHandle<()>,
    tx_close_ping: tokio::sync::oneshot::Sender<()>,
    jh_ping: tokio::task::JoinHandle<()>,
     capturing: std::sync::atomic::AtomicBool,
}

impl Device {
    fn new(
        device: rusb::Device<rusb::GlobalContext>,
        descriptor: rusb::DeviceDescriptor
    ) -> anyhow::Result<Self> {
        const CONNECTED_RGB: messages::SetRGB = messages::SetRGB{
            r: 0,
            g: 0,
            b: 255,
        };
        let descriptor = Arc::new(descriptor);
        let device_handle = match device.open() {
            Ok(v) => Arc::new(v),
            Err(err) => anyhow::bail!("Failed to open device(bus = {}, address = {}, id = {}): {err}", device.bus_number(), device.address(), device.port_number())
        };
        match device_handle.set_auto_detach_kernel_driver(false) {
            Ok(()) => (),
            Err(err) => anyhow::bail!("Failed to set auto detach kernel driver: {err}")
        };
        match device_handle.claim_interface(0) {
            Ok(()) => (),
            Err(err) => anyhow::bail!("Failed to claim device on interface 0: {err}")
        };

        let id = Arc::new(Mutex::new(None));
        let meta_data = Arc::new(Mutex::new(None));
        let rgb = Arc::new(Mutex::new(CONNECTED_RGB));

        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let (tx_close, rx_close) = tokio::sync::oneshot::channel();
        let (tx_close_ping, rx_close_ping) = tokio::sync::oneshot::channel();
        let jh_ping = {
            let device_handle2 = device_handle.clone();
            tokio::task::spawn(async{
                let device_handle = device_handle2;
                let mut rx_close_ping = rx_close_ping;
                let mut interval = tokio::time::interval(Duration::from_millis(500));
                interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
                loop{
                    tokio::select! {
                        biased;
                        _ = &mut rx_close_ping => {
                            break;
                        },
                        _ = interval.tick() => {
                            match Self::send_internal(&device_handle, &messages::TxMessage::Ping) {
                                Ok(()) => (),
                                Err(err) => {
                                    eprintln!("Failed to send ping message: {err}");
                                    break;
                                }
                            }
                        }
                    }
                }
            })
        };
        let jh = {
            let id = id.clone();
            let meta_data = meta_data.clone();
            let device_handle2 = device_handle.clone();
            tokio::task::spawn(async move{
                let mut rx_close = rx_close;
                let tx = tx;
                let device_handle = device_handle2;
                loop{
                    let device_handle = device_handle.clone();
                    tokio::select! {
                        biased;
                        _ = &mut rx_close => {
                            break;
                        },
                        result = tokio::task::spawn_blocking(move ||{
                            let mut buf = [0u8; MAX_MESSAGE_SIZE as usize];
                            let out = device_handle.read_bulk(0x81, &mut buf, std::time::Duration::from_secs(120));
                            (buf, out)
                        }) => {
                            let result = match result {
                                Ok(v) => v,
                                Err(err) => {
                                    eprintln!("Panicked, whilst trying to read from device: {err}");
                                    break;
                                }
                            };
                            let (buf, result) = result;
                            let buf = match result {
                                Ok(len) => &buf[..len],
                                Err(err) => {
                                    eprintln!("Failed to read from device: {err}");
                                    break;
                                }
                            };
                            let message = match aglio::deserialize(buf) {
                                Ok(messages::RxMessage::Id(new_id)) => {
                                    let mut lock = id.lock().await;
                                    *lock = Some(new_id);
                                    continue;
                                },
                                Ok(messages::RxMessage::MetaData(new_meta_data)) => {
                                    let mut lock = meta_data.lock().await;
                                    *lock = Some(new_meta_data);
                                    continue;
                                },
                                Ok(message) => message,
                                Err(err) => {
                                    eprintln!("Failed to deserialize message: {err}");
                                    continue;
                                }
                            };
                            println!("Received message: {message:?}");
                            match tx.send(message).await {
                                Ok(()) => (),
                                Err(err) => {
                                    eprintln!("Failed to send message to channel: {err}");
                                    break;
                                }
                            }
                        }
                    }
                }
            })
        };


        let device = Device{
            device,
            device_handle,
            descriptor,
            id,
            meta_data,
            rgb,
            rx_queue: rx,
            tx_close,
            jh,
            tx_close_ping,
            jh_ping,
            capturing: std::sync::atomic::AtomicBool::new(false),
        };

        device.send(&messages::TxMessage::GetId)?;
        device.send(&messages::TxMessage::GetMetaData)?;
        device.send(&messages::TxMessage::SetRGB(CONNECTED_RGB))?;
        Ok(device)
    }

    fn send_internal(device_handle: &rusb::DeviceHandle<rusb::GlobalContext>, message: &messages::TxMessage) -> anyhow::Result<()> {
        match aglio::serialize(message) {
            Ok(v) => match device_handle.write_bulk(1, v.as_slice(), std::time::Duration::from_secs(1)){
                Ok(_) => Ok(()),
                Err(err) => anyhow::bail!("Failed to write to device: {err}")
            },
            Err(err) => anyhow::bail!("Failed to serialize message: {err}")
        }
    }

    #[inline]
    fn send(&self, message: &messages::TxMessage) -> anyhow::Result<()> {
        Self::send_internal(&self.device_handle, message)
    }

    pub fn start_capture(&self) -> anyhow::Result<()> {
        if self.capturing.compare_exchange(false, true, std::sync::atomic::Ordering::AcqRel, std::sync::atomic::Ordering::Acquire).is_ok() {
            match self.send(&messages::TxMessage::Start) {
                Ok(()) => (),
                Err(err) => {
                    eprintln!("Failed to start capture: {err}");
                    self.capturing.store(false, std::sync::atomic::Ordering::Release);
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub fn stop_capture(&self) -> anyhow::Result<()> {
        if self.capturing.compare_exchange(true, false, std::sync::atomic::Ordering::AcqRel, std::sync::atomic::Ordering::Acquire).is_ok() {
            match self.send(&messages::TxMessage::Stop) {
                Ok(()) => (),
                Err(err) => {
                    eprintln!("Failed to stop capture: {err}");
                    self.capturing.store(true, std::sync::atomic::Ordering::Release);
                    return Err(err);
                }
            }
        }
        Ok(())
    }
    pub async fn id(&self) -> Option<messages::Id> {
        self.id.lock().await.clone()
    }
    pub fn descriptor(&self) -> &rusb::DeviceDescriptor {
        &self.descriptor
    }
}
impl Drop for Device {
    fn drop(&mut self) {
        if self.capturing {
            match self.stop_capture() {
                Ok(()) => (),
                Err(err) => {
                    eprintln!("Failed to stop capture: {err}");
                }
            }
        }

        let (mut tx, _) = tokio::sync::oneshot::channel();
        core::mem::swap(&mut self.tx_close, &mut tx);
        tx.send(()).ok();
        self.jh.abort();

        let (mut tx, _) = tokio::sync::oneshot::channel();
        core::mem::swap(&mut self.tx_close_ping, &mut tx);
        tx.send(()).ok();
        self.jh_ping.abort();
    }
}