use std::str::FromStr;
use clap::{AppSettings, Args, Parser, Subcommand};
use goxlr_types::{ChannelName, ColourDisplay, ColourOffStyle, FaderName, InputDevice, MuteFunction, OutputDevice};

// TODO: Likely going to shuffle this to use subcommands rather than parameters..

#[derive(Parser, Debug)]
#[clap(about, version, author)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub struct Cli {
    /// The specific device's serial number to execute commands on.
    /// This field is optional if you have exactly one GoXLR, but required if you have more.
    #[clap(long)]
    pub device: Option<String>,

    /// Display the device information after any subcommands have been executed.
    #[clap(long)]
    pub status: bool,

    /// Display device information as JSON after command..
    #[clap(long)]
    pub status_json: bool,

    #[clap(flatten, help_heading = "Microphone controls")]
    pub microphone_controls: MicrophoneControls,

    #[clap(subcommand)]
    pub subcommands: Option<SubCommands>,
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
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum SubCommands {
    /// Profile Settings
    Profiles {
        #[clap(subcommand)]
        command: Option<ProfileType>,
    },

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

    /// Configure the Bleep Button
    Bleep {
        #[clap(subcommand)]
        command: Option<BleepCommands>
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
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum ProfileType {
    /// General Device Profile
    Device {
        #[clap(subcommand)]
        command: ProfileAction
    },

    /// Microphone Profile
    Microphone {
        #[clap(subcommand)]
        command: ProfileAction
    }
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum ProfileAction {
    /// Load a profile by name
    #[clap(setting = AppSettings::ArgRequiredElseHelp)]
    Load {
        /// The profile name to load
        profile_name: String,
    },

    /// Save the currently running profile
    #[clap(unset_setting = AppSettings::ArgRequiredElseHelp)]
    Save {},

    /// Save the currently running profile with a new name
    #[clap(setting = AppSettings::ArgRequiredElseHelp)]
    SaveAs {
        /// The new Profile Name
        profile_name: String,
    }
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
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
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
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
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum BleepCommands {
    /// Change the behaviour of the Swear Button
    Volume {
        /// Set Bleep Button Volume
        #[clap(parse(try_from_str=parse_volume))]
        volume_percent: Option<u8>
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
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
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