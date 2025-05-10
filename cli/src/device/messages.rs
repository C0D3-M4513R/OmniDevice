#[derive(Debug, Clone, Copy, serde_derive::Deserialize, serde_derive::Serialize)]
pub struct Version {
    major: u8,
    minor: u8,
    patch: u8,
}
impl Version {
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self { major, minor, patch }
    }
    pub const fn major(&self) -> u8 { self.major }
    pub const fn minor(&self) -> u8 { self.minor }
    pub const fn patch(&self) -> u8 { self.patch }
}
#[repr(u8)]
pub enum MessageType {
    Id = 0,
    MeasureData = 1,
    MetaData = 2,
}
#[repr(u8)]
#[derive(Debug, serde_derive::Deserialize, serde_derive::Serialize)]
pub enum RxMessage {
    Id(Id) = 0,
    MeasureData(MeasureData) = 1,
    MetaData(MetaData) = 2,
}
#[derive(Debug, Clone, serde_derive::Deserialize, serde_derive::Serialize)]
pub struct Id {
    serial: String,
    r#type: String,
    sample_rate: u32,
    hw_version: Version,
    sw_version: Version,
    sw_git_hash: String,
}
impl Id {
    pub const fn serial(&self) -> &String { &self.serial }
    pub const fn r#type(&self) -> &String { &self.r#type }
    pub const fn sample_rate(&self) -> u32 { self.sample_rate }
    pub const fn hw_version(&self) -> Version { self.hw_version }
    pub const fn sw_version(&self) -> Version { self.sw_version }
    pub const fn sw_git_hash(&self) -> &String { &self.sw_git_hash }
}
#[derive(Debug, serde_derive::Deserialize, serde_derive::Serialize)]
pub struct StartOfFrame{
    content: u16,
}
#[derive(Debug, serde_derive::Deserialize, serde_derive::Serialize)]
pub struct MeasureData{
    package_counter: u8,
    sof: StartOfFrame,
    data: Vec<u16>,
}
#[derive(Debug, Clone, serde_derive::Deserialize, serde_derive::Serialize)]
pub struct MetaData{
    data: String,
}

#[repr(u8)]
#[derive(serde_derive::Serialize)]
pub enum TxMessage {
    GetId = 0,
    Ping = 1,
    Start = 2,
    Stop = 3,
    SetRGB(SetRGB) = 4,
    SetMetaData(SetMetaData) = 5,
    GetMetaData = 6,
}
#[derive(Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct SetRGB {
    pub(super) r: u8,
    pub(super) g: u8,
    pub(super) b: u8,
}
impl SetRGB {
    pub const fn r(&self) -> u8 { self.r }
    pub const fn g(&self) -> u8 { self.g }
    pub const fn b(&self) -> u8 { self.b }
}
#[derive(serde_derive::Serialize)]
pub struct SetMetaData {
    data: String,
}