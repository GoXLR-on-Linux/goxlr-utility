#[cfg(feature = "clap")]
use clap::ArgEnum;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::fmt::Formatter;
use strum::{Display, EnumCount, EnumIter};

#[derive(Copy, Clone, Debug, Display, EnumIter, EnumCount)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ChannelName {
    Mic,
    LineIn,
    Console,
    System,
    Game,
    Chat,
    Sample,
    Music,
    Headphones,
    MicMonitor,
    LineOut,
}

#[derive(Copy, Clone, Debug, Display, EnumIter, EnumCount)]
#[cfg_attr(feature = "clap", derive(ArgEnum))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FaderName {
    A,
    B,
    C,
    D,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FirmwareVersions {
    pub firmware: VersionNumber,
    pub fpga_count: u32,
    pub dice: VersionNumber,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VersionNumber(pub u32, pub u32, pub u32, pub u32);

impl std::fmt::Display for VersionNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0, self.1, self.2, self.3)
    }
}

impl std::fmt::Debug for VersionNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0, self.1, self.2, self.3)
    }
}
