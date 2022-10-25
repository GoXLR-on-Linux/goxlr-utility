use enum_map::Enum;
use strum::{Display, EnumIter, EnumProperty};

pub mod components;
pub mod error;
pub mod mic_profile;
pub mod microphone;
pub mod profile;

#[derive(Debug, Display, Enum, EnumIter, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SampleButtons {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Clear,
}

#[derive(Debug, Enum, EnumIter, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Faders {
    A,
    B,
    C,
    D,
}

#[derive(Debug, EnumIter, Enum, EnumProperty, Copy, Clone)]
pub enum Preset {
    #[strum(props(tagSuffix = "preset1", contextTitle = "effects1"))]
    #[strum(to_string = "PRESET_1")]
    Preset1,

    #[strum(props(tagSuffix = "preset2", contextTitle = "effects2"))]
    #[strum(to_string = "PRESET_2")]
    Preset2,

    #[strum(props(tagSuffix = "preset3", contextTitle = "effects3"))]
    #[strum(to_string = "PRESET_3")]
    Preset3,

    #[strum(props(tagSuffix = "preset4", contextTitle = "effects4"))]
    #[strum(to_string = "PRESET_4")]
    Preset4,

    #[strum(props(tagSuffix = "preset5", contextTitle = "effects5"))]
    #[strum(to_string = "PRESET_5")]
    Preset5,

    #[strum(props(tagSuffix = "preset6", contextTitle = "effects6"))]
    #[strum(to_string = "PRESET_6")]
    Preset6,
}
