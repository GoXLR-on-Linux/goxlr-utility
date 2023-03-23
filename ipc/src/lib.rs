use json_patch::Patch;
use serde::{Deserialize, Serialize};

pub mod client;
pub mod clients;
mod device;

pub use device::*;
use goxlr_types::{
    Button, ButtonColourGroups, ButtonColourOffStyle, ChannelName, CompressorAttackTime,
    CompressorRatio, CompressorReleaseTime, DisplayMode, DisplayModeComponents, EchoStyle,
    EffectBankPresets, EncoderColourTargets, EqFrequencies, FaderDisplayStyle, FaderName,
    GateTimes, GenderStyle, HardTuneSource, HardTuneStyle, InputDevice, MegaphoneStyle,
    MicrophoneType, MiniEqFrequencies, MuteFunction, MuteState, OutputDevice, PitchStyle,
    ReverbStyle, RobotRange, RobotStyle, SampleBank, SampleButtons, SamplePlayOrder,
    SamplePlaybackMode, SamplerColourTargets, SimpleColourTargets,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonRequest {
    Ping,
    OpenUi,
    GetStatus,
    StopDaemon,
    GetHttpState,
    OpenPath(PathTypes),
    SetShowTrayIcon(bool),
    SetAutoStartEnabled(bool),
    RecoverDefaults(PathTypes),
    Command(String, GoXLRCommand),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonResponse {
    Ok,
    Error(String),
    HttpState(HttpSettings),
    Status(DaemonStatus),
    Patch(Patch),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebsocketRequest {
    pub id: u32,
    pub data: DaemonRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebsocketResponse {
    pub id: u32,
    pub data: DaemonResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum PathTypes {
    Profiles,
    MicProfiles,
    Presets,
    Samples,
    Icons,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GoXLRCommand {
    SetShutdownCommands(Vec<GoXLRCommand>),
    SetSamplerPreBufferDuration(u16),

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
    SetCompressorMakeupGain(i8),

    // Used to switch between display modes..
    SetElementDisplayMode(DisplayModeComponents, DisplayMode),

    // DeEss
    SetDeeser(u8),

    // Colour Related Settings..
    SetFaderDisplayStyle(FaderName, FaderDisplayStyle),
    SetFaderColours(FaderName, String, String),
    SetAllFaderColours(String, String),
    SetAllFaderDisplayStyle(FaderDisplayStyle),

    SetButtonColours(Button, String, Option<String>),
    SetButtonOffStyle(Button, ButtonColourOffStyle),
    SetButtonGroupColours(ButtonColourGroups, String, Option<String>),
    SetButtonGroupOffStyle(ButtonColourGroups, ButtonColourOffStyle),

    SetSimpleColour(SimpleColourTargets, String),
    SetEncoderColour(EncoderColourTargets, String, String, String),
    SetSampleColour(SamplerColourTargets, String, String, String),
    SetSampleOffStyle(SamplerColourTargets, ButtonColourOffStyle),

    // Effect Related Settings..
    LoadEffectPreset(String),
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

    // Sampler..
    SetSamplerFunction(SampleBank, SampleButtons, SamplePlaybackMode),
    SetSamplerOrder(SampleBank, SampleButtons, SamplePlayOrder),
    AddSample(SampleBank, SampleButtons, String),
    SetSampleStartPercent(SampleBank, SampleButtons, usize, f32),
    SetSampleStopPercent(SampleBank, SampleButtons, usize, f32),
    RemoveSampleByIndex(SampleBank, SampleButtons, usize),
    PlaySampleByIndex(SampleBank, SampleButtons, usize),
    PlayNextSample(SampleBank, SampleButtons),
    StopSamplePlayback(SampleBank, SampleButtons),

    // Scribbles
    SetScribbleIcon(FaderName, String),
    SetScribbleText(FaderName, String),
    SetScribbleNumber(FaderName, String),
    SetScribbleInvert(FaderName, bool),

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

    // General Settings
    SetMuteHoldDuration(u16),
    SetVCMuteAlsoMuteCM(bool),

    // These control the current GoXLR 'State'..
    SetActiveEffectPreset(EffectBankPresets),
    SetActiveSamplerBank(SampleBank),
    SetMegaphoneEnabled(bool),
    SetRobotEnabled(bool),
    SetHardTuneEnabled(bool),
    SetFXEnabled(bool),
    SetFaderMuteState(FaderName, MuteState),
    SetCoughMuteState(MuteState),
}
