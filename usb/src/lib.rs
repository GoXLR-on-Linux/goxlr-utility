pub use rusb;
pub mod buttonstate;
pub mod channelstate;
pub mod colouring;
pub mod commands;
pub mod dcp;
pub mod devices;
pub mod error;
pub mod microphone;
pub mod routing;

pub mod device;

pub const VID_GOXLR: u16 = 0x1220;
pub const PID_GOXLR_MINI: u16 = 0x8fe4;
pub const PID_GOXLR_FULL: u16 = 0x8fe0;
