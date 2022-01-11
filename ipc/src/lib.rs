use futures::{SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

mod device;
mod socket;

pub use device::*;
use goxlr_types::{ChannelName, FaderName};
pub use socket::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonRequest {
    Ping,
    Command(GoXLRCommand),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonResponse {
    Ok(Option<DeviceStatus>),
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GoXLRCommand {
    GetStatus,
    AssignFader(FaderName, ChannelName),
    SetVolume(ChannelName, u8),
}
