use enumset::EnumSetType;

pub enum ButtonStates {
    Colour1,
    Colour2,
    DimmedColour1,
    DimmedColour2,
    Flashing,
}

impl ButtonStates {
    pub fn id(&self) -> u8 {
        match self {
            ButtonStates::Colour1 => 0x01,
            ButtonStates::Colour2 => 0x00,
            ButtonStates::DimmedColour1 => 0x02,
            ButtonStates::DimmedColour2 => 0x04,
            ButtonStates::Flashing => 0x03,
        }
    }
}

#[derive(EnumSetType, Debug)]
pub enum Buttons {
    // These are all the buttons from the GoXLR Mini.
    Fader1Mute,
    Fader2Mute,
    Fader3Mute,
    Fader4Mute,
    Bleep,
    MicrophoneMute,

    // The rest are GoXLR Full Buttons. On the mini, they will simply be ignored.
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
