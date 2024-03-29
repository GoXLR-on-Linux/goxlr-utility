use clap::{Parser, ValueEnum};
use directories::ProjectDirs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(about, version, author)]
pub struct Cli {
    /// Minimum log level to print out
    #[arg(long, value_enum)]
    pub log_level: Option<LevelFilter>,

    /// Location of the daemon configuration file on disk
    #[arg(long, default_value_os_t = default_config_location())]
    pub config: PathBuf,

    /// Disable the HTTP Server and Client Web UI
    #[arg(long)]
    pub http_disable: bool,

    /// Define the port the HTTP Server should listen on
    #[arg(long, default_value = "14564")]
    pub http_port: u16,

    /// Enable CORS on the HTTP Server to allow cross-origin communication
    #[arg(long)]
    pub http_enable_cors: bool,

    /// Set the HTTP Bind Address (0.0.0.0 for all interfaces)
    #[arg(long)]
    pub http_bind_address: Option<String>,

    /// Disable the Tray Icon
    #[arg(long)]
    pub disable_tray: Option<bool>,

    /// Force Run the Daemon as Root
    #[arg(long)]
    pub force_root: bool,

    /// Automatically Launch the UI on Start..
    #[arg(long)]
    pub start_ui: bool,

    /// Force regular expression to use when finding the Sampler Input
    #[arg(long)]
    pub override_sample_input_device: Option<String>,

    /// Force regular expression to use when finding the Sampler Output
    #[arg(long)]
    pub override_sample_output_device: Option<String>,
}

fn default_config_location() -> PathBuf {
    let proj_dirs = ProjectDirs::from("org", "GoXLR-on-Linux", "GoXLR-Utility")
        .expect("Couldn't find project directory");

    proj_dirs.config_dir().join("settings.json")
}

#[repr(usize)]
#[derive(ValueEnum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum LevelFilter {
    /// A level lower than all log levels.
    Off,
    /// Corresponds to the `Error` log level.
    Error,
    /// Corresponds to the `Warn` log level.
    Warn,
    /// Corresponds to the `Info` log level.
    Info,
    /// Corresponds to the `Debug` log level.
    Debug,
    /// Corresponds to the `Trace` log level.
    Trace,
}
