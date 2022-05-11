#[cfg(feature = "clap")]
use clap::ArgEnum;
#[cfg(feature = "enumset")]
use enumset::EnumSetType;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::fmt::Formatter;
use strum::{Display, EnumCount, EnumIter};
use enum_map::Enum;

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

#[derive(Debug, Copy, Clone, Display, EnumIter, EnumCount)]
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
    EchoXFBLtoR = 0x0026,
    EchoFeedbackR = 0x0025,
    EchoXFBRtoL = 0x0027,
    PitchAmount = 0x005d,
    PitchCharacter = 0x0167,
    PitchStyle = 0x0159,
    GenderAmount = 0x0060,
    MegaphoneAmount = 0x003c,
    MegaphonePostGain = 0x0040,
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
    HardTuneAmount = 0x005a,
    HardTuneRate = 0x005c,
    HardTuneWindow = 0x005b,

    RobotEnabled = 0x014e,
    MegaphoneEnabled = 0x00d7,
    HardTuneEnabled = 0x00d8,

    Encoder1Enabled = 0x00d5,
    Encoder2Enabled = 0x00d6,
    Encoder3Enabled = 0x0150,
    Encoder4Enabled = 0x0151
}

#[derive(Debug, Copy, Clone, Display, EnumIter, EnumCount)]
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
