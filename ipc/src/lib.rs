use futures::{SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

mod device;
mod socket;

pub use device::*;
use goxlr_types::{ChannelName, ColourDisplay, ColourOffStyle, FaderName, InputDevice, MicrophoneType, MuteFunction, OutputDevice};
pub use socket::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonRequest {
    Ping,
    GetStatus,
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
    SetFaderDisplay(FaderName, ColourDisplay),
    SetFaderColours(FaderName, String, String),
    SetFaderButtonColours(FaderName, String, ColourOffStyle, Option<String>),
    SetAllFaderColours(String, String),
    SetAllFaderButtonColours(String, ColourOffStyle, Option<String>),


    SetVolume(ChannelName, u8),
    SetMicrophoneGain(MicrophoneType, u16),
    SetRouter(InputDevice, OutputDevice, bool),

    // Cough Button
    SetCoughMuteFunction(MuteFunction),
    SetCoughColourConfiguration(String, ColourOffStyle, Option<String>),

    // Bleep Button
    SetSwearButtonVolume(i8),
    SetSwearButtonColourConfiguration(String, ColourOffStyle, Option<String>),

    // Profile Handling..
    LoadProfile(String),
    SaveProfile(),
    SaveProfileAs(String),

    LoadMicProfile(String),
    SaveMicProfile(),
    SaveMicProfileAs(String),
}