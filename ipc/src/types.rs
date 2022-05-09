use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Serialize, EnumIter, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::ArgEnum))]
pub enum ChannelName {
    Mic,
    Chat,
    Music,
    Game,
    Console,
    LineIn,
    LineOut,
    System,
    Sample,
    Headphones,
    MicMonitor,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::ArgEnum))]
pub enum FaderName {
    A,
    B,
    C,
    D,
}
