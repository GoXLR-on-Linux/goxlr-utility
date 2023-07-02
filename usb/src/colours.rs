/**
 * This is primarily a replacement for colour handling, that removes the difficulty of manually
 * building the colour array. Instead this struct can be built, stored, and altered and will
 * produce the correct output.
 *
 * Note, this is not used in the util as of yet (there's simply too much to change), but may
 * be used later.
 */
use byteorder::{ByteOrder, LittleEndian};

const FADER_COUNT: usize = 4;
const MOOD_COUNT: usize = 2;
const PRESET_COUNT: usize = 6;
const ENCODER_COUNT: usize = 4;
const SAMPLE_BANK_COUNT: usize = 3;
const SAMPLE_BUTTON_COUNT: usize = 4;
const FX_BUTTON_COUNT: usize = 4;
const MIC_BUTTON_COUNT: usize = 2;

#[derive(Default)]
struct ColourScheme {
    scribbles: [TwoColour; FADER_COUNT],
    mood: [TwoColour; MOOD_COUNT],
    mutes: [TwoColour; FADER_COUNT],
    faders: [FaderColour; FADER_COUNT],
    dummy1: [Dummy; 1],
    presets: [TwoColour; PRESET_COUNT],
    encoders: [EncoderColour; ENCODER_COUNT],
    dummy2: [Dummy; 1],
    sample_banks: [TwoColour; SAMPLE_BANK_COUNT],
    sample_buttons: [TwoColour; SAMPLE_BUTTON_COUNT],
    fx_buttons: [TwoColour; FX_BUTTON_COUNT],
    mic_buttons: [TwoColour; MIC_BUTTON_COUNT],
}

impl ColourScheme {
    pub fn get_two_colour_target(&mut self, target: TwoColourTargets) -> &mut TwoColour {
        match target {
            TwoColourTargets::Scribble1 => &mut self.scribbles[0],
            TwoColourTargets::Scribble2 => &mut self.scribbles[1],
            TwoColourTargets::Scribble3 => &mut self.scribbles[2],
            TwoColourTargets::Scribble4 => &mut self.scribbles[3],
            TwoColourTargets::InternalLight => &mut self.mood[0],
            TwoColourTargets::LogoX => &mut self.mood[1],
            TwoColourTargets::Fader1Mute => &mut self.mutes[0],
            TwoColourTargets::Fader2Mute => &mut self.mutes[1],
            TwoColourTargets::Fader3Mute => &mut self.mutes[2],
            TwoColourTargets::Fader4Mute => &mut self.mutes[3],
            TwoColourTargets::EffectSelect1 => &mut self.presets[0],
            TwoColourTargets::EffectSelect2 => &mut self.presets[1],
            TwoColourTargets::EffectSelect3 => &mut self.presets[2],
            TwoColourTargets::EffectSelect4 => &mut self.presets[3],
            TwoColourTargets::EffectSelect5 => &mut self.presets[4],
            TwoColourTargets::EffectSelect6 => &mut self.presets[5],
            TwoColourTargets::SamplerSelectA => &mut self.sample_banks[0],
            TwoColourTargets::SamplerSelectB => &mut self.sample_banks[1],
            TwoColourTargets::SamplerSelectC => &mut self.sample_banks[2],
            TwoColourTargets::SamplerClear => &mut self.sample_buttons[0],
            TwoColourTargets::SamplerTopLeft => &mut self.sample_buttons[1],
            TwoColourTargets::SamplerTopRight => &mut self.sample_buttons[2],
            TwoColourTargets::SamplerBottomLeft => &mut self.sample_buttons[3],
            TwoColourTargets::SamplerBottomRight => &mut self.sample_buttons[4],
            TwoColourTargets::EffectMegaphone => &mut self.fx_buttons[0],
            TwoColourTargets::EffectRobot => &mut self.fx_buttons[1],
            TwoColourTargets::EffectHardTune => &mut self.fx_buttons[2],
            TwoColourTargets::EffectFx => &mut self.fx_buttons[3],
            TwoColourTargets::Swear => &mut self.mic_buttons[0],
            TwoColourTargets::CoughButton => &mut self.mic_buttons[1],
        }
    }

    pub fn get_fader_target(&mut self, target: FaderTarget) -> &mut FaderColour {
        match target {
            FaderTarget::FaderA => &mut self.faders[0],
            FaderTarget::FaderB => &mut self.faders[1],
            FaderTarget::FaderC => &mut self.faders[2],
            FaderTarget::FaderD => &mut self.faders[3],
        }
    }

