use enum_map::Enum;

use crate::profile::Profile;

pub mod components;
pub mod error;
pub mod profile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profile = Profile::load_from_file("test-data/profile.xml")?;
    profile.write("test-data/output.xml")?;

    Ok(())
}

#[derive(Debug, Enum)]
pub enum SampleButtons {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Clear,
}
