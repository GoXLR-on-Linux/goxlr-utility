#[derive(Default, Copy, Clone, Debug, Eq, PartialEq)]
pub enum ChannelState {
    Muted,
    #[default]
    Unmuted,
}

impl ChannelState {
    pub fn id(&self) -> u8 {
        match self {
            ChannelState::Muted => 0x01,
            ChannelState::Unmuted => 0x00,
        }
    }
}
