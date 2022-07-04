use std::str::FromStr;
use clap::{AppSettings, Args, Parser, Subcommand};
use goxlr_types::{ChannelName, FaderDisplayStyle, ButtonColourOffStyle, FaderName, InputDevice, MuteFunction, OutputDevice, ButtonColourTargets, ButtonColourGroups, GateTimes, CompressorRatio, CompressorAttackTime, CompressorReleaseTime, EqFrequencies, MiniEqFrequencies};

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
        command: ProfileType,
    },

    /// Adjust the microphone settings (Eq, Gate and Compressor)
    Microphone {
        #[clap(subcommand)]
        command: MicrophoneCommands,
    },

    /// Adjust Channel Volumes
    Volume {
        /// The Channel To Change
        #[clap(arg_enum)]
        channel: ChannelName,

        /// The new volume as a percentage [0 - 100]
        #[clap(parse(try_from_str=percent_value))]
        volume_percent: u8
    },

    /// Configure the Bleep Button
    BleepVolume {
        /// Set Bleep Button Volume
        #[clap(parse(try_from_str=percent_value))]
        volume_percent: u8
    },

    /// Commands to manipulate the individual GoXLR Faders
    Faders {
        #[clap(subcommand)]
        fader: FaderCommands
    },

    /// Commands for configuring the cough button
    CoughButton {
        #[clap(subcommand)]
        command: CoughButtonBehaviours,
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
        enabled: bool
    },

    /// Commands to control the GoXLR lighting
    Lighting {
        #[clap(subcommand)]
        command: LightingCommands
    }
}

