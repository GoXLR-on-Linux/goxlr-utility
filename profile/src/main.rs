use enum_map::Enum;

use crate::profile::Profile;

mod components;
mod error;
mod profile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profile = Profile::load("test-data/profile.xml")?;
    profile.write("test-data/output.xml")?;

    Ok(())
}

#[derive(Debug, Enum)]
enum SampleButtons {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Clear,
}
