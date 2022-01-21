use goxlr_types::{InputDevice as BasicInputDevice, OutputDevice as BasicOutputDevice};

#[derive(Copy, Clone, Debug)]
pub enum OutputDevice {
    HeadphonesRight,
    HeadphonesLeft,
    BroadcastMixRight,
    BroadcastMixLeft,
    ChatMicRight,
    ChatMicLeft,
    SamplerRight,
    SamplerLeft,
    LineOutRight,
    LineOutLeft,
    Unknown,
}

impl OutputDevice {
    pub fn position(&self) -> usize {
        match self {
            OutputDevice::HeadphonesRight => 1,
            OutputDevice::HeadphonesLeft => 3,
            OutputDevice::BroadcastMixRight => 5,
            OutputDevice::BroadcastMixLeft => 7,
            OutputDevice::ChatMicRight => 9,
            OutputDevice::ChatMicLeft => 11,
            OutputDevice::SamplerRight => 13,
            OutputDevice::SamplerLeft => 15,
            OutputDevice::LineOutRight => 17,
            OutputDevice::LineOutLeft => 19,
            OutputDevice::Unknown => 21,
        }
    }

    pub fn from_basic(basic: &BasicOutputDevice) -> &'static [OutputDevice; 2] {
        match basic {
            BasicOutputDevice::Headphones => {
                &[OutputDevice::HeadphonesLeft, OutputDevice::HeadphonesRight]
            }
            BasicOutputDevice::BroadcastMix => &[
                OutputDevice::BroadcastMixLeft,
                OutputDevice::BroadcastMixRight,
            ],
            BasicOutputDevice::ChatMic => &[OutputDevice::ChatMicLeft, OutputDevice::ChatMicRight],
            BasicOutputDevice::Sampler => &[OutputDevice::SamplerLeft, OutputDevice::SamplerRight],
            BasicOutputDevice::LineOut => &[OutputDevice::LineOutLeft, OutputDevice::LineOutRight],
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum InputDevice {
    MicrophoneRight,
    MicrophoneLeft,
    MusicRight,
    MusicLeft,
    GameRight,
    GameLeft,
    ChatRight,
    ChatLeft,
    ConsoleRight,
    ConsoleLeft,
    LineInRight,
    LineInLeft,
    SystemRight,
    SystemLeft,
    SamplesRight,
    SamplesLeft,
}

impl InputDevice {
    pub fn id(&self) -> u8 {
        match self {
            InputDevice::MicrophoneRight => 0x02,
            InputDevice::MicrophoneLeft => 0x03,
            InputDevice::MusicRight => 0x0e,
            InputDevice::MusicLeft => 0x0f,
            InputDevice::GameRight => 0x0a,
            InputDevice::GameLeft => 0x0b,
            InputDevice::ChatRight => 0x0c,
            InputDevice::ChatLeft => 0x0d,
            InputDevice::ConsoleRight => 0x06,
            InputDevice::ConsoleLeft => 0x07,
            InputDevice::LineInRight => 0x04,
            InputDevice::LineInLeft => 0x05,
            InputDevice::SystemRight => 0x08,
            InputDevice::SystemLeft => 0x09,
            InputDevice::SamplesRight => 0x10,
            InputDevice::SamplesLeft => 0x11,
        }
    }

    pub fn from_basic(basic: &BasicInputDevice) -> &'static [InputDevice; 2] {
        match basic {
            BasicInputDevice::Microphone => {
                &[InputDevice::MicrophoneLeft, InputDevice::MicrophoneRight]
            }
            BasicInputDevice::Chat => &[InputDevice::ChatLeft, InputDevice::ChatRight],
            BasicInputDevice::Music => &[InputDevice::MusicLeft, InputDevice::MusicRight],
            BasicInputDevice::Game => &[InputDevice::GameLeft, InputDevice::GameRight],
            BasicInputDevice::Console => &[InputDevice::ConsoleLeft, InputDevice::ConsoleRight],
            BasicInputDevice::LineIn => &[InputDevice::LineInLeft, InputDevice::LineInRight],
            BasicInputDevice::System => &[InputDevice::SystemLeft, InputDevice::SystemRight],
            BasicInputDevice::Samples => &[InputDevice::SamplesLeft, InputDevice::SamplesRight],
        }
    }
}