    pub fn get_encoder_target(&mut self, target: EncoderTarget) -> &mut EncoderColour {
        match target {
            EncoderTarget::Pitch => &mut self.encoders[0],
            EncoderTarget::Gender => &mut self.encoders[1],
            EncoderTarget::Reverb => &mut self.encoders[2],
            EncoderTarget::Echo => &mut self.encoders[3],
        }
    }

    pub fn build_colour_map(&mut self, legacy: bool) -> Vec<u8> {
        let mut map = vec![];
        for button in &self.scribbles {
            map.append(&mut button.get_colours_as_bytes());
        }

        for button in &self.mood {
            map.append(&mut button.get_colours_as_bytes());
        }

        for button in &self.mutes {
            map.append(&mut button.get_colours_as_bytes());
        }

        for fader in &self.faders {
            map.append(&mut fader.get_bytes(legacy));
        }

        for dummy in &self.dummy1 {
            map.append(&mut dummy.get_bytes());
        }

        for button in &self.presets {
            map.append(&mut button.get_colours_as_bytes());
        }

        for encoder in &self.encoders {
            map.append(&mut encoder.get_bytes());
        }

        for dummy in &self.dummy2 {
            map.append(&mut dummy.get_bytes());
        }

        for button in &self.sample_banks {
            map.append(&mut button.get_colours_as_bytes());
        }

        for button in &self.sample_buttons {
            map.append(&mut button.get_colours_as_bytes());
        }

        for button in &self.fx_buttons {
            map.append(&mut button.get_colours_as_bytes());
        }

        for button in &self.mic_buttons {
            map.append(&mut button.get_colours_as_bytes());
        }

        map
    }
}

#[derive(Default)]
struct Dummy {}
impl Dummy {
    fn get_bytes(&self) -> Vec<u8> {
        vec![0; 4]
    }
}

#[derive(Default)]
struct FaderColour {
    colours: TwoColour,
}
impl FaderColour {
    fn get_bytes(&self, legacy: bool) -> Vec<u8> {
        let mut result = Vec::new();
        result.append(&mut self.colours.get_colours_as_bytes());

        if !legacy {
            // The Animation Firmware changed faders from 2 colours to 14 colours, from what I
            // can tell, the additional colours do nothing, just fill the remaining bytes with 0
            let mut extension = vec![0; 12 * 4];
            result.append(&mut extension);
        }

        vec![]
    }
}

#[derive(Default)]
struct TwoColour {
    colour1: Colour,
    colour2: Colour,
}
impl TwoColour {
    fn get_colours_as_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();
        result.append(&mut self.colour1.get_colour_as_bytes());
        result.append(&mut self.colour2.get_colour_as_bytes());

        result
    }
}

#[derive(Default)]
struct EncoderColour {
    left: Colour,
    right: Colour,
    knob: Colour,
}

impl EncoderColour {
    fn get_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();
        result.append(&mut self.left.get_colour_as_bytes());
        result.append(&mut self.right.get_colour_as_bytes());
        result.append(&mut self.knob.get_colour_as_bytes());

        result
    }
}

#[derive(Default)]
struct Colour {
    red: u32,
    green: u32,
    blue: u32,
}

impl Colour {
    pub fn pack(&self) -> u32 {
        ((self.red) << 16) | ((self.green) << 8) | (self.blue)
    }

    fn get_colour_as_bytes(&self) -> Vec<u8> {
        let mut value = [0; 4];
        LittleEndian::write_u32(&mut value, self.pack());

        Vec::from(value)
    }
}

enum TwoColourTargets {
    // Scribble Bar first..
    Scribble1,
    Scribble2,
    Scribble3,
    Scribble4,

    // Mood Lighting..
    InternalLight,
    LogoX,

    // Fader Mute Buttons
    Fader1Mute,
    Fader2Mute,
    Fader3Mute,
    Fader4Mute,

    // Effect Presets Selectors
    EffectSelect1,
    EffectSelect2,
    EffectSelect3,
    EffectSelect4,
    EffectSelect5,
    EffectSelect6,

    // Sample Bank Selectors
    SamplerSelectA,
    SamplerSelectB,
    SamplerSelectC,

    // Sample Buttons
    SamplerClear,
    SamplerTopLeft,
    SamplerTopRight,
    SamplerBottomLeft,
    SamplerBottomRight,

    // FX Buttons
    EffectMegaphone,
    EffectRobot,
    EffectHardTune,
    EffectFx,

    // Finally, the Mic Buttons
    Swear,
    CoughButton,
}

enum FaderTarget {
    FaderA,
    FaderB,
    FaderC,
    FaderD,
}

enum EncoderTarget {
    Pitch,
    Gender,
    Reverb,
    Echo,
}
