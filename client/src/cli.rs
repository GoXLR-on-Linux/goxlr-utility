use clap::{AppSettings, Args, Parser, Subcommand};
use goxlr_types::{
    Button, ButtonColourGroups, ButtonColourOffStyle, ChannelName, CompressorAttackTime,
    CompressorRatio, CompressorReleaseTime, EchoStyle, EffectBankPresets, EncoderColourTargets,
    EqFrequencies, FaderDisplayStyle, FaderName, GateTimes, GenderStyle, HardTuneSource,
    HardTuneStyle, InputDevice, MegaphoneStyle, MiniEqFrequencies, MuteFunction, MuteState,
    OutputDevice, PitchStyle, ReverbStyle, RobotRange, RobotStyle, SampleBank, SampleButtons,
    SamplePlayOrder, SamplePlaybackMode, SimpleColourTargets,
};
use std::str::FromStr;

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

    #[clap(long)]
    pub status_http: bool,

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
        volume_percent: u8,
    },

    /// Configure the Bleep Button
    BleepVolume {
        /// Set Bleep Button Volume
        #[clap(parse(try_from_str=percent_value))]
        volume_percent: u8,
    },

    /// Commands to manipulate the individual GoXLR Faders
    Faders {
        #[clap(subcommand)]
        fader: FaderCommands,
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
        enabled: bool,
    },

    /// Commands to control the GoXLR lighting
    Lighting {
        #[clap(subcommand)]
        command: LightingCommands,
    },

    /// Commands to Control the Effects Panel
    Effects {
        #[clap(subcommand)]
        command: EffectsCommands,
    },

    Sampler {
        #[clap[subcommand]]
        command: SamplerCommands,
    },
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

