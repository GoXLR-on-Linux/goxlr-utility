use enum_map::Enum;

pub mod components;
pub mod microphone;
pub mod error;
pub mod profile;
pub mod mic_profile;


#[derive(Debug, Enum)]
pub enum SampleButtons {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Clear,
}
