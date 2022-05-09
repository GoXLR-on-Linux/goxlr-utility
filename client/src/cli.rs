use clap::{Args, Parser, Subcommand};
use goxlr_types::{ChannelName, InputDevice, OutputDevice};

// TODO: Likely going to shuffle this to use subcommands rather than parameters..

#[derive(Parser, Debug)]
#[clap(about, version, author)]
pub struct Cli {
    /// The specific device's serial number to execute commands on.
    /// This field is optional if you have exactly one GoXLR, but required if you have more.
    #[clap(long)]
    pub device: Option<String>,

    #[clap(flatten, help_heading = "Profile Management")]
    pub profile: Profile,

    #[clap(flatten, help_heading = "Fader controls")]
    pub faders: FaderControls,

    #[clap(flatten, help_heading = "Channel volumes")]
    pub channel_volumes: ChannelVolumes,

    #[clap(flatten, help_heading = "Microphone controls")]
    pub microphone_controls: MicrophoneControls,

    #[clap(subcommand)]
    pub router: Option<RouterCommands>

}

#[derive(Debug, Args)]
pub struct Profile {
    /// List all profiles available for loading
    #[clap(long, display_order=1)]
    pub list_profiles: bool,

    /// List all microphone profiles available for loading
    #[clap(long, display_order=2)]
    pub list_mic_profiles: bool,

    /// Load a GoXLR Profile
    #[clap(long, name="PROFILE", display_order=3)]
    pub load_profile: Option<String>,

    /// Load a GoXLR Microphone Profile
    #[clap(long, name="MIC_PROFILE", display_order=4)]
    pub load_mic_profile: Option<String>,

    /// Saves the current configuration to disk
    #[clap(long, display_order=5)]
    pub save_profile: bool,

    /// Save the currently configured microphone profile to disk
    #[clap(long, display_order=6)]
    pub save_mic_profile: bool,
}

#[derive(Debug, Args)]
pub struct FaderControls {
    /// Assign fader A
    #[clap(arg_enum, long)]
    pub fader_a: Option<ChannelName>,

    /// Assign fader B
    #[clap(arg_enum, long)]
    pub fader_b: Option<ChannelName>,

    /// Assign fader C
    #[clap(arg_enum, long)]
    pub fader_c: Option<ChannelName>,

    /// Assign fader D
    #[clap(arg_enum, long)]
    pub fader_d: Option<ChannelName>,
}

#[derive(Debug, Args)]
pub struct ChannelVolumes {
    /// Set Mic volume (0-255)
    #[clap(long)]
    pub mic_volume: Option<u8>,

    /// Set Line-In volume (0-255)
    #[clap(long)]
    pub line_in_volume: Option<u8>,

    /// Set Console volume (0-255)
    #[clap(long)]
    pub console_volume: Option<u8>,

    /// Set System volume (0-255)
    #[clap(long)]
    pub system_volume: Option<u8>,

    /// Set Game volume (0-255)
    #[clap(long)]
    pub game_volume: Option<u8>,

    /// Set Chat volume (0-255)
    #[clap(long)]
    pub chat_volume: Option<u8>,

    /// Set Sample volume (0-255)
    #[clap(long)]
    pub sample_volume: Option<u8>,

    /// Set Music volume (0-255)
    #[clap(long)]
    pub music_volume: Option<u8>,

    /// Set Headphones volume (0-255)
    #[clap(long)]
    pub headphones_volume: Option<u8>,

    /// Set Mic-Monitor volume (0-255)
    #[clap(long)]
    pub mic_monitor_volume: Option<u8>,

    /// Set Line-Out volume (0-255)
    #[clap(long)]
    pub line_out_volume: Option<u8>,
}

#[derive(Debug, Args)]
pub struct MicrophoneControls {
    /// Set the gain of the plugged in dynamic (XLR) microphone.
    /// Value is in decibels and recommended to be lower than 72dB.
    #[clap(long)]
    pub dynamic_gain: Option<u16>,

    /// Set the gain of the plugged in condenser (XLR with phantom power) microphone.
    /// Value is in decibels and recommended to be lower than 72dB.
    #[clap(long)]
    pub condenser_gain: Option<u16>,

    /// Set the gain of the plugged in jack (3.5mm) microphone.
    /// Value is in decibels and recommended to be lower than 72dB.
    #[clap(long)]
    pub jack_gain: Option<u16>,
}

#[derive(Subcommand, Debug)]
pub enum RouterCommands {
    /// Manipulate the GoXLR Router
    Router {
        /// The input device
        #[clap(arg_enum)]
        input: InputDevice,

        /// The output device
        #[clap(arg_enum)]
        output: OutputDevice,

        /// Is routing enabled between these two devices? [true | false]
        #[clap(parse(try_from_str))]
        enabled: bool
    },
}

