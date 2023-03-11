use clap::{ArgAction, Args, Parser, Subcommand};

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
#[command(about, version, author)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// The specific device's serial number to execute commands on.
    /// This field is optional if you have exactly one GoXLR, but required if you have more.
    #[arg(long)]
    pub device: Option<String>,

    /// Display the device information after any subcommands have been executed.
    #[arg(long)]
    pub status: bool,

    /// Display device information as JSON after command..
    #[arg(long)]
    pub status_json: bool,

    #[arg(long)]
    pub status_http: bool,

    /// Use HTTP Instead of IPC. Specify base path as the param (defaults to http://localhost:14564)
    #[arg(long, num_args=0..=1, default_missing_value="http://localhost:14564")]
    pub use_http: Option<String>,

    #[command(flatten, next_help_heading = "Microphone controls")]
    pub microphone_controls: MicrophoneControls,

    #[command(subcommand)]
    pub subcommands: Option<SubCommands>,
}

#[derive(Debug, Args)]
pub struct MicrophoneControls {
    /// Set the gain of the plugged in dynamic (XLR) microphone.
    /// Value is in decibels and recommended to be lower than 72dB.
    #[arg(long)]
    pub dynamic_gain: Option<u16>,

    /// Set the gain of the plugged in condenser (XLR with phantom power) microphone.
    /// Value is in decibels and recommended to be lower than 72dB.
    #[arg(long)]
    pub condenser_gain: Option<u16>,

