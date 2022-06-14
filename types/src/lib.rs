#[cfg(feature = "clap")]
use clap::ArgEnum;
#[cfg(feature = "enumset")]
use enumset::EnumSetType;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::fmt::Formatter;
use strum::{Display, EnumCount, EnumIter};
use enum_map::Enum;
use derivative::Derivative;

#[derive(Copy, Clone, Debug, Display, EnumIter, EnumCount, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ChannelName {
    Mic,
    LineIn,
    Console,
    System,
    Game,
    Chat,
    Sample,
    Music,
    Headphones,
    MicMonitor,
    LineOut,
}

#[derive(Copy, Clone, Debug, Display, EnumIter, EnumCount, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FaderName {
    A,
    B,
    C,
    D,
}

#[derive(Copy, Clone, Debug, Display, EnumIter, EnumCount, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum EncoderName {
    Pitch = 0x00,
    Gender = 0x01,
    Reverb = 0x02,
    Echo = 0x03,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FirmwareVersions {
    pub firmware: VersionNumber,
    pub fpga_count: u32,
    pub dice: VersionNumber,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VersionNumber(pub u32, pub u32, pub u32, pub u32);

impl std::fmt::Display for VersionNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0, self.1, self.2, self.3)
    }
}

impl std::fmt::Debug for VersionNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0, self.1, self.2, self.3)
    }
}

#[derive(Debug, Display, Enum, EnumIter, EnumCount)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "enumset", derive(EnumSetType))]
#[cfg_attr(not(feature = "enumset"), derive(Copy, Clone))]
pub enum OutputDevice {
    Headphones,
    BroadcastMix,
    LineOut,
    ChatMic,
    Sampler,
}

#[derive(Debug, Display, Enum, EnumIter, EnumCount)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "enumset", derive(EnumSetType))]
#[cfg_attr(not(feature = "enumset"), derive(Copy, Clone))]
pub enum InputDevice {
    Microphone,
    Chat,
    Music,
    Game,
    Console,
    LineIn,
    System,
    Samples,
}

