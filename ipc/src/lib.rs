use futures::{SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

pub mod client;
mod device;
mod socket;

pub use device::*;
use goxlr_types::{
    ButtonColourGroups, ButtonColourOffStyle, ButtonColourTargets, ChannelName,
    CompressorAttackTime, CompressorRatio, CompressorReleaseTime, EchoStyle, EffectBankPresets,
    EncoderColourTargets, EqFrequencies, FaderDisplayStyle, FaderName, GateTimes, GenderStyle,
    HardTuneSource, HardTuneStyle, InputDevice, MegaphoneStyle, MicrophoneType, MiniEqFrequencies,
    MuteFunction, OutputDevice, PitchStyle, ReverbStyle, RobotRange, RobotStyle,
    SimpleColourTargets,
};
pub use socket::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonRequest {
    Ping,
    GetStatus,
    InvalidateCaches,
    OpenPath(PathTypes),
    Command(String, GoXLRCommand),
}

// TODO: Check this..
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum DaemonResponse {
    Ok,
    Error(String),
    Status(DaemonStatus),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PathTypes {
    Profiles,
    MicProfiles,
    Presets,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GoXLRCommand {
    SetFader(FaderName, ChannelName),
    SetFaderMuteFunction(FaderName, MuteFunction),

    SetVolume(ChannelName, u8),
    SetMicrophoneType(MicrophoneType),
    SetMicrophoneGain(MicrophoneType, u16),
    SetRouter(InputDevice, OutputDevice, bool),

    // Cough Button
    SetCoughMuteFunction(MuteFunction),
    SetCoughIsHold(bool),

    // Bleep Button
    SetSwearButtonVolume(i8),

    // EQ Settings
    SetEqMiniGain(MiniEqFrequencies, i8),
    SetEqMiniFreq(MiniEqFrequencies, f32),
    SetEqGain(EqFrequencies, i8),
    SetEqFreq(EqFrequencies, f32),

    // Gate Settings
    SetGateThreshold(i8),
    SetGateAttenuation(u8),
    SetGateAttack(GateTimes),
    SetGateRelease(GateTimes),
    SetGateActive(bool),

    // Compressor..
    SetCompressorThreshold(i8),
    SetCompressorRatio(CompressorRatio),
    SetCompressorAttack(CompressorAttackTime),
    SetCompressorReleaseTime(CompressorReleaseTime),
    SetCompressorMakeupGain(u8),

    // DeEss
    SetDeeser(u8),

    // Colour Related Settings..
    SetFaderDisplayStyle(FaderName, FaderDisplayStyle),
    SetFaderColours(FaderName, String, String),
    SetAllFaderColours(String, String),
    SetAllFaderDisplayStyle(FaderDisplayStyle),

    SetButtonColours(ButtonColourTargets, String, Option<String>),
    SetButtonOffStyle(ButtonColourTargets, ButtonColourOffStyle),
    SetButtonGroupColours(ButtonColourGroups, String, Option<String>),
    SetButtonGroupOffStyle(ButtonColourGroups, ButtonColourOffStyle),

    SetSimpleColour(SimpleColourTargets, String),
    SetEncoderColour(EncoderColourTargets, String, String, String),
    //SetSampleColour(SamplerColourTargets, String, )

    // Effect Related Settings..
    LoadEffectPreset(String),
    SetActiveEffectPreset(EffectBankPresets),
    RenameActivePreset(String),
    SaveActivePreset(),

    // Reverb
    SetReverbStyle(ReverbStyle),
    SetReverbAmount(u8),
    SetReverbDecay(u16),
    SetReverbEarlyLevel(i8),
    SetReverbTailLevel(i8),
    SetReverbPreDelay(u8),
    SetReverbLowColour(i8),
    SetReverbHighColour(i8),
    SetReverbHighFactor(i8),
    SetReverbDiffuse(i8),
    SetReverbModSpeed(i8),
    SetReverbModDepth(i8),

    // Echo..
    SetEchoStyle(EchoStyle),
    SetEchoAmount(u8),
    SetEchoFeedback(u8),
    SetEchoTempo(u16),
    SetEchoDelayLeft(u16),
    SetEchoDelayRight(u16),
    SetEchoFeedbackLeft(u8),
    SetEchoFeedbackRight(u8),
    SetEchoFeedbackXFBLtoR(u8),
    SetEchoFeedbackXFBRtoL(u8),

    // Pitch
    SetPitchStyle(PitchStyle),
    SetPitchAmount(i8),
    SetPitchCharacter(u8),

    // Gender
    SetGenderStyle(GenderStyle),
    SetGenderAmount(i8),

    // Megaphone
    SetMegaphoneStyle(MegaphoneStyle),
    SetMegaphoneAmount(u8),
    SetMegaphonePostGain(i8),

    // Robot
    SetRobotStyle(RobotStyle),
    SetRobotGain(RobotRange, i8),
    SetRobotFreq(RobotRange, u8),
    SetRobotWidth(RobotRange, u8),
    SetRobotWaveform(u8),
    SetRobotPulseWidth(u8),
    SetRobotThreshold(i8),
    SetRobotDryMix(i8),

    // Hardtune
    SetHardTuneStyle(HardTuneStyle),
    SetHardTuneAmount(u8),
    SetHardTuneRate(u8),
    SetHardTuneWindow(u16),
    SetHardTuneSource(HardTuneSource),

    // Profile Handling..
    NewProfile(String),
    LoadProfile(String),
    LoadProfileColours(String),
    SaveProfile(),
    SaveProfileAs(String),
    DeleteProfile(String),

    NewMicProfile(String),
    LoadMicProfile(String),
    SaveMicProfile(),
    SaveMicProfileAs(String),
    DeleteMicProfile(String),
}
