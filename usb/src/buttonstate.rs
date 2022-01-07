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
    Fader1Mute = 4,
    Fader2Mute = 9,
    Fader3Mute = 14,
    Fader4Mute = 19,
    Bleep = 22,
    MicrophoneMute = 23,

    // The rest are GoXLR Full Buttons. On the mini, they will simply be ignored.
    EffectSelect1 = 0,
    EffectSelect2 = 5,
    EffectSelect3 = 11,
    EffectSelect4 = 15,
    EffectSelect5 = 1,
    EffectSelect6 = 6,

    EffectFx = 21,
    EffectMegaphone = 20,
    EffectRobot = 10,
    EffectHardTune = 16,

    SamplerSelectA = 2,
    SamplerSelectB = 7,
    SamplerSelectC = 12,

    SamplerTopLeft = 3,
    SamplerTopRight = 8,
    SamplerBottomLeft = 17,
    SamplerBottomRight = 13,
    SamplerClear = 18,
}