#[derive(Debug, Eq, Copy, Clone, Display, EnumIter, EnumCount, Derivative)]
#[derivative(PartialEq, Hash)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum EffectKey {
    DisableMic = 0x0158,
    BleepLevel = 0x0073,
    GateMode = 0x0010,
    GateThreshold = 0x0011,
    GateEnabled = 0x0014,
    GateAttenuation = 0x0015,
    GateAttack = 0x0016,
    GateRelease = 0x0017,
    Unknown14b = 0x014b,
    Equalizer31HzFrequency = 0x0126,
    Equalizer31HzGain = 0x0127,
    Equalizer63HzFrequency = 0x00f8,
    Equalizer63HzGain = 0x00f9,
    Equalizer125HzFrequency = 0x0113,
    Equalizer125HzGain = 0x0114,
    Equalizer250HzFrequency = 0x0129,
    Equalizer250HzGain = 0x012a,
    Equalizer500HzFrequency = 0x0116,
    Equalizer500HzGain = 0x0117,
    Equalizer1KHzFrequency = 0x011d,
    Equalizer1KHzGain = 0x011e,
    Equalizer2KHzFrequency = 0x012c,
    Equalizer2KHzGain = 0x012d,
    Equalizer4KHzFrequency = 0x0120,
    Equalizer4KHzGain = 0x0121,
    Equalizer8KHzFrequency = 0x0109,
    Equalizer8KHzGain = 0x010a,
    Equalizer16KHzFrequency = 0x012f,
    Equalizer16KHzGain = 0x0130,
    CompressorThreshold = 0x013d,
    CompressorRatio = 0x013c,
    CompressorAttack = 0x013e,
    CompressorRelease = 0x013f,
    CompressorMakeUpGain = 0x0140,
    DeEsser = 0x000b,
    ReverbAmount = 0x0076,
    ReverbDecay = 0x002f,
    ReverbEarlyLevel = 0x0037,
    ReverbTailLevel = 0x0039,   // Always sent as 0.
    ReverbPredelay = 0x0030,
    ReverbLoColor = 0x0032,
    ReverbHiColor = 0x0033,
    ReverbHiFactor = 0x0034,
    ReverbDiffuse = 0x0031,
    ReverbModSpeed = 0x0035,
    ReverbModDepth = 0x0036,
    ReverbStyle = 0x002e,
    EchoAmount = 0x0075,
    EchoFeedback = 0x0028,
    EchoTempo = 0x001f,
    EchoDelayL = 0x0022,
    EchoDelayR = 0x0023,
    EchoFeedbackL = 0x0024,
    EchoFeedbackR = 0x0025,
    EchoXFBLtoR = 0x0026,
    EchoXFBRtoL = 0x0027,
    EchoSource = 0x001e,
    EchoDivL = 0x0020,
    EchoDivR = 0x0021,
    EchoFilterStyle = 0x002a,
    PitchAmount = 0x005d,
    PitchCharacter = 0x0167,
    PitchThreshold = 0x0159,
    GenderAmount = 0x0060,
    MegaphoneAmount = 0x003c,
    MegaphonePostGain = 0x0040,
    MegaphoneStyle = 0x003a,
    MegaphoneHP = 0x003d,
    MegaphoneLP = 0x003e,
    MegaphonePreGain = 0x003f,
    MegaphoneDistType = 0x0041,
    MegaphonePresenceGain = 0x0042,
    MegaphonePresenceFC = 0x0043,
    MegaphonePresenceBW = 0x0044,
    MegaphoneBeatboxEnable = 0x0045,
    MegaphoneFilterControl = 0x0046,
    MegaphoneFilter = 0x0047,
    MegaphoneDrivePotGainCompMid = 0x0048,
    MegaphoneDrivePotGainCompMax = 0x0049,
    RobotLowGain = 0x0134,
    RobotLowFreq = 0x0133,
    RobotLowWidth = 0x0135,
    RobotMidGain = 0x013a,
    RobotMidFreq = 0x0139,
    RobotMidWidth = 0x013b,
    RobotHiGain = 0x0137,
    RobotHiFreq = 0x0136,
    RobotHiWidth = 0x0138,
    RobotWaveform = 0x0147,
    RobotPulseWidth = 0x0146,
    RobotThreshold = 0x0157,
    RobotDryMix = 0x014d,
    RobotStyle = 0x0000,
    HardTuneKeySource = 0x0059, // Always sent as 0.
    HardTuneAmount = 0x005a,
    HardTuneRate = 0x005c,
    HardTuneWindow = 0x005b,
    HardTuneScale = 0x005e,
    HardTunePitchAmount = 0x005f,

    RobotEnabled = 0x014e,
    MegaphoneEnabled = 0x00d7,
    HardTuneEnabled = 0x00d8,

    Encoder1Enabled = 0x00d5,
    Encoder2Enabled = 0x00d6,
    Encoder3Enabled = 0x0150,
    Encoder4Enabled = 0x0151
}

// Eq and Derivative allow for these to be added to a HashSet (the values make EnumSet unusable)
#[derive(Debug, Copy, Clone, Eq, Display, EnumIter, EnumCount, Derivative)]
#[derivative(PartialEq, Hash)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MicrophoneParamKey {
    MicType = 0x000,
    DynamicGain = 0x001,
    CondenserGain = 0x002,
    JackGain = 0x003,
    GateThreshold = 0x30200,
    GateAttack = 0x30400,
    GateRelease = 0x30600,
    GateAttenuation = 0x30900,
    CompressorThreshold = 0x60200,
    CompressorRatio = 0x60300,
    CompressorAttack = 0x60400,
    CompressorRelease = 0x60600,
    CompressorMakeUpGain = 0x60700,
    BleepLevel = 0x70100,

    /*
      These are the values for the GoXLR mini, it seems there's a difference in how the two
      are setup, The Mini does EQ via mic parameters, where as the full does it via effects.
     */
    Equalizer90HzFrequency = 0x40000,
    Equalizer90HzGain = 0x40001,
    Equalizer250HzFrequency = 0x40003,
    Equalizer250HzGain = 0x40004,
    Equalizer500HzFrequency = 0x40006,
    Equalizer500HzGain = 0x40007,
    Equalizer1KHzFrequency = 0x50000,
    Equalizer1KHzGain = 0x50001,
    Equalizer3KHzFrequency = 0x50003,
    Equalizer3KHzGain = 0x50004,
    Equalizer8KHzFrequency = 0x50006,
    Equalizer8KHzGain = 0x50007,
}

#[derive(Debug, Copy, Clone, Display, EnumIter, EnumCount, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(clap::ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ColourDisplay {
    TwoColour,
    Gradient,
    Meter,
    GradientMeter,
}

#[derive(Debug, Copy, Clone, Display, EnumIter, EnumCount, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(clap::ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ColourOffStyle {
    Dimmed,
    Colour2,
    DimmedColour2,
}

