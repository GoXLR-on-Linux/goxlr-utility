/*
   This file primarily defines the Layout of the GoXLR so that Aggregate devices can be
   correctly created.
*/

use core_foundation::base::UInt32;
use enum_map::Enum;
use strum::EnumIter;

#[derive(Enum, EnumIter, Debug)]
pub(crate) enum Outputs {
    System,
    Game,
    Chat,
    Music,
    Sample,
}

impl Outputs {
    pub fn get_name(&self) -> String {
        match self {
            Outputs::System => "System",
            Outputs::Game => "Game",
            Outputs::Chat => "Chat",
            Outputs::Music => "Music",
            Outputs::Sample => "Sample",
        }
        .into()
    }

    pub fn get_channels(&self) -> StereoChannels {
        match self {
            Outputs::System => StereoChannels { left: 1, right: 2 },
            Outputs::Game => StereoChannels { left: 3, right: 4 },
            Outputs::Chat => StereoChannels { left: 5, right: 6 },
            Outputs::Music => StereoChannels { left: 7, right: 8 },
            Outputs::Sample => StereoChannels { left: 9, right: 10 },
        }
    }
}

#[derive(Enum, EnumIter, Debug)]
pub(crate) enum Inputs {
    StreamMix,
    ChatMic,
    Sampler,
}

impl Inputs {
    pub fn get_name(&self) -> String {
        match self {
            Inputs::StreamMix => "Stream Mix",
            Inputs::ChatMic => "Chat Mic",
            Inputs::Sampler => "Sampler",
        }
        .into()
    }

    pub fn get_channels(&self) -> StereoChannels {
        match self {
            Inputs::StreamMix => StereoChannels { left: 1, right: 2 },
            Inputs::ChatMic => StereoChannels { left: 3, right: 4 },
            Inputs::Sampler => StereoChannels { left: 5, right: 6 },
        }
    }
}

pub(crate) struct StereoChannels {
    pub(crate) left: UInt32,
    pub(crate) right: UInt32,
}
