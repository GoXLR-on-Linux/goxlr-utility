use crate::channels::Channel;
use crate::faders::Fader;

#[derive(Copy, Clone, Debug)]
pub enum Command {
    SystemInfo(SystemInfoCommand),
    SetChannelState(Channel),
    SetChannelVolume(Channel),
    SetFader(Fader),
    SetRouting(Channel),
}

impl Command {
    pub fn command_id(&self) -> u32 {
        match self {
            Command::SystemInfo(sub) => sub.id(),
            Command::SetChannelState(channel) => (0x809 << 12) | channel.id() as u32,
            Command::SetChannelVolume(channel) => (0x806 << 12) | channel.id() as u32,
            Command::SetFader(fader) => (0x805 << 12) | fader.id(),
            Command::SetRouting(channel) => (0x804 << 12) | channel.id() as u32,
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