// MuteChat
#[derive(Debug, Copy, Clone, Display, EnumIter, EnumCount, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MuteFunction {
    All,
    ToStream,
    ToVoiceChat,
    ToPhones,
    ToLineOut,
}

#[derive(Debug, Copy, Clone, Display, EnumIter, EnumCount, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MicrophoneType {
    Dynamic,
    Condenser,
    Jack,
}

impl MicrophoneType {
    pub fn get_gain_param(&self) -> MicrophoneParamKey {
        match self {
            MicrophoneType::Dynamic => MicrophoneParamKey::DynamicGain,
            MicrophoneType::Condenser => MicrophoneParamKey::CondenserGain,
            MicrophoneType::Jack => MicrophoneParamKey::JackGain,
        }
    }

    pub fn has_phantom_power(&self) -> bool {
        matches!(self, MicrophoneType::Condenser)
    }
}

#[derive(Debug, Copy, Clone, Display, EnumIter, EnumCount, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum EffectBankPresets {
    Preset1,
    Preset2,
    Preset3,
    Preset4,
    Preset5,
    Preset6
}

#[derive(Debug, Copy, Clone, Display, PartialEq)]
pub enum SampleBank {
    A, B, C
}

/*
 * The following Enums aren't technically passed via IPC, but they're instead used as 'assists'
 * to help map more refined keys to their respective Effect / MicParam keys.
 */

pub enum GateKeys {
    GateThreshold,
    GateAttack,
    GateRelease,
    GateAttenuation,
}

impl GateKeys {
    pub fn to_effect_key(&self) -> EffectKey {
        match self {
            GateKeys::GateThreshold => EffectKey::GateThreshold,
            GateKeys::GateAttack => EffectKey::GateAttack,
            GateKeys::GateRelease => EffectKey::GateRelease,
            GateKeys::GateAttenuation => EffectKey::GateAttenuation
        }
    }

    pub fn to_mic_param(&self) -> MicrophoneParamKey {
        match self {
            GateKeys::GateThreshold => MicrophoneParamKey::GateThreshold,
            GateKeys::GateAttack => MicrophoneParamKey::GateAttack,
            GateKeys::GateRelease => MicrophoneParamKey::GateRelease,
            GateKeys::GateAttenuation => MicrophoneParamKey::GateAttenuation
        }
    }
}

pub enum CompressorKeys {
    CompressorThreshold,
    CompressorRatio,
    CompressorAttack,
    CompressorRelease,
    CompressorMakeUpGain,
}

impl CompressorKeys {
    pub fn to_effect_key(&self) -> EffectKey {
        match self {
            CompressorKeys::CompressorThreshold => EffectKey::CompressorThreshold,
            CompressorKeys::CompressorRatio => EffectKey::CompressorRatio,
            CompressorKeys::CompressorAttack => EffectKey::CompressorAttack,
            CompressorKeys::CompressorRelease => EffectKey::CompressorRatio,
            CompressorKeys::CompressorMakeUpGain => EffectKey::CompressorMakeUpGain
        }
    }

    pub fn to_mic_param(&self) -> MicrophoneParamKey {
        match self {
            CompressorKeys::CompressorThreshold => MicrophoneParamKey::CompressorThreshold,
            CompressorKeys::CompressorRatio => MicrophoneParamKey::CompressorRatio,
            CompressorKeys::CompressorAttack => MicrophoneParamKey::CompressorAttack,
            CompressorKeys::CompressorRelease => MicrophoneParamKey::CompressorRelease,
            CompressorKeys::CompressorMakeUpGain => MicrophoneParamKey::CompressorMakeUpGain
        }
    }
}

pub enum MiniEqGains {
    Equalizer90HzGain,
    Equalizer250HzGain,
    Equalizer500HzGain,
    Equalizer1KHzGain,
    Equalizer3KHzGain,
    Equalizer8KHzGain,
}

impl MiniEqGains {
    pub fn to_mic_param(&self) -> MicrophoneParamKey {
        match self {
            MiniEqGains::Equalizer90HzGain => MicrophoneParamKey::Equalizer90HzGain,
            MiniEqGains::Equalizer250HzGain => MicrophoneParamKey::Equalizer250HzGain,
            MiniEqGains::Equalizer500HzGain => MicrophoneParamKey::Equalizer500HzGain,
            MiniEqGains::Equalizer1KHzGain => MicrophoneParamKey::Equalizer1KHzGain,
            MiniEqGains::Equalizer3KHzGain => MicrophoneParamKey::Equalizer3KHzGain,
            MiniEqGains::Equalizer8KHzGain => MicrophoneParamKey::Equalizer8KHzGain,
        }
    }
}

