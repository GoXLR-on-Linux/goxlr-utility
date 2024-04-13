use enum_map::Enum;
use strum::{Display, EnumIter, EnumProperty};

pub mod components;
pub mod error;
pub mod mic_profile;
pub mod microphone;
pub mod profile;

#[derive(Debug, Display, Enum, EnumIter, EnumProperty, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SampleButtons {
    #[strum(props(contextTitle = "sampleTopLeft"))]
    TopLeft,

    #[strum(props(contextTitle = "sampleTopRight"))]
    TopRight,

    #[strum(props(contextTitle = "sampleBottomLeft"))]
    BottomLeft,

    #[strum(props(contextTitle = "sampleBottomRight"))]
    BottomRight,

    #[strum(props(contextTitle = "sampleClear"))]
    Clear,
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

#[derive(Debug, Enum, EnumIter, EnumProperty, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Faders {
    #[strum(props(
        faderContext = "FaderMeter0",
        muteContext = "mute1",
        scribbleContext = "scribble1"
    ))]
    A,

    #[strum(props(
        faderContext = "FaderMeter1",
        muteContext = "mute2",
        scribbleContext = "scribble2",
    ))]
    B,

    #[strum(props(
        faderContext = "FaderMeter2",
        muteContext = "mute3",
        scribbleContext = "scribble3",
    ))]
    C,

    #[strum(props(
        faderContext = "FaderMeter3",
        muteContext = "mute4",
        scribbleContext = "scribble4",
    ))]
    D,
}
