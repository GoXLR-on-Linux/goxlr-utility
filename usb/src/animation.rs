#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AnimationMode {
    RetroRainbow,
    RainbowDark,
    RainbowBright,
    Simple,
    Ripple,
    None,
}

pub enum WaterFallDir {
    Down,
    Up,
    Off,
}