pub enum MiniEqFrequencies {
    Equalizer90HzFrequency,
    Equalizer250HzFrequency,
    Equalizer500HzFrequency,
    Equalizer1KHzFrequency,
    Equalizer3KHzFrequency,
    Equalizer8KHzFrequency,
}

impl MiniEqFrequencies {
    pub fn to_mic_param(&self) -> MicrophoneParamKey {
        match self {
            MiniEqFrequencies::Equalizer90HzFrequency => MicrophoneParamKey::Equalizer90HzFrequency,
            MiniEqFrequencies::Equalizer250HzFrequency => MicrophoneParamKey::Equalizer250HzFrequency,
            MiniEqFrequencies::Equalizer500HzFrequency => MicrophoneParamKey::Equalizer500HzFrequency,
            MiniEqFrequencies::Equalizer1KHzFrequency => MicrophoneParamKey::Equalizer1KHzFrequency,
            MiniEqFrequencies::Equalizer3KHzFrequency => MicrophoneParamKey::Equalizer3KHzFrequency,
            MiniEqFrequencies::Equalizer8KHzFrequency => MicrophoneParamKey::Equalizer8KHzFrequency
        }
    }
}

pub enum EqGains {
    Equalizer31HzGain,
    Equalizer63HzGain,
    Equalizer125HzGain,
    Equalizer250HzGain,
    Equalizer500HzGain,
    Equalizer1KHzGain,
    Equalizer2KHzGain,
    Equalizer4KHzGain,
    Equalizer8KHzGain,
    Equalizer16KHzGain,
}

impl EqGains {
    pub fn to_effect_key(&self) -> EffectKey {
        match self {
            EqGains::Equalizer31HzGain => EffectKey::Equalizer31HzGain,
            EqGains::Equalizer63HzGain => EffectKey::Equalizer63HzGain,
            EqGains::Equalizer125HzGain => EffectKey::Equalizer125HzGain,
            EqGains::Equalizer250HzGain => EffectKey::Equalizer250HzGain,
            EqGains::Equalizer500HzGain => EffectKey::Equalizer500HzGain,
            EqGains::Equalizer1KHzGain => EffectKey::Equalizer1KHzGain,
            EqGains::Equalizer2KHzGain => EffectKey::Equalizer2KHzGain,
            EqGains::Equalizer4KHzGain => EffectKey::Equalizer4KHzGain,
            EqGains::Equalizer8KHzGain => EffectKey::Equalizer8KHzGain,
            EqGains::Equalizer16KHzGain => EffectKey::Equalizer16KHzGain,
        }
    }
}

pub enum EqFrequencies {
    Equalizer31HzFrequency,
    Equalizer63HzFrequency,
    Equalizer125HzFrequency,
    Equalizer250HzFrequency,
    Equalizer500HzFrequency,
    Equalizer1KHzFrequency,
    Equalizer2KHzFrequency,
    Equalizer4KHzFrequency,
    Equalizer8KHzFrequency,
    Equalizer16KHzFrequency,
}

impl EqFrequencies {
    pub fn to_effect_key(&self) -> EffectKey {
        match self {
            EqFrequencies::Equalizer31HzFrequency => EffectKey::Equalizer31HzFrequency,
            EqFrequencies::Equalizer63HzFrequency => EffectKey::Equalizer63HzFrequency,
            EqFrequencies::Equalizer125HzFrequency => EffectKey::Equalizer125HzFrequency,
            EqFrequencies::Equalizer250HzFrequency => EffectKey::Equalizer250HzFrequency,
            EqFrequencies::Equalizer500HzFrequency => EffectKey::Equalizer500HzFrequency,
            EqFrequencies::Equalizer1KHzFrequency => EffectKey::Equalizer1KHzFrequency,
            EqFrequencies::Equalizer2KHzFrequency => EffectKey::Equalizer2KHzFrequency,
            EqFrequencies::Equalizer4KHzFrequency => EffectKey::Equalizer4KHzFrequency,
            EqFrequencies::Equalizer8KHzFrequency => EffectKey::Equalizer8KHzFrequency,
            EqFrequencies::Equalizer16KHzFrequency => EffectKey::Equalizer16KHzFrequency,
        }
    }
}