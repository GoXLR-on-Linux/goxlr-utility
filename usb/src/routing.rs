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
    HardTune,
}

impl OutputDevice {
    pub fn position(&self) -> usize {
        match self {
            OutputDevice::HeadphonesLeft => 1,
            OutputDevice::HeadphonesRight => 3,
            OutputDevice::BroadcastMixLeft => 5,
            OutputDevice::BroadcastMixRight => 7,
            OutputDevice::ChatMicLeft => 9,
            OutputDevice::ChatMicRight => 11,
            OutputDevice::SamplerLeft => 13,
            OutputDevice::SamplerRight => 15,
            OutputDevice::LineOutLeft => 17,
            OutputDevice::LineOutRight => 19,
            OutputDevice::HardTune => 21,
        }
    }

    pub fn from_basic(basic: &BasicOutputDevice) -> (OutputDevice, OutputDevice) {
        match basic {
            BasicOutputDevice::Headphones => {
                (OutputDevice::HeadphonesLeft, OutputDevice::HeadphonesRight)
            }
            BasicOutputDevice::BroadcastMix => (
                OutputDevice::BroadcastMixLeft,
                OutputDevice::BroadcastMixRight,
            ),
            BasicOutputDevice::ChatMic => (OutputDevice::ChatMicLeft, OutputDevice::ChatMicRight),
            BasicOutputDevice::Sampler => (OutputDevice::SamplerLeft, OutputDevice::SamplerRight),
            BasicOutputDevice::LineOut => (OutputDevice::LineOutLeft, OutputDevice::LineOutRight),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
            InputDevice::MicrophoneLeft => 0x02,
            InputDevice::MicrophoneRight => 0x03,
            InputDevice::MusicLeft => 0x0e,
            InputDevice::MusicRight => 0x0f,
            InputDevice::GameLeft => 0x0a,
            InputDevice::GameRight => 0x0b,
            InputDevice::ChatLeft => 0x0c,
            InputDevice::ChatRight => 0x0d,
            InputDevice::ConsoleLeft => 0x06,
            InputDevice::ConsoleRight => 0x07,
            InputDevice::LineInLeft => 0x04,
            InputDevice::LineInRight => 0x05,
            InputDevice::SystemLeft => 0x08,
            InputDevice::SystemRight => 0x09,
            InputDevice::SamplesLeft => 0x10,
            InputDevice::SamplesRight => 0x11,
        }
    }

    pub fn from_basic(basic: &BasicInputDevice) -> (InputDevice, InputDevice) {
        match basic {
            BasicInputDevice::Microphone => {
                (InputDevice::MicrophoneLeft, InputDevice::MicrophoneRight)
            }
            BasicInputDevice::Chat => (InputDevice::ChatLeft, InputDevice::ChatRight),
            BasicInputDevice::Music => (InputDevice::MusicLeft, InputDevice::MusicRight),
            BasicInputDevice::Game => (InputDevice::GameLeft, InputDevice::GameRight),
            BasicInputDevice::Console => (InputDevice::ConsoleLeft, InputDevice::ConsoleRight),
            BasicInputDevice::LineIn => (InputDevice::LineInLeft, InputDevice::LineInRight),
            BasicInputDevice::System => (InputDevice::SystemLeft, InputDevice::SystemRight),
            BasicInputDevice::Samples => (InputDevice::SamplesLeft, InputDevice::SamplesRight),
        }
    }
}
