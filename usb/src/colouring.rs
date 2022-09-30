use strum::EnumIter;

/**
 * This can probably be handled a lot better, there's a lot of duplication going on here
 * at this point in the interests of making things work.. Traits and better OO should allow for
 * better building of structures, and definitions. Todo: Later.
 */

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum ColourTargets {
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

    // FX Button labelled as 'fxClear' in config?
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

    // Extras for Colouring:
    FadeMeter1,
    FadeMeter2,
    FadeMeter3,
    FadeMeter4,

    Scribble1,
    Scribble2,
    Scribble3,
    Scribble4,

    PitchEncoder,
    GenderEncoder,
    ReverbEncoder,
    EchoEncoder,

    // LogoX is known as 'Accent' in the Windows UI
    LogoX,
    InternalLight,
}

impl ColourTargets {
    pub fn get_colour_count(&self) -> u8 {
        match self {
            ColourTargets::Scribble1 => 1,
            ColourTargets::Scribble2 => 1,
            ColourTargets::Scribble3 => 1,
            ColourTargets::Scribble4 => 1,
            ColourTargets::PitchEncoder => 3,
            ColourTargets::GenderEncoder => 3,
            ColourTargets::ReverbEncoder => 3,
            ColourTargets::EchoEncoder => 3,
            _ => 2,
        }
    }

    fn get_start(&self) -> usize {
        match self {
            ColourTargets::Fader1Mute => 12,
            ColourTargets::Fader2Mute => 14,
            ColourTargets::Fader3Mute => 16,
            ColourTargets::Fader4Mute => 18,
            ColourTargets::Bleep => 78,
            ColourTargets::MicrophoneMute => 80,
            ColourTargets::EffectSelect1 => 29,
            ColourTargets::EffectSelect2 => 31,
            ColourTargets::EffectSelect3 => 33,
            ColourTargets::EffectSelect4 => 35,
            ColourTargets::EffectSelect5 => 37,
            ColourTargets::EffectSelect6 => 39,
            ColourTargets::EffectFx => 76,
            ColourTargets::EffectMegaphone => 70,
            ColourTargets::EffectRobot => 72,
            ColourTargets::EffectHardTune => 74,
            ColourTargets::SamplerSelectA => 54,
            ColourTargets::SamplerSelectB => 56,
            ColourTargets::SamplerSelectC => 58,
            ColourTargets::SamplerTopLeft => 62,
            ColourTargets::SamplerTopRight => 64,
            ColourTargets::SamplerBottomRight => 66,
            ColourTargets::SamplerBottomLeft => 68,
            ColourTargets::SamplerClear => 60,
            ColourTargets::FadeMeter1 => 20,
            ColourTargets::FadeMeter2 => 22,
            ColourTargets::FadeMeter3 => 24,
            ColourTargets::FadeMeter4 => 26,
            ColourTargets::Scribble1 => 0,
            ColourTargets::Scribble2 => 2,
            ColourTargets::Scribble3 => 4,
            ColourTargets::Scribble4 => 6,
            ColourTargets::PitchEncoder => 41,
            ColourTargets::GenderEncoder => 44,
            ColourTargets::ReverbEncoder => 47,
            ColourTargets::EchoEncoder => 50,
            ColourTargets::LogoX => 10,
            ColourTargets::InternalLight => 8,
        }
    }

    fn get_start_1_3_40(&self) -> usize {
        match self {
            // +48 on everything except Scribble / Mute / FaderMeter / Global / Logo
            ColourTargets::Fader1Mute => 12,
            ColourTargets::Fader2Mute => 14,
            ColourTargets::Fader3Mute => 16,
            ColourTargets::Fader4Mute => 18,
            ColourTargets::Bleep => 126,
            ColourTargets::MicrophoneMute => 128,
            ColourTargets::EffectSelect1 => 77,
            ColourTargets::EffectSelect2 => 79,
            ColourTargets::EffectSelect3 => 81,
            ColourTargets::EffectSelect4 => 83,
            ColourTargets::EffectSelect5 => 85,
            ColourTargets::EffectSelect6 => 87,
            ColourTargets::EffectFx => 124,
            ColourTargets::EffectMegaphone => 118,
            ColourTargets::EffectRobot => 120,
            ColourTargets::EffectHardTune => 122,
            ColourTargets::SamplerSelectA => 102,
            ColourTargets::SamplerSelectB => 104,
            ColourTargets::SamplerSelectC => 106,
            ColourTargets::SamplerTopLeft => 110,
            ColourTargets::SamplerTopRight => 112,
            ColourTargets::SamplerBottomRight => 114,
            ColourTargets::SamplerBottomLeft => 116,
            ColourTargets::SamplerClear => 108,
            ColourTargets::FadeMeter1 => 20,
            ColourTargets::FadeMeter2 => 34,
            ColourTargets::FadeMeter3 => 48,
            ColourTargets::FadeMeter4 => 62,
            ColourTargets::Scribble1 => 0,
            ColourTargets::Scribble2 => 2,
            ColourTargets::Scribble3 => 4,
            ColourTargets::Scribble4 => 6,
            ColourTargets::PitchEncoder => 89,
            ColourTargets::GenderEncoder => 92,
            ColourTargets::ReverbEncoder => 95,
            ColourTargets::EchoEncoder => 98,
            ColourTargets::LogoX => 10,
            ColourTargets::InternalLight => 8,
        }
    }

    pub fn position(&self, colour: u8, format_1_3_40: bool) -> usize {
        // For some odd reason, the Encoder dial order seems to be 1, 0, 2 as the colour definitions
        // where as all other buttons are 0, 1.. We *COULD* make this simpler by assuming that if
        // there are three colours, the order will be different, for now I'm just adding a simple
        // exception for the Encoders.

        // We should also error check here, make sure colour is in the range of get_colour_count..
        let start_point = if format_1_3_40 {
            self.get_start_1_3_40()
        } else {
            self.get_start()
        };

        match self {
            ColourTargets::PitchEncoder => {
                if colour == 0 {
                    return start_point * 4 + 4;
                }
                if colour == 1 {
                    return start_point * 4;
                }

                (start_point * 4) + (colour * 4) as usize
            }

            ColourTargets::GenderEncoder => {
                if colour == 0 {
                    return start_point * 4 + 4;
                }
                if colour == 1 {
                    return start_point * 4;
                }

                // Not sure how matches work, can we just fall this through to the bottom?
                (start_point * 4) + (colour * 4) as usize
            }

            ColourTargets::ReverbEncoder => {
                if colour == 0 {
                    return start_point * 4 + 4;
                }
                if colour == 1 {
                    return start_point * 4;
                }

                (start_point * 4) + (colour * 4) as usize
            }

            ColourTargets::EchoEncoder => {
                if colour == 0 {
                    return start_point * 4 + 4;
                }
                if colour == 1 {
                    return start_point * 4;
                }

                (start_point * 4) + (colour * 4) as usize
            }

            _ => (start_point * 4) + (colour * 4) as usize,
        }
    }
}
