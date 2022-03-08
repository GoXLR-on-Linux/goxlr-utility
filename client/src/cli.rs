use clap::{Args, Parser};
use goxlr_types::ChannelName;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
pub struct Cli {
    /// The specific device's serial number to execute commands on.
    /// This field is optional if you have exactly one GoXLR, but required if you have more.
    #[clap(long)]
    pub device: Option<String>,

    #[clap(long)]
    /// Load the specific profile from disk, by name
    pub profile: Option<String>,

    #[clap(flatten, help_heading = "Fader controls")]
    pub faders: FaderControls,

    #[clap(flatten, help_heading = "Channel volumes")]
    pub channel_volumes: ChannelVolumes,

    #[clap(flatten, help_heading = "Channel states")]
    pub channel_states: ChannelStates,

    #[clap(flatten, help_heading = "Microphone controls")]
    pub microphone_controls: MicrophoneControls,
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
pub struct ChannelStates {
    /// Set Mic muted status (true/false)
    #[clap(long)]
    pub mic_muted: Option<bool>,

    /// Set Line-In muted status (true/false)
    #[clap(long)]
    pub line_in_muted: Option<bool>,

    /// Set Console muted status (true/false)
    #[clap(long)]
    pub console_muted: Option<bool>,

    /// Set System muted status (true/false)
    #[clap(long)]
    pub system_muted: Option<bool>,

    /// Set Game muted status (true/false)
    #[clap(long)]
    pub game_muted: Option<bool>,

    /// Set Chat muted status (true/false)
    #[clap(long)]
    pub chat_muted: Option<bool>,

    /// Set Sample muted status (true/false)
    #[clap(long)]
    pub sample_muted: Option<bool>,

    /// Set Music muted status (true/false)
    #[clap(long)]
    pub music_muted: Option<bool>,

    /// Set Headphones muted status (true/false)
    #[clap(long)]
    pub headphones_muted: Option<bool>,

    /// Set Mic-Monitor muted status (true/false)
    #[clap(long)]
    pub mic_monitor_muted: Option<bool>,

    /// Set Line-Out muted status (true/false)
    #[clap(long)]
    pub line_out_muted: Option<bool>,
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
