#[derive(Copy, Clone, Debug)]
pub enum ChannelState {
    Muted,
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
