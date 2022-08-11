use enum_map::Enum;
use strum::EnumIter;

pub mod components;
pub mod error;
pub mod mic_profile;
pub mod microphone;
pub mod profile;

#[derive(Debug, Enum, EnumIter, Copy, Clone, PartialEq, Eq, Hash)]
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
