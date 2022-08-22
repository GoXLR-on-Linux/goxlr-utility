use enumset::EnumSet;
use goxlr_types::{
    ButtonColourOffStyle, ButtonColourTargets, ChannelName, CompressorAttackTime, CompressorRatio,
    CompressorReleaseTime, EchoStyle, EffectBankPresets, EqFrequencies, FaderDisplayStyle,
    FaderName, FirmwareVersions, GateTimes, GenderStyle, HardTuneSource, HardTuneStyle,
    InputDevice, MegaphoneStyle, MicrophoneType, MiniEqFrequencies, MuteFunction, OutputDevice,
    PitchStyle, ReverbStyle, RobotStyle,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use strum::EnumCount;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub mixers: HashMap<String, MixerStatus>,
    pub paths: Paths,
    pub files: Files,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerStatus {
    pub hardware: HardwareStatus,
    pub fader_status: [FaderStatus; 4],
    pub mic_status: MicSettings,
    pub levels: Levels,
    pub router: [EnumSet<OutputDevice>; InputDevice::COUNT],
    pub router_table: [[bool; OutputDevice::COUNT]; InputDevice::COUNT],
    pub cough_button: CoughButton,
    pub lighting: Lighting,
    pub effects: Option<Effects>,
    pub profile_name: String,
    pub mic_profile_name: String,
}

impl MixerStatus {
    pub fn get_fader_status(&self, fader: FaderName) -> &FaderStatus {
        &self.fader_status[fader as usize]
    }

    pub fn get_channel_volume(&self, channel: ChannelName) -> u8 {
        self.levels.volumes[channel as usize]
    }

    pub fn set_channel_volume(&mut self, channel: ChannelName, volume: u8) {
        self.levels.volumes[channel as usize] = volume;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareStatus {
    pub versions: FirmwareVersions,
    pub serial_number: String,
    pub manufactured_date: String,
    pub device_type: DeviceType,
    pub usb_device: UsbProductInformation,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
pub struct FaderStatus {
    pub channel: ChannelName,
    pub mute_type: MuteFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
pub struct CoughButton {
    pub is_toggle: bool,
    pub mute_type: MuteFunction,
}

impl Default for FaderStatus {
    fn default() -> Self {
        FaderStatus {
            channel: ChannelName::Mic,
            mute_type: MuteFunction::All,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicSettings {
    pub mic_type: MicrophoneType,
    pub mic_gains: [u16; MicrophoneType::COUNT],

    pub equaliser: Equaliser,
    pub equaliser_mini: EqualiserMini,
    pub noise_gate: NoiseGate,
    pub compressor: Compressor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Levels {
    pub volumes: [u8; ChannelName::COUNT],
    pub bleep: i8,
    pub deess: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Equaliser {
    pub gain: HashMap<EqFrequencies, i8>,
    pub frequency: HashMap<EqFrequencies, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqualiserMini {
    pub gain: HashMap<MiniEqFrequencies, i8>,
    pub frequency: HashMap<MiniEqFrequencies, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseGate {
    pub threshold: i8,
    pub attack: GateTimes,
    pub release: GateTimes,
    pub enabled: bool,
    pub attenuation: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Compressor {
    pub threshold: i8,
    pub ratio: CompressorRatio,
    pub attack: CompressorAttackTime,
    pub release: CompressorReleaseTime,
    pub makeup_gain: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lighting {
    pub faders: HashMap<FaderName, FaderLighting>,
    pub buttons: HashMap<ButtonColourTargets, ButtonLighting>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonLighting {
    pub off_style: ButtonColourOffStyle,
    pub colours: TwoColours,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaderLighting {
    pub style: FaderDisplayStyle,
    pub colours: TwoColours,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoColours {
    pub colour_one: String,
    pub colour_two: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Effects {
    pub active_preset: EffectBankPresets,
    pub preset_names: HashMap<EffectBankPresets, String>,
    pub current: ActiveEffects,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveEffects {
    pub reverb: Reverb,
    pub echo: Echo,
    pub pitch: Pitch,
    pub gender: Gender,
    pub megaphone: Megaphone,
    pub robot: Robot,
    pub hard_tune: HardTune,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reverb {
    pub style: ReverbStyle,
    pub amount: u8,
    pub decay: u16,
    pub early_level: i8,
    pub tail_level: i8,
    pub pre_delay: u8,
    pub lo_colour: i8,
    pub hi_colour: i8,
    pub hi_factor: i8,
    pub diffuse: i8,
    pub mod_speed: i8,
    pub mod_depth: i8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Echo {
    pub style: EchoStyle,
    pub amount: u8,
    pub feedback: u8,
    pub tempo: u16,
    pub delay_left: u16,
    pub delay_right: u16,
    pub feedback_left: u8,
    pub feedback_right: u8,
    pub feedback_xfb_l_to_r: u8,
    pub feedback_xfb_r_to_l: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pitch {
    pub style: PitchStyle,
    pub amount: i8,
    pub character: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gender {
    pub style: GenderStyle,
    pub amount: i8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Megaphone {
    pub is_enabled: bool,
    pub style: MegaphoneStyle,
    pub amount: u8,
    pub post_gain: i8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Robot {
    pub is_enabled: bool,
    pub style: RobotStyle,
    pub low_gain: i8,
    pub low_freq: u8,
    pub low_width: u8,
    pub mid_gain: i8,
    pub mid_freq: u8,
    pub mid_width: u8,
    pub high_gain: i8,
    pub high_freq: u8,
    pub high_width: u8,
    pub waveform: u8,
    pub pulse_width: u8,
    pub threshold: i8,
    pub dry_mix: i8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardTune {
    pub is_enabled: bool,
    pub style: HardTuneStyle,
    pub amount: u8,
    pub rate: u8,
    pub window: u16,
    pub source: HardTuneSource,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Paths {
    pub profile_directory: PathBuf,
    pub mic_profile_directory: PathBuf,
    pub samples_directory: PathBuf,
    pub presets_directory: PathBuf,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Files {
    pub profiles: HashSet<String>,
    pub mic_profiles: HashSet<String>,
    pub presets: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbProductInformation {
    pub manufacturer_name: String,
    pub product_name: String,
    pub version: (u8, u8, u8),
    pub is_claimed: bool,
    pub has_kernel_driver_attached: bool,
    pub bus_number: u8,
    pub address: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceType {
    Unknown,
    Full,
    Mini,
}

impl Default for DeviceType {
    fn default() -> Self {
        DeviceType::Unknown
    }
}