fn percent_value(s: &str) -> Result<u8, String> {
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
pub enum CoughButtonBehaviours {
    ButtonIsHold {
        #[clap(parse(try_from_str))]
        is_hold: bool
    },

    MuteBehaviour {
        /// Where a single press will mute (Hold will always Mute to All)
        #[clap(arg_enum)]
        mute_behaviour: MuteFunction
    }
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
    Load {
        /// The profile name to load
        profile_name: String,
    },

    /// Save the currently running profile
    #[clap(unset_setting = AppSettings::ArgRequiredElseHelp)]
    Save {},

    /// Save the currently running profile with a new name
    SaveAs {
        /// The new Profile Name
        profile_name: String,
    }
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum MicrophoneCommands {
    /// Configure the Equaliser for the Full GoXLR Device
    Equaliser {
        #[clap(subcommand)]
        command: EqualiserCommands,
    },

    /// Configure the Equaliser for the GoXLR Mini
    EqualiserMini {
        #[clap(subcommand)]
        command: EqualiserMiniCommands,
    },

    /// Configure the microphone noise gate
    NoiseGate {
        #[clap(subcommand)]
        command: NoiseGateCommands,
    },

    /// Configure the Microphone Compressor
    Compressor {
        #[clap(subcommand)]
        command: CompressorCommands,
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum EqualiserMiniCommands {
    /// Fine tune the Equaliser Frequencies
    Frequency {
        #[clap(arg_enum)]
        /// The Frequency to Modify
        frequency: MiniEqFrequencies,

        #[clap(parse(try_from_str=parse_full_frequency))]
        /// The new Frequency
        value: f32,
    },

    /// Set the Gain Value for frequencies
    Gain {
        #[clap(arg_enum)]
        /// The Frequency to modify
        frequency: MiniEqFrequencies,

        #[clap(parse(try_from_str=parse_gain))]
        #[clap(allow_hyphen_values = true)]
        /// The new Gain Value
        gain: i8,
    }
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum EqualiserCommands {
    /// Fine tune the Equaliser Frequencies
    Frequency {
        #[clap(arg_enum)]
        /// The Frequency to modify
        frequency: EqFrequencies,

        #[clap(parse(try_from_str=parse_full_frequency))]
        /// The new frequency
        value: f32,
    },
    Gain {
        #[clap(arg_enum)]
        /// The Frequency to Modify
        frequency: EqFrequencies,

        #[clap(parse(try_from_str=parse_gain))]
        #[clap(allow_hyphen_values = true)]
        /// The new Gain Value
        gain: i8
    }
}

// TODO: The mini has a known smaller frequency range than the full device, find it.
fn parse_full_frequency(s: &str) -> Result<f32, String> {
    let value = f32::from_str(s);
    if value.is_err() {
        return Err(String::from("Value must be between 300 and 18000hz"));
    }

    let value = value.unwrap();
    if value > 18000.0 {
        return Err(String::from("Value must be lower than 18000hz"));
    }

    if value < 300.0 {
        return Err(String::from("Value must be higher than 300hz"));
    }
    Ok(value)
}

fn parse_gain(s: &str) -> Result<i8, String> {
    let value = i8::from_str(s);
    if value.is_err() {
        return Err(String::from("Value must be between -9 and 9db"));
    }

    let value = value.unwrap();
    if value > 9 {
        return Err(String::from("Value must be 9db or lower"));
    }

    if value < -9 {
        return Err(String::from("Value must be -9db or higher"));
    }
    Ok(value)
}


#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum NoiseGateCommands {
    /// Activation Threshold in dB [-59 - 0]
    Threshold {
        #[clap(parse(try_from_str=parse_gate_threshold))]
        #[clap(allow_hyphen_values = true)]
        value: i8,
    },

    /// Attenuation Percentage [0 - 100]
    Attenuation {
        #[clap(parse(try_from_str=percent_value))]
        value: u8,
    },

    /// Attack Time
    Attack {
        #[clap(arg_enum)]
        value: GateTimes,
    },

    /// Release Time
    Release {
        #[clap(arg_enum)]
        value: GateTimes,
    },

    /// Is Gate Active?
    Active {
        #[clap(parse(try_from_str))]
        enabled: bool,
    },
}

fn parse_gate_threshold(s: &str) -> Result<i8, String> {
    let value = i8::from_str(s);
    if value.is_err() {
        return Err(String::from("Value must be between -59 and 0"));
    }

    let value = value.unwrap();
    if value > 0 {
        return Err(String::from("Value must be lower than 0"));
    }

    if value < -59 {
        return Err(String::from("Value must be higher than -59"));
    }
    Ok(value)
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum CompressorCommands {
    /// Activation Threshold in dB [-24 - 0]
    Threshold {
        #[clap(parse(try_from_str=parse_compressor_threshold))]
        #[clap(allow_hyphen_values = true)]
        value: i8
    },
    Ratio {
        #[clap(arg_enum)]
        value: CompressorRatio,
    },
    Attack {
        #[clap(arg_enum)]
        value: CompressorAttackTime,
    },
    Release {
        #[clap(arg_enum)]
        value: CompressorReleaseTime,
    },
    MakeUp {
        #[clap(parse(try_from_str=parse_compressor_makeup))]
        value: u8,
    }
}

fn parse_compressor_threshold(s: &str) -> Result<i8, String> {
    let value = i8::from_str(s);
    if value.is_err() {
        return Err(String::from("Value must be between -24 and 0"));
    }

    let value = value.unwrap();
    if value > 0 {
        return Err(String::from("Value must be 0 or below"));
    }

    if value < -24 {
        return Err(String::from("Value must be -24 or higher"));
    }
    Ok(value)
}

fn parse_compressor_makeup(s: &str) -> Result<u8, String> {
    let value = u8::from_str(s);
    if value.is_err() {
        return Err(String::from("Value must be between 0 and 24"));
    }

    let value = value.unwrap();
    if value > 24 {
        return Err(String::from("Value must be 24 or lower"));
    }
    Ok(value)
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
        channel: ChannelName,
    },

    /// Change the behaviour of a Fader Mute Button
    MuteBehaviour {
        /// The Fader to Change
        #[clap(arg_enum)]
        fader: FaderName,

        /// Where a single press will mute (Hold will always Mute to All)
        #[clap(arg_enum)]
        mute_behaviour: MuteFunction
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
        mute_behaviour: MuteFunction
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum LightingCommands {
    /// Configure Lighting for a specific fader
    Fader {
        #[clap(subcommand)]
        command: FaderLightingCommands
    },

    /// Configure lighting for all faders at once
    FadersAll {
        #[clap(subcommand)]
        command: FadersAllLightingCommands
    },

    /// Configure lighting for a specific button
    Button {
        #[clap(subcommand)]
        command: ButtonLightingCommands
    },

    /// Configure lighting for a group of common bottoms
    ButtonGroup {
        #[clap(subcommand)]
        command: ButtonGroupLightingCommands
    }
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum FaderLightingCommands {
    Display {
        /// The Fader to Change
        #[clap(arg_enum)]
        fader: FaderName,

        /// The new display method
        #[clap(arg_enum)]
        display: FaderDisplayStyle
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
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum FadersAllLightingCommands {
    Display {
        /// The new display method
        #[clap(arg_enum)]
        display: FaderDisplayStyle
    },

    /// Sets the Top and Bottom colours of a fader
    Colour {
        /// Top colour in hex format [RRGGBB]
        top: String,

        /// Bottom colour in hex format [RRGGBB]
        bottom: String
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum ButtonLightingCommands {
    Colour {
        /// The Button to change
        #[clap(arg_enum)]
        button: ButtonColourTargets,

        /// The primary button colour [RRGGBB]
        colour_one: String,

        /// The secondary button colour [RRGGBB]
        colour_two: Option<String>,
    },

    OffStyle {
        /// The Button to change
        #[clap(arg_enum)]
        button: ButtonColourTargets,

        /// How the button should be presented when 'off'
        #[clap(arg_enum)]
        off_style: ButtonColourOffStyle,
    }
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum ButtonGroupLightingCommands {
    Colour {
        /// The group to change
        #[clap(arg_enum)]
        group: ButtonColourGroups,

        /// The primary button colour [RRGGBB]
        colour_one: String,

        /// The secondary button colour [RRGGBB]
        colour_two: Option<String>,
    },

    OffStyle {
        /// The group to change
        #[clap(arg_enum)]
        group: ButtonColourGroups,

        /// How the button should be presented when 'off'
        #[clap(arg_enum)]
        off_style: ButtonColourOffStyle,
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
        display: FaderDisplayStyle
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
        off_style: ButtonColourOffStyle,

        /// The secondary button colour [RRGGBB]
        colour_two: Option<String>,
    }
}