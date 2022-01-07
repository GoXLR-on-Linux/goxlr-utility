use crate::channels::Channel;
use crate::faders::Fader;
use crate::routing::InputDevice;

#[derive(Copy, Clone, Debug)]
pub enum Command {
    SystemInfo(SystemInfoCommand),
    SetChannelState(Channel),
    SetChannelVolume(Channel),
    SetFader(Fader),
    SetRouting(InputDevice),
    SetButtonStates(),
    SetMicrophoneType(),
    SetColourMap(),
    SetFaderDisplayMode(Fader),
    SetScribble(Fader),
    GetButtonStates,
}

impl Command {
    pub fn command_id(&self) -> u32 {
        match self {
            Command::SystemInfo(sub) => sub.id(),
            Command::SetChannelState(channel) => (0x809 << 12) | channel.id() as u32,
            Command::SetChannelVolume(channel) => (0x806 << 12) | channel.id() as u32,
            Command::SetFader(fader) => (0x805 << 12) | fader.id(),
            Command::SetRouting(input_device) => (0x804 << 12) | input_device.id() as u32,
            Command::SetColourMap() => (0x803 << 12) | 0x00 as u32,
            Command::SetButtonStates() => (0x808 << 12) | 0x00 as u32,
            Command::SetFaderDisplayMode(fader) => (0x814 << 12) | fader.id() as u32,
            Command::SetScribble(fader) => (0x802 << 12) | fader.id() as u32,
            Command::GetButtonStates => (0x800 << 12) | 0x00 as u32,

            // There are multiple versions of this command, we only support one currently..
            Command::SetMicrophoneType() => (0x80b << 12) | 0x00 as u32,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum SystemInfoCommand {
    FirmwareVersion,
    SupportsDCPCategory,
}

impl SystemInfoCommand {
    pub fn id(&self) -> u32 {
        match self {
            SystemInfoCommand::FirmwareVersion => 2,
            SystemInfoCommand::SupportsDCPCategory => 1,
        }
    }
}
