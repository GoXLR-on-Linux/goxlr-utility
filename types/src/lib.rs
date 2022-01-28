#[cfg(feature = "clap")]
use clap::ArgEnum;
#[cfg(feature = "enumset")]
use enumset::EnumSetType;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::fmt::Formatter;
use strum::{Display, EnumCount, EnumIter};

#[derive(Copy, Clone, Debug, Display, EnumIter, EnumCount)]
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

#[derive(Copy, Clone, Debug, Display, EnumIter, EnumCount)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FaderName {
    A,
    B,
    C,
    D,
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

#[derive(Debug, Display, EnumIter, EnumCount)]
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

#[derive(Debug, Display, EnumIter, EnumCount)]
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

#[derive(Debug, Copy, Clone, Display, EnumIter)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum EffectKey {
    GateThreshold = 0x0011,
    GateAttenuation = 0x0015,
    GateAttack = 0x0016,
    GateRelease = 0x0017,
    Equalizer31HzFrequency = 0x0126,
    Equalizer31HzValue = 0x0127,
    Equalizer63HzFrequency = 0x00f8,
    Equalizer63HzValue = 0x00f9,
    Equalizer125HzFrequency = 0x0113,
    Equalizer125HzValue = 0x0114,
    Equalizer250HzFrequency = 0x0129,
    Equalizer250HzValue = 0x012a,
    Equalizer500HzFrequency = 0x0116,
    Equalizer500HzValue = 0x0117,
    Equalizer1KHzFrequency = 0x011d,
    Equalizer1KHzValue = 0x011e,
    Equalizer2KHzFrequency = 0x012c,
    Equalizer2KHzValue = 0x012d,
    Equalizer4KHzFrequency = 0x0120,
    Equalizer4KHzValue = 0x0121,
    Equalizer8KHzFrequency = 0x0109,
    Equalizer8KHzValue = 0x010a,
    Equalizer16KHzFrequency = 0x012f,
    Equalizer16KHzValue = 0x0130,
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
}
