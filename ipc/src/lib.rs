use futures::{SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

mod device;
mod socket;

pub use device::*;
use goxlr_types::{ChannelName, ColourDisplay, FaderName, InputDevice, MicrophoneType, MuteFunction, OutputDevice};
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
    SetAllFaderColours(String, String),

    SetVolume(ChannelName, u8),
    SetMicrophoneGain(MicrophoneType, u16),
    SetRouter(InputDevice, OutputDevice, bool),

    // Profile Handling..
    ListProfiles(),
    ImportProfile(String),
    LoadProfile(String),
    SaveProfile(),

    ListMicProfiles(),
    ImportMicProfile(String),
    LoadMicProfile(String),
    SaveMicProfile()
}
