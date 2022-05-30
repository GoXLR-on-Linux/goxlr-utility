use std::str::FromStr;
use clap::{AppSettings, Args, Parser, Subcommand};
use goxlr_types::{ChannelName, ColourDisplay, ColourOffStyle, FaderName, InputDevice, MuteFunction, OutputDevice};

// TODO: Likely going to shuffle this to use subcommands rather than parameters..

#[derive(Parser, Debug)]
#[clap(about, version, author)]
#[clap(global_setting = AppSettings::ArgRequiredElseHelp)]
pub struct Cli {
    /// The specific device's serial number to execute commands on.
    /// This field is optional if you have exactly one GoXLR, but required if you have more.
    #[clap(long)]
    pub device: Option<String>,

    /// Display the device information after any subcommands have been executed.
    #[clap(long)]
    pub status: bool,

    #[clap(flatten, help_heading = "Profile Management")]
    pub profile: Profile,

    #[clap(flatten, help_heading = "Fader controls")]
    pub faders: FaderControls,

    #[clap(flatten, help_heading = "Channel volumes")]
    pub channel_volumes: ChannelVolumes,

    #[clap(flatten, help_heading = "Microphone controls")]
    pub microphone_controls: MicrophoneControls,

    #[clap(subcommand)]
    pub subcommands: Option<SubCommands>,
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
#[clap(setting = AppSettings::DeriveDisplayOrder)]
pub enum SubCommands {
    /// Adjust Channel Volumes
    Volume {
        /// The Channel To Change
        #[clap(arg_enum)]
        channel: ChannelName,

        /// The new volume as a percentage [0 - 100]
        #[clap(parse(try_from_str=parse_volume))]
        volume_percent: Option<u8>
    },

    /// Commands to manipulate the individual GoXLR Faders
    Faders {
        #[clap(subcommand)]
        fader: Option<FaderCommands>
    },

    /// Commands to manipulate all GoXLR faders at once
    FadersAll {
        #[clap(subcommand)]
        command: Option<AllFaderCommands>
    },

    /// Commands for configuring the cough button
    Cough {
        #[clap(subcommand)]
        command: Option<CoughCommands>
    },

    /// Commands to manipulate the GoXLR Router
    Router {
        /// The input device
        #[clap(arg_enum)]
        input: InputDevice,

        /// The output device
        #[clap(arg_enum)]
        output: OutputDevice,

        /// Is routing enabled between these two devices? [true | false]
        #[clap(parse(try_from_str))]
        enabled: Option<bool>
    },
}

fn parse_volume(s: &str) -> Result<u8, String> {
    let value = u8::from_str(s);
    if value.is_err() {
        return Err(String::from("Value must be between 0 and 100"));
    }

    let value = value.unwrap();
    if value > 100 {
        return Err(String::from("Value must be lower than 100"));
    }
    Ok(value)
}


#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
pub enum FaderCommands {
    /// Assign a new Channel to a Fader
    Channel {
        /// The Fader to Change
        #[clap(arg_enum)]
        fader: FaderName,

        /// The New Channel Name
        #[clap(arg_enum)]
        channel: Option<ChannelName>,
    },

    /// Change the behaviour of a Fader Mute Button
    MuteBehaviour {
        /// The Fader to Change
        #[clap(arg_enum)]
        fader: FaderName,

        /// Where a single press will mute (Hold will always Mute to All)
        #[clap(arg_enum)]
        mute_behaviour: Option<MuteFunction>
    },

    /// Set the appearance of Slider Lighting
    Display {
        /// The Fader to Change
        #[clap(arg_enum)]
        fader: FaderName,

        /// The new display method
        #[clap(arg_enum)]
        display: Option<ColourDisplay>
    },

    /// Sets the Top and Bottom colours of a fader
    Colour {
        /// The Fader name to Change
        #[clap(arg_enum)]
        fader: FaderName,

        /// Top colour in hex format [RRGGBB]
        top: String,

        /// Bottom colour in hex format [RRGGBB]
        bottom: String
    },

    ButtonColour {
        /// The Fader to Change
        #[clap(arg_enum)]
        fader: FaderName,

        /// The primary button colour [RRGGBB]
        colour_one: String,

        /// How the button should be presented when 'off'
        #[clap(arg_enum)]
        off_style: ColourOffStyle,

        /// The secondary button colour [RRGGBB]
        colour_two: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
pub enum CoughCommands {
    /// Change the behaviour of a Fader Mute Button
    MuteBehaviour {
        /// Where a single press will mute (Hold will always Mute to All)
        #[clap(arg_enum)]
        mute_behaviour: Option<MuteFunction>
    },

    Colour {
        /// The primary button colour [RRGGBB]
        colour_one: String,

        /// How the button should be presented when 'off'
        #[clap(arg_enum)]
        off_style: ColourOffStyle,

        /// The secondary button colour [RRGGBB]
        colour_two: Option<String>,
    }
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
pub enum AllFaderCommands {
    /// Set the appearance of Slider Lighting
    Display {
        /// The new display method
        #[clap(arg_enum)]
        display: ColourDisplay
    },

    /// Set the colour of all GoXLR Faders
    Colour {
        /// Top colour in hex format [RRGGBB]
        top: String,

        /// Bottom colour in hex format [RRGGBB]
        bottom: String
    },

    /// Set the colours of all the fader buttons
    ButtonColour {
        /// The primary button colour [RRGGBB]
        colour_one: String,

        /// How the button should be presented when 'off'
        #[clap(arg_enum)]
        off_style: ColourOffStyle,

        /// The secondary button colour [RRGGBB]
        colour_two: Option<String>,
    }
}