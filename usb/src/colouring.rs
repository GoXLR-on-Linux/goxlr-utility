/**
 * This can probably be handled a lot better, there's a lot of duplication going on here
 * at this point in the interests of making things work.. Traits and better OO should allow for
 * better building of structures, and definitions. Todo: Later.
 */

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

    // I believe this is referred to as 'Global' in the UI
    LogoX1,
    LogoX2,
}

impl ColourTargets {
    pub fn getColourCount(&self) -> u8 {
        match self {
            ColourTargets::Scribble1 => 1,
            ColourTargets::Scribble2 => 1,
            ColourTargets::Scribble3 => 1,
            ColourTargets::Scribble4 => 1,
            ColourTargets::PitchEncoder => 3,
            ColourTargets::GenderEncoder => 3,
            ColourTargets::ReverbEncoder => 3,
            ColourTargets::EchoEncoder => 3,
            _ => 2
        }
    }

    fn getStart(&self) -> usize {
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
            ColourTargets::SamplerBottomLeft => 66,
            ColourTargets::SamplerBottomRight => 68,
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
            ColourTargets::LogoX1 => 8,
            ColourTargets::LogoX2 => 10,
        }
    }

    // There are a few buttons which seem to configure as 00000000 when offStyle is set to
    // 'dimmed', this indicates whether or not that's true for a button..
    pub fn isBlankWhenDimmed(&self) -> bool {
        match self {
            ColourTargets::Fader1Mute => true,
            ColourTargets::Fader2Mute => true,
            ColourTargets::Fader3Mute => true,
            ColourTargets::Fader4Mute => true,
            ColourTargets::Bleep => true,
            ColourTargets::MicrophoneMute => true,
            ColourTargets::EffectSelect1 => true,
            ColourTargets::EffectSelect2 => true,
            ColourTargets::EffectSelect3 => true,
            ColourTargets::EffectSelect4 => true,
            ColourTargets::EffectSelect5 => true,
            ColourTargets::EffectSelect6 => true,
            ColourTargets::EffectFx => true,
            ColourTargets::EffectMegaphone => true,
            ColourTargets::EffectRobot => true,
            ColourTargets::EffectHardTune => true,
            ColourTargets::SamplerSelectA => true,
            ColourTargets::SamplerSelectB => true,
            ColourTargets::SamplerSelectC => true,
            _ => false,
        }
    }

    pub fn position(&self, colour: u8) -> usize {
        // For some odd reason, the Encoder dial order seems to be 1, 0, 2 as the colour definitions
        // where as all other buttons are 0, 1.. We *COULD* make this simpler by assuming that if
        // there are three colours, the order will be different, for now I'm just adding a simple
        // exception for the Encoders.

        // We should also error check here, make sure colour is in the range of getColourCount..


        match self {
            ColourTargets::PitchEncoder => {
                if colour == 0 {
                    return self.getStart() * 4 + 4 as usize
                }
                if colour == 1 {
                    return self.getStart() * 4 as usize
                }

                return self.getStart() + (colour * 4) as usize
            }

            ColourTargets::GenderEncoder => {
                if colour == 0 {
                    return self.getStart() * 4 + 4 as usize
                }
                if colour == 1 {
                    return self.getStart() * 4 as usize
                }

                // Not sure how matches work, can we just fall this through to the bottom?
                self.getStart() + (colour * 4) as usize
            }

            ColourTargets::ReverbEncoder => {
                if colour == 0 {
                    return self.getStart() * 4 + 4 as usize
                }
                if colour == 1 {
                    return self.getStart() * 4 as usize
                }

                return self.getStart() + (colour * 4) as usize
            }

            ColourTargets::EchoEncoder => {
                if colour == 0 {
                    return self.getStart() * 4 + 4 as usize
                }
                if colour == 1 {
                    return self.getStart() * 4 as usize
                }

                self.getStart() + (colour * 4) as usize
            }

            _ => (self.getStart() * 4) + (colour * 4) as usize
        }
    }
}