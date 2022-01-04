pub enum ButtonStates {
    On,
    Off,
    Dimmed,
    Flashing,
}

impl ButtonStates {
    pub fn id(&self) -> u8 {
        match self {
            ButtonStates::On => 0x01,
            ButtonStates::Off => 0x04,
            ButtonStates::Dimmed => 0x02,
            ButtonStates::Flashing => 0x03,
        }
    }
}

pub enum Buttons {
    // These are all the buttons from the GoXLR Mini, I'm not sure how it handles button states
    // or commands, but we may need to do some splitting
    Fader1Mute,
    Fader2Mute,
    Fader3Mute,
    Fader4Mute,
    Bleep,
    MicrophoneMute,

    // The rest are GoXLR Full Buttons
    EffectSelect1,
    EffectSelect2,
    EffectSelect3,
    EffectSelect4,
    EffectSelect5,
    EffectSelect6,

    EffectFx,
    EffectMegaphone,
    EffectRobot,
    EffectHardTune,

    SamplerSelectA,
    SamplerSelectB,
    SamplerSelectC,

    SamplerTopLeft,
    SamplerTopRight,
    SamplerBottomLeft,
    SamplerBottomRight,
    SamplerClear,
}

/**
 * This might be abstractable, I'm only currently aware of this specific order for the full fat
 * GoXLR colour state set command, but it may be used in other commands I haven't seen yet
 */
impl Buttons {
    pub fn position(&self) -> usize {
        match self {
            Buttons::Fader1Mute => 4,
            Buttons::Fader2Mute => 9,
            Buttons::Fader3Mute => 14,
            Buttons::Fader4Mute => 19,
            Buttons::Bleep => 22,
            Buttons::MicrophoneMute => 23,
            Buttons::EffectSelect1 => 0,
            Buttons::EffectSelect2 => 5,
            Buttons::EffectSelect3 => 11,
            Buttons::EffectSelect4 => 15,
            Buttons::EffectSelect5 => 1,
            Buttons::EffectSelect6 => 6,
            Buttons::EffectFx => 21,
            Buttons::EffectMegaphone => 20,
            Buttons::EffectRobot => 10,
            Buttons::EffectHardTune => 16,
            Buttons::SamplerSelectA => 2,
            Buttons::SamplerSelectB => 7,
            Buttons::SamplerSelectC => 12,
            Buttons::SamplerTopLeft => 3,
            Buttons::SamplerTopRight => 8,
            Buttons::SamplerBottomLeft => 17,
            Buttons::SamplerBottomRight => 13,
            Buttons::SamplerClear => 18,
        }
    }
}