    /// Set the gain of the plugged in jack (3.5mm) microphone.
    /// Value is in decibels and recommended to be lower than 72dB.
    #[arg(long)]
    pub jack_gain: Option<u16>,
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum SubCommands {
    /// Profile Settings
    Profiles {
        #[command(subcommand)]
        command: ProfileType,
    },

    /// Adjust the microphone settings (Eq, Gate and Compressor)
    Microphone {
        #[command(subcommand)]
        command: MicrophoneCommands,
    },

    /// Adjust Channel Volumes
    Volume {
        /// The Channel To Change
        #[arg(value_enum)]
        channel: ChannelName,

        /// The new volume as a percentage [0 - 100]
        #[arg(value_parser=percent_value)]
        volume_percent: u8,
    },

    /// Configure the Bleep Button
    BleepVolume {
        /// Set Bleep Button Volume
        #[arg(value_parser=percent_value)]
        volume_percent: u8,
    },

    /// Commands to manipulate the individual GoXLR Faders
    Faders {
        #[command(subcommand)]
        fader: FaderCommands,
    },

    /// Commands for configuring the cough button
    CoughButton {
        #[command(subcommand)]
        command: CoughButtonBehaviours,
    },

    /// Commands to manipulate the GoXLR Router
    Router {
        /// The input device
        #[arg(value_enum)]
        input: InputDevice,

        /// The output device
        #[arg(value_enum)]
        output: OutputDevice,

        /// Is routing enabled between these two devices? [true | false]
        #[arg(value_parser, action = ArgAction::Set)]
        enabled: bool,
    },

    /// Commands to control the GoXLR lighting
    Lighting {
        #[command(subcommand)]
        command: LightingCommands,
    },

    /// Commands to Control the Effects Panel
    Effects {
        #[command(subcommand)]
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
#[command(arg_required_else_help = true)]
pub enum CoughButtonBehaviours {
    ButtonIsHold {
        #[arg(value_parser, action = ArgAction::Set)]
        is_hold: bool,
    },

    MuteBehaviour {
        /// Where a single press will mute (Hold will always Mute to All)
        #[arg(value_enum)]
        mute_behaviour: MuteFunction,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum ProfileType {
    /// General Device Profile
    Device {
        #[command(subcommand)]
        command: ProfileAction,
    },

    /// Microphone Profile
    Microphone {
        #[command(subcommand)]
        command: ProfileAction,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
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
    Save,

    /// Save the currently running profile with a new name
    SaveAs {
        /// The new Profile Name
        profile_name: String,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum MicrophoneCommands {
    /// Configure the Equaliser for the Full GoXLR Device
    Equaliser {
        #[command(subcommand)]
        command: EqualiserCommands,
    },

    /// Configure the Equaliser for the GoXLR Mini
    EqualiserMini {
        #[command(subcommand)]
        command: EqualiserMiniCommands,
    },

    /// Configure the microphone noise gate
    NoiseGate {
        #[command(subcommand)]
        command: NoiseGateCommands,
    },

    /// Configure the Microphone Compressor
    Compressor {
        #[command(subcommand)]
        command: CompressorCommands,
    },

    /// Set the DeEss percentage
    DeEss {
        #[arg(value_parser=percent_value)]
        level: u8,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum EqualiserMiniCommands {
    /// Fine tune the Equaliser Frequencies
    Frequency {
        #[arg(value_enum)]
        /// The Frequency to Modify
        frequency: MiniEqFrequencies,

        /// The new Frequency
        value: f32,
    },

    /// Set the Gain Value for frequencies
    Gain {
        #[arg(value_enum)]
        /// The Frequency to modify
        frequency: MiniEqFrequencies,

        #[arg(allow_hyphen_values = true)]
        /// The new Gain Value
        gain: i8,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum EqualiserCommands {
    /// Fine tune the Equaliser Frequencies
    Frequency {
        #[arg(value_enum)]
        /// The Frequency to modify
        frequency: EqFrequencies,

        /// The new frequency
        value: f32,
    },
    Gain {
        #[arg(value_enum)]
        /// The Frequency to Modify
        frequency: EqFrequencies,

        #[arg(allow_hyphen_values = true)]
        /// The new Gain Value
        gain: i8,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum NoiseGateCommands {
    /// Activation Threshold in dB [-59 - 0]
    Threshold {
        #[arg(allow_hyphen_values = true)]
        value: i8,
    },

    /// Attenuation Percentage [0 - 100]
    Attenuation {
        #[arg(value_parser=percent_value)]
        value: u8,
    },

    /// Attack Time
    Attack {
        #[arg(value_enum)]
        value: GateTimes,
    },

    /// Release Time
    Release {
        #[arg(value_enum)]
        value: GateTimes,
    },

    /// Is Gate Active?
    Active {
        #[arg(value_parser, action = ArgAction::Set)]
        enabled: bool,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum CompressorCommands {
    /// Activation Threshold in dB [-24 - 0]
    Threshold {
        #[arg(allow_hyphen_values = true)]
        value: i8,
    },
    Ratio {
        #[arg(value_enum)]
        value: CompressorRatio,
    },
    Attack {
        #[arg(value_enum)]
        value: CompressorAttackTime,
    },
    Release {
        #[arg(value_enum)]
        value: CompressorReleaseTime,
    },
    MakeUp {
        value: i8,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum FaderCommands {
    /// Assign a new Channel to a Fader
    Channel {
        /// The Fader to Change
        #[arg(value_enum)]
        fader: FaderName,

        /// The New Channel Name
        #[arg(value_enum)]
        channel: ChannelName,
    },

    /// Change the behaviour of a Fader Mute Button
    MuteBehaviour {
        /// The Fader to Change
        #[arg(value_enum)]
        fader: FaderName,

        /// Where a single press will mute (Hold will always Mute to All)
        #[arg(value_enum)]
        mute_behaviour: MuteFunction,
    },

    /// Sets the Current Mute State of the Fader
    MuteState {
        /// The Fader to Change
        #[arg(value_enum)]
        fader: FaderName,

        /// The new State
        #[arg(value_enum)]
        state: MuteState,
    },

    Scribbles {
        #[command(subcommand)]
        command: Scribbles,
    },
}
#[derive(Subcommand, Debug)]
pub enum Scribbles {
    Icon {
        #[arg(value_enum)]
        fader: FaderName,
        name: String,
    },

    Text {
        #[arg(value_enum)]
        fader: FaderName,
        text: String,
    },

    Number {
        #[arg(value_enum)]
        fader: FaderName,
        text: String,
    },

    Invert {
        #[arg(value_enum)]
        fader: FaderName,

        #[arg(value_parser, action = ArgAction::Set)]
        inverted: bool,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum CoughCommands {
    /// Change the behaviour of a Fader Mute Button
    MuteBehaviour {
        /// Where a single press will mute (Hold will always Mute to All)
        #[arg(value_enum)]
        mute_behaviour: MuteFunction,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum LightingCommands {
    /// Configure Lighting for a specific fader
    Fader {
        #[command(subcommand)]
        command: FaderLightingCommands,
    },

    /// Configure lighting for all faders at once
    FadersAll {
        #[command(subcommand)]
        command: FadersAllLightingCommands,
    },

    /// Configure lighting for a specific button
    Button {
        #[command(subcommand)]
        command: ButtonLightingCommands,
    },

    /// Configure lighting for a group of common bottoms
    ButtonGroup {
        #[command(subcommand)]
        command: ButtonGroupLightingCommands,
    },

    SimpleColour {
        #[arg(value_enum)]
        target: SimpleColourTargets,
        colour: String,
    },

    EncoderColour {
        /// The Encoder to Change
        #[arg(value_enum)]
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
#[command(arg_required_else_help = true)]
pub enum FaderLightingCommands {
    Display {
        /// The Fader to Change
        #[arg(value_enum)]
        fader: FaderName,

        /// The new display method
        #[arg(value_enum)]
        display: FaderDisplayStyle,
    },

    /// Sets the Top and Bottom colours of a fader
    Colour {
        /// The Fader name to Change
        #[arg(value_enum)]
        fader: FaderName,

        /// Top colour in hex format [RRGGBB]
        top: String,

        /// Bottom colour in hex format [RRGGBB]
        bottom: String,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum FadersAllLightingCommands {
    Display {
        /// The new display method
        #[arg(value_enum)]
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
#[command(arg_required_else_help = true)]
pub enum ButtonLightingCommands {
    Colour {
        /// The Button to change
        #[arg(value_enum)]
        button: Button,

        /// The primary button colour [RRGGBB]
        colour_one: String,

        /// The secondary button colour [RRGGBB]
        colour_two: Option<String>,
    },

    OffStyle {
        /// The Button to change
        #[arg(value_enum)]
        button: Button,

        /// How the button should be presented when 'off'
        #[arg(value_enum)]
        off_style: ButtonColourOffStyle,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum ButtonGroupLightingCommands {
    Colour {
        /// The group to change
        #[arg(value_enum)]
        group: ButtonColourGroups,

        /// The primary button colour [RRGGBB]
        colour_one: String,

        /// The secondary button colour [RRGGBB]
        colour_two: Option<String>,
    },

    OffStyle {
        /// The group to change
        #[arg(value_enum)]
        group: ButtonColourGroups,

        /// How the button should be presented when 'off'
        #[arg(value_enum)]
        off_style: ButtonColourOffStyle,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum AllFaderCommands {
    /// Set the appearance of Slider Lighting
    Display {
        /// The new display method
        #[arg(value_enum)]
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
        #[arg(value_enum)]
        off_style: ButtonColourOffStyle,

        /// The secondary button colour [RRGGBB]
        colour_two: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum EffectsCommands {
    LoadEffectPreset {
        name: String,
    },
    RenameActivePreset {
        name: String,
    },
    SaveActivePreset,
    SetActivePreset {
        #[arg(value_enum)]
        preset: EffectBankPresets,
    },
    Reverb {
        #[command(subcommand)]
        command: Reverb,
    },
    Echo {
        #[command(subcommand)]
        command: Echo,
    },
    Pitch {
        #[command(subcommand)]
        command: Pitch,
    },
    Gender {
        #[command(subcommand)]
        command: Gender,
    },
    Megaphone {
        #[command(subcommand)]
        command: Megaphone,
    },
    Robot {
        #[command(subcommand)]
        command: Robot,
    },
    HardTune {
        #[command(subcommand)]
        command: HardTune,
    },

    /// Sets the current state of the FX
    Enabled {
        #[arg(value_parser, action = ArgAction::Set)]
        enabled: bool,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum Reverb {
    /// Set the Reverb Style
    Style {
        /// The Style to Set
        #[arg(value_enum)]
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
#[command(arg_required_else_help = true)]
pub enum Echo {
    /// Set the Echo Style
    Style {
        #[arg(value_enum)]
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
#[command(arg_required_else_help = true)]
pub enum Pitch {
    /// Set the Pitch Style
    Style {
        #[arg(value_enum)]
        style: PitchStyle,
    },

    /// Set the pitch Amount
    Amount { amount: i8 },

    /// Set the Pitch Character
    Character { character: u8 },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum Gender {
    Style {
        #[arg(value_enum)]
        style: GenderStyle,
    },
    Amount {
        amount: i8,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum Megaphone {
    /// Set the Megaphone Style
    Style {
        #[arg(value_enum)]
        style: MegaphoneStyle,
    },

    /// Set the Megaphone Amount
    Amount { amount: u8 },

    /// Set the Post Processing Gain
    PostGain { gain: i8 },

    /// Sets the State of the Megaphone Button
    Enabled {
        #[arg(value_parser, action = ArgAction::Set)]
        enabled: bool,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum Robot {
    /// Set the Robot Style
    Style {
        #[arg(value_enum)]
        style: RobotStyle,
    },

    /// Sets the Robot Gain
    Gain {
        /// The Gain Range
        #[arg(value_enum)]
        range: RobotRange,

        /// The Gain Value
        gain: i8,
    },

    /// Sets the Robot Frequency
    Frequency {
        /// The Frequency Range
        #[arg(value_enum)]
        range: RobotRange,
        /// The frequency Value
        frequency: u8,
    },
    /// Sets the Robot Bandwidth
    Bandwidth {
        /// The Bandwidth Range
        #[arg(value_enum)]
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
        #[arg(value_parser, action = ArgAction::Set)]
        enabled: bool,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum HardTune {
    /// Sets the Hard Tune Style
    Style {
        #[arg(value_enum)]
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
        #[arg(value_enum)]
        source: HardTuneSource,
    },

    /// Sets the current state of the HardTune Button
    Enabled {
        #[arg(value_parser, action = ArgAction::Set)]
        enabled: bool,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum SamplerCommands {
    Add {
        #[arg(value_enum)]
        bank: SampleBank,

        #[arg(value_enum)]
        button: SampleButtons,

        file: String,
    },

    RemoveByIndex {
        #[arg(value_enum)]
        bank: SampleBank,

        #[arg(value_enum)]
        button: SampleButtons,

        index: usize,
    },

    PlayByIndex {
        #[arg(value_enum)]
        bank: SampleBank,

        #[arg(value_enum)]
        button: SampleButtons,

        index: usize,
    },

    PlayNextTrack {
        #[arg(value_enum)]
        bank: SampleBank,

        #[arg(value_enum)]
        button: SampleButtons,
    },

    StopPlayback {
        #[arg(value_enum)]
        bank: SampleBank,

        #[arg(value_enum)]
        button: SampleButtons,
    },

    PlaybackMode {
        #[arg(value_enum)]
        bank: SampleBank,

        #[arg(value_enum)]
        button: SampleButtons,

        #[arg(value_enum)]
        mode: SamplePlaybackMode,
    },

    PlaybackOrder {
        #[arg(value_enum)]
        bank: SampleBank,

        #[arg(value_enum)]
        button: SampleButtons,

        #[arg(value_enum)]
        mode: SamplePlayOrder,
    },

    StartPercent {
        #[arg(value_enum)]
        bank: SampleBank,

        #[arg(value_enum)]
        button: SampleButtons,

        sample_id: usize,

        #[arg(value_parser=percent_value_float)]
        start_position: f32,
    },

    StopPercent {
        #[arg(value_enum)]
        bank: SampleBank,

        #[arg(value_enum)]
        button: SampleButtons,

        sample_id: usize,

        #[arg(value_parser=percent_value_float)]
        stop_position: f32,
    },
}
