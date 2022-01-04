#[derive(Copy, Clone, Debug)]
pub enum MicrophoneType {
    None,
    Dynamic,
    Phantom,
    Basic,
}

impl MicrophoneType {
    pub fn id(&self) -> u8 {
        match self {
            MicrophoneType::None => 0x00,
            MicrophoneType::Dynamic => 0x01,
            MicrophoneType::Phantom => 0x02,
            MicrophoneType::Basic => 0x03,
        }
    }
}
