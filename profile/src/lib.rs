use enum_map::Enum;

pub mod components;
pub mod error;
pub mod mic_profile;
pub mod microphone;
pub mod profile;

#[derive(Debug, Enum)]
pub enum SampleButtons {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Clear,
}
