#[derive(Copy, Clone, Debug)]
pub enum DCPCategory {
    Peaks,
    Router,
    Mixer,
    NVM,
}

impl DCPCategory {
    pub fn id(&self) -> u16 {
        match self {
            DCPCategory::Peaks => 1,
            DCPCategory::Router => 2,
            DCPCategory::Mixer => 3,
            DCPCategory::NVM => 4,
        }
    }
}
