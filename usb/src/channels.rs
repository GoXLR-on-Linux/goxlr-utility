#[derive(Copy, Clone, Debug)]
pub enum Channel {
    Mic,
    Chat,
    Music,
    Game,
    Console,
    LineIn,
    LineOut,
    System,
    Sample,
    Headphones,
    MicMonitor,
}

impl Channel {
    pub fn id(&self) -> u8 {
        match self {
            Channel::Mic => 0x00,
            Channel::LineIn => 0x01,
            Channel::Console => 0x02,
            Channel::System => 0x03,
            Channel::Game => 0x04,
            Channel::Chat => 0x05,
            Channel::Sample => 0x06,
            Channel::Music => 0x07,
            Channel::Headphones => 0x08,
            Channel::MicMonitor => 0x09,
            Channel::LineOut => 0x0a,
        }
    }
}
