#[derive(clap_derive::Parser, Debug, Clone, Default)]
pub struct Options {
    #[arg(long, default_value = "false")]
    ///Prints the current version. Version is set via the crate version.
    version: bool,
    #[arg(long, default_value = "false")]
    ///Prints all connected devices, color identical to current LED color
    search: bool,
    #[arg(long)]
    ///Start the devices with the given UUIDs
    device: Vec<uuid::Uuid>,
    #[arg(short, long, default_value = "false")]
    ///Add extra for debugging information
    verbose: bool,
    #[arg(short, long)]
    ///Add a file you want the data to be saved in
    output: Option<std::path::PathBuf>,
    #[arg(short, long, default_value = "false")]
    ///Add if you want the file to be in a JSON format
    json: bool,
    #[arg(short, long, default_value = "false")]
    ///Starts the websocket. To send data a UUID has to be given
    websocket: bool,
    #[arg(short, long, default_value = "8080")]
    ///Sets the port for the websocket to start on.
    port: u16,
}
impl Options{
    pub const fn version(&self) -> bool { self.version }
    pub const fn search(&self) -> bool { self.search }
    pub fn device(&self) -> &[uuid::Uuid] { &self.device.as_slice() }
    pub const fn verbose(&self) -> bool { self.verbose }
    pub const fn output(&self) -> Option<&std::path::PathBuf> { self.output.as_ref() }
    pub const fn json(&self) -> bool { self.json }
    pub const fn websocket(&self) -> bool { self.websocket }
    pub const fn port(&self) -> u16 { self.port }
}