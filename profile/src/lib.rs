use enum_map::Enum;

pub mod components;
pub mod error;
pub mod profile;

#[derive(Debug, Enum)]
pub enum SampleButtons {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Clear,
}
