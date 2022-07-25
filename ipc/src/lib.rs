use futures::{SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

pub mod client;
mod device;
mod socket;

pub use device::*;
use goxlr_types::{
    ButtonColourGroups, ButtonColourOffStyle, ButtonColourTargets, ChannelName,
    CompressorAttackTime, CompressorRatio, CompressorReleaseTime, EqFrequencies, FaderDisplayStyle,
    FaderName, GateTimes, InputDevice, MicrophoneType, MiniEqFrequencies, MuteFunction,
    OutputDevice,
};
pub use socket::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonRequest {
    Ping,
    GetStatus,
    InvalidateCaches,
    Command(String, GoXLRCommand),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonResponse {
    Ok,
    Error(String),
    Status(DaemonStatus),
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

    // Profile Handling..
    NewProfile(String),
    LoadProfile(String),
    SaveProfile(),
    SaveProfileAs(String),
    DeleteProfile(String),

    NewMicProfile(String),
    LoadMicProfile(String),
    SaveMicProfile(),
    SaveMicProfileAs(String),
    DeleteMicProfile(String),
}
