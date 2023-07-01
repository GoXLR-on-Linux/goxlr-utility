#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AnimationMode {
    RetroRainbow,
    RainbowDark,
    RainbowBright,
    AnimationSimple,
    AnimationRipple,
    AnimationNone,
}

pub enum WaterFallDir {
    Down,
    Up,
    Off,
}
