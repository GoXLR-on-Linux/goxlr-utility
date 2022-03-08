use futures::{SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

mod device;
mod socket;

pub use device::*;
use goxlr_types::{ChannelName, FaderName, MicrophoneType};
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
    AssignFader(FaderName, ChannelName),
    SetVolume(ChannelName, u8),
    SetChannelMuted(ChannelName, bool),
    SetMicrophoneGain(MicrophoneType, u16),
    LoadProfile(String),
}
