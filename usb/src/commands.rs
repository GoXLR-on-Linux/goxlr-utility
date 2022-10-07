use crate::routing::InputDevice;
use goxlr_types::{ChannelName, EncoderName, FaderName};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Command {
    ResetCommandIndex,
    SystemInfo(SystemInfoCommand),
    SetChannelState(ChannelName),
    SetChannelVolume(ChannelName),
    SetEncoderValue(EncoderName),
    SetEncoderMode(EncoderName),
    SetFader(FaderName),
    SetRouting(InputDevice),
    SetButtonStates(),
    SetEffectParameters,
    SetMicrophoneParameters,
    GetMicrophoneLevel,
    SetColourMap(),
    SetFaderDisplayMode(FaderName),
    SetScribble(FaderName),
    GetButtonStates,
    GetHardwareInfo(HardwareInfoCommand),
}

impl Command {
    pub fn command_id(&self) -> u32 {
        match self {
            Command::ResetCommandIndex => 0,
            Command::SystemInfo(sub) => sub.id(),
            Command::SetChannelState(channel) => (0x809 << 12) | *channel as u32,
            Command::SetChannelVolume(channel) => (0x806 << 12) | *channel as u32,
            Command::SetEncoderValue(encoder) => (0x80a << 12) | *encoder as u32,
            Command::SetEncoderMode(encoder) => (0x811 << 12) | *encoder as u32,
            Command::SetFader(fader) => (0x805 << 12) | *fader as u32,
            Command::SetRouting(input_device) => (0x804 << 12) | input_device.id() as u32,
            Command::SetColourMap() => 0x803 << 12,
            Command::SetButtonStates() => 0x808 << 12,
            Command::SetFaderDisplayMode(fader) => (0x814 << 12) | *fader as u32,
            Command::SetScribble(fader) => (0x802 << 12) | *fader as u32,
            Command::GetButtonStates => 0x800 << 12,
            Command::GetHardwareInfo(sub) => (0x80f << 12) | *sub as u32,
            Command::GetMicrophoneLevel => 0x80c << 12,
            Command::SetMicrophoneParameters => 0x80b << 12,
            Command::SetEffectParameters => 0x801 << 12,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SystemInfoCommand {
    FirmwareVersion,
    SupportsDCPCategory,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HardwareInfoCommand {
    FirmwareVersion = 0,
    SerialNumber = 1,
}

impl SystemInfoCommand {
    pub fn id(&self) -> u32 {
        match self {
            SystemInfoCommand::FirmwareVersion => 2,
            SystemInfoCommand::SupportsDCPCategory => 1,
        }
    }
}