fn percent_value_float(s: &str) -> Result<f32, String> {
    let value = f32::from_str(s);
    if value.is_err() {
        return Err(String::from("Value must be between 0 and 100"));
    }

    let value = value.unwrap();
    if !(0.0..=100.0).contains(&value) {
        return Err(String::from("Value must be between 0 and 100"));
    }

    Ok(value)
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum CoughButtonBehaviours {
    ButtonIsHold {
        #[clap(parse(try_from_str))]
        is_hold: bool,
    },

    MuteBehaviour {
        /// Where a single press will mute (Hold will always Mute to All)
        #[clap(arg_enum)]
        mute_behaviour: MuteFunction,
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum ProfileType {
    /// General Device Profile
    Device {
        #[clap(subcommand)]
        command: ProfileAction,
    },

    /// Microphone Profile
    Microphone {
        #[clap(subcommand)]
        command: ProfileAction,
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum ProfileAction {
    /// Create a new profile
    New { profile_name: String },

    /// Load a profile by name
    Load {
        /// The profile name to load
        profile_name: String,
    },

    /// Load a Profiles Colours Only
    LoadColours {
        /// The name of the profile to load colours from
        profile_name: String,
    },

    /// Save the currently running profile
    #[clap(unset_setting = AppSettings::ArgRequiredElseHelp)]
    Save {},

    /// Save the currently running profile with a new name
    SaveAs {
        /// The new Profile Name
        profile_name: String,
    },
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

    /// Set the DeEss percentage
    DeEss {
        #[clap(parse(try_from_str=percent_value))]
        level: u8,
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
    },
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
        gain: i8,
    },
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
        value: i8,
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
        value: i8,
    },
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

fn parse_compressor_makeup(s: &str) -> Result<i8, String> {
    let value = i8::from_str(s);
    if value.is_err() {
        return Err(String::from("Value must be between 0 and 24"));
    }

    let value = value.unwrap();
    if !(-6..=24).contains(&value) {
        return Err(String::from("Value must between -4 and 24"));
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
        mute_behaviour: MuteFunction,
    },

    /// Sets the Current Mute State of the Fader
    MuteState {
        /// The Fader to Change
        #[clap(arg_enum)]
        fader: FaderName,

        /// The new State
        #[clap(arg_enum)]
        state: MuteState,
    },

    Scribbles {
        #[clap(subcommand)]
        command: Scribbles,
    },
}
#[derive(Subcommand, Debug)]
pub enum Scribbles {
    Icon {
        #[clap(arg_enum)]
        fader: FaderName,
        name: String,
    },

    Text {
        #[clap(arg_enum)]
        fader: FaderName,
        text: String,
    },

    Number {
        #[clap(arg_enum)]
        fader: FaderName,
        text: String,
    },

    Invert {
        #[clap(arg_enum)]
        fader: FaderName,

        #[clap(parse(try_from_str))]
        inverted: bool,
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
        mute_behaviour: MuteFunction,
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum LightingCommands {
    /// Configure Lighting for a specific fader
    Fader {
        #[clap(subcommand)]
        command: FaderLightingCommands,
    },

    /// Configure lighting for all faders at once
    FadersAll {
        #[clap(subcommand)]
        command: FadersAllLightingCommands,
    },

    /// Configure lighting for a specific button
    Button {
        #[clap(subcommand)]
        command: ButtonLightingCommands,
    },

    /// Configure lighting for a group of common bottoms
    ButtonGroup {
        #[clap(subcommand)]
        command: ButtonGroupLightingCommands,
    },

    SimpleColour {
        #[clap(arg_enum)]
        target: SimpleColourTargets,
        colour: String,
    },

    EncoderColour {
        /// The Encoder to Change
        #[clap(arg_enum)]
        target: EncoderColourTargets,

        /// The 'Inactive' Colour?
        colour_one: String,

        /// The 'Active' Colour
        colour_two: String,

        /// The Knob Colour
        colour_three: String,
    },
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
        display: FaderDisplayStyle,
    },

    /// Sets the Top and Bottom colours of a fader
    Colour {
        /// The Fader name to Change
        #[clap(arg_enum)]
        fader: FaderName,

        /// Top colour in hex format [RRGGBB]
        top: String,

        /// Bottom colour in hex format [RRGGBB]
        bottom: String,
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum FadersAllLightingCommands {
    Display {
        /// The new display method
        #[clap(arg_enum)]
        display: FaderDisplayStyle,
    },

    /// Sets the Top and Bottom colours of a fader
    Colour {
        /// Top colour in hex format [RRGGBB]
        top: String,

        /// Bottom colour in hex format [RRGGBB]
        bottom: String,
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum ButtonLightingCommands {
    Colour {
        /// The Button to change
        #[clap(arg_enum)]
        button: Button,

        /// The primary button colour [RRGGBB]
        colour_one: String,

        /// The secondary button colour [RRGGBB]
        colour_two: Option<String>,
    },

    OffStyle {
        /// The Button to change
        #[clap(arg_enum)]
        button: Button,

        /// How the button should be presented when 'off'
        #[clap(arg_enum)]
        off_style: ButtonColourOffStyle,
    },
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
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum AllFaderCommands {
    /// Set the appearance of Slider Lighting
    Display {
        /// The new display method
        #[clap(arg_enum)]
        display: FaderDisplayStyle,
    },

    /// Set the colour of all GoXLR Faders
    Colour {
        /// Top colour in hex format [RRGGBB]
        top: String,

        /// Bottom colour in hex format [RRGGBB]
        bottom: String,
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
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum EffectsCommands {
    LoadEffectPreset {
        name: String,
    },
    RenameActivePreset {
        name: String,
    },
    SaveActivePreset,
    SetActivePreset {
        #[clap(arg_enum)]
        preset: EffectBankPresets,
    },
    Reverb {
        #[clap(subcommand)]
        command: Reverb,
    },
    Echo {
        #[clap(subcommand)]
        command: Echo,
    },
    Pitch {
        #[clap(subcommand)]
        command: Pitch,
    },
    Gender {
        #[clap(subcommand)]
        command: Gender,
    },
    Megaphone {
        #[clap(subcommand)]
        command: Megaphone,
    },
    Robot {
        #[clap(subcommand)]
        command: Robot,
    },
    HardTune {
        #[clap(subcommand)]
        command: HardTune,
    },

    /// Sets the current state of the FX
    Enabled {
        #[clap(parse(try_from_str))]
        enabled: bool,
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum Reverb {
    /// Set the Reverb Style
    Style {
        /// The Style to Set
        #[clap(arg_enum)]
        style: ReverbStyle,
    },

    /// Set the Reverb Amount
    Amount { amount: u8 },

    /// Set the Reverb Decay
    Decay { decay: u16 },

    /// Set the Reverb Early Level
    EarlyLevel { level: i8 },

    /// Set the Reverb Tail Level
    TailLevel { level: i8 },

    /// Set the Reverb Pre-Delay
    PreDelay { delay: u8 },

    /// Set the Reverb Low 'Colour'
    LowColour { colour: i8 },

    /// Set the Reverb High 'Colour'
    HighColour { colour: i8 },

    /// Set the Reverb High Factor
    HighFactor { factor: i8 },

    /// Set the Reverb Diffuse Level
    Diffuse { diffuse: i8 },

    /// Set the Reverb Mod Speed
    ModSpeed { speed: i8 },

    /// Set the Reverb Mod Depth
    ModDepth { depth: i8 },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum Echo {
    /// Set the Echo Style
    Style {
        #[clap(arg_enum)]
        style: EchoStyle,
    },

    /// Set the Echo Amount (Percentage)
    Amount { amount: u8 },

    /// Set the Echo Feedback Level
    Feedback { feedback: u8 },

    /// Set the Echo Tempo (only valid if 'Style' is 'ClassicSlap')
    Tempo { tempo: u16 },

    /// Set the Reverb Left Delay (only valid if 'Style' is not 'ClassicSlap')
    DelayLeft { delay: u16 },

    /// Set the Reverb Right Delay (only valid if 'Style' is not 'ClassicSlap')
    DelayRight { delay: u16 },

    /// Set the Echo XFB from Left to Right
    FeedbackXFBLtoR { feedback: u8 },

    /// Set the Echo XFB from Right to Left
    FeedbackXFBRtoL { feedback: u8 },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum Pitch {
    /// Set the Pitch Style
    Style {
        #[clap(arg_enum)]
        style: PitchStyle,
    },

    /// Set the pitch Amount
    Amount { amount: i8 },

    /// Set the Pitch Character
    Character { character: u8 },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum Gender {
    Style {
        #[clap(arg_enum)]
        style: GenderStyle,
    },
    Amount {
        amount: i8,
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum Megaphone {
    /// Set the Megaphone Style
    Style {
        #[clap(arg_enum)]
        style: MegaphoneStyle,
    },

    /// Set the Megaphone Amount
    Amount { amount: u8 },

    /// Set the Post Processing Gain
    PostGain { gain: i8 },

    /// Sets the State of the Megaphone Button
    Enabled {
        #[clap(parse(try_from_str))]
        enabled: bool,
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum Robot {
    /// Set the Robot Style
    Style {
        #[clap(arg_enum)]
        style: RobotStyle,
    },

    /// Sets the Robot Gain
    Gain {
        /// The Gain Range
        #[clap(arg_enum)]
        range: RobotRange,

        /// The Gain Value
        gain: i8,
    },

    /// Sets the Robot Frequency
    Frequency {
        /// The Frequency Range
        #[clap(arg_enum)]
        range: RobotRange,
        /// The frequency Value
        frequency: u8,
    },
    /// Sets the Robot Bandwidth
    Bandwidth {
        /// The Bandwidth Range
        #[clap(arg_enum)]
        range: RobotRange,
        /// The Bandwidth Value
        bandwidth: u8,
    },
    /// Sets the Robot Waveform
    WaveForm { waveform: u8 },

    /// Sets the Robot Pulse Width
    PulseWidth { width: u8 },

    /// Sets the Robot Activation Threshold
    Threshold { threshold: i8 },

    /// Sets the Robot Dry Mix
    DryMix { dry_mix: i8 },

    /// Sets the Current state of the Robot Button
    Enabled {
        #[clap(parse(try_from_str))]
        enabled: bool,
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]
pub enum HardTune {
    /// Sets the Hard Tune Style
    Style {
        #[clap(arg_enum)]
        style: HardTuneStyle,
    },

    /// Sets the Hard Tune Amount
    Amount { amount: u8 },

    /// Sets the Hard Tune Rate
    Rate { rate: u8 },

    /// Sets the Hard Tune Window
    Window { window: u16 },

    /// Sets the Hard Tune Source
    Source {
        #[clap(arg_enum)]
        source: HardTuneSource,
    },

    /// Sets the current state of the HardTune Button
    Enabled {
        #[clap(parse(try_from_str))]
        enabled: bool,
    },
}

#[derive(Subcommand, Debug)]
#[clap(setting = AppSettings::DeriveDisplayOrder)]
#[clap(setting = AppSettings::ArgRequiredElseHelp)]

/**
Reference Commands:

RemoveSampleByIndex(SampleBank, SampleButtons, usize),
PlaySampleByIndex(SampleBank, SampleButtons, usize),
StopSamplePlayback(SampleBank, SampleButtons),
*/

pub enum SamplerCommands {
    Add {
        #[clap(arg_enum)]
        bank: SampleBank,

        #[clap(arg_enum)]
        button: SampleButtons,

        file: String,
    },

    RemoveByIndex {
        #[clap(arg_enum)]
        bank: SampleBank,

        #[clap(arg_enum)]
        button: SampleButtons,

        index: usize,
    },

    PlayByIndex {
        #[clap(arg_enum)]
        bank: SampleBank,

        #[clap(arg_enum)]
        button: SampleButtons,

        index: usize,
    },

    StopPlayback {
        #[clap(arg_enum)]
        bank: SampleBank,

        #[clap(arg_enum)]
        button: SampleButtons,
    },

    PlaybackMode {
        #[clap(arg_enum)]
        bank: SampleBank,

        #[clap(arg_enum)]
        button: SampleButtons,

        #[clap(arg_enum)]
        mode: SamplePlaybackMode,
    },

    PlaybackOrder {
        #[clap(arg_enum)]
        bank: SampleBank,

        #[clap(arg_enum)]
        button: SampleButtons,

        #[clap(arg_enum)]
        mode: SamplePlayOrder,
    },

    StartPercent {
        #[clap(arg_enum)]
        bank: SampleBank,

        #[clap(arg_enum)]
        button: SampleButtons,

        sample_id: usize,

        #[clap(parse(try_from_str=percent_value_float))]
        start_position: f32,
    },

    StopPercent {
        #[clap(arg_enum)]
        bank: SampleBank,

        #[clap(arg_enum)]
        button: SampleButtons,

        sample_id: usize,

        #[clap(parse(try_from_str=percent_value_float))]
        stop_position: f32,
    },
}
