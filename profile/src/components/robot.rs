use std::collections::HashMap;
use std::io::Write;
use std::os::raw::c_float;

use enum_map::EnumMap;
use strum::{EnumIter, EnumProperty, IntoEnumIterator};

use anyhow::{anyhow, Result};
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::Writer;

use crate::components::colours::ColourMap;
use crate::components::robot::RobotStyle::Robot1;
use crate::profile::Attribute;
use crate::Preset;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),

    #[error("Expected float: {0}")]
    ExpectedFloat(#[from] std::num::ParseFloatError),

    #[error("Expected enum: {0}")]
    ExpectedEnum(#[from] strum::ParseError),

    #[error("Invalid colours: {0}")]
    InvalidColours(#[from] crate::components::colours::ParseError),
}

/**
 * This is relatively static, main tag contains standard colour mapping, subtags contain various
 * presets, we'll use an EnumMap to define the 'presets' as they'll be useful for the other various
 * 'types' of presets (encoders and effects).
 */
#[derive(Debug)]
pub struct RobotEffectBase {
    colour_map: ColourMap,
    preset_map: EnumMap<Preset, RobotEffect>,
}

impl RobotEffectBase {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name;
        Self {
            colour_map: ColourMap::new(colour_map),
            preset_map: EnumMap::default(),
        }
    }

    pub fn parse_robot_root(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            if !self.colour_map.read_colours(attr)? {
                println!("[robotEffect] Unparsed Attribute: {}", attr.name);
            }
        }

        Ok(())
    }

    pub fn parse_robot_preset(
        &mut self,
        preset_enum: Preset,
        attributes: &Vec<Attribute>,
    ) -> Result<()> {
        let mut preset = RobotEffect::new();
        for attr in attributes {
            if attr.name == "robotEffectstate" {
                preset.state = matches!(attr.value.as_str(), "1");
                continue;
            }
            if attr.name == "ROBOT_STYLE" {
                for style in RobotStyle::iter() {
                    if style.get_str("uiIndex").unwrap() == attr.value {
                        preset.style = style;
                        break;
                    }
                }
                continue;
            }

            /* Same as Microphone, I haven't seen any random float values in the config for robot
             * but I'm not gonna rule it out.. */

            if attr.name == "ROBOT_SYNTHOSC_PULSEWIDTH" {
                preset.synthosc_pulse_width = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "ROBOT_SYNTHOSC_WAVEFORM" {
                preset.synthosc_waveform = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "ROBOT_VOCODER_GATE_THRESHOLD" {
                preset.vocoder_gate_threshold = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name == "ROBOT_DRY_MIX" {
                preset.dry_mix = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name == "ROBOT_VOCODER_LOW_FREQ" {
                preset.vocoder_low_freq = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "ROBOT_VOCODER_LOW_GAIN" {
                preset.vocoder_low_gain = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name == "ROBOT_VOCODER_LOW_BW" {
                preset.vocoder_low_bw = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "ROBOT_VOCODER_MID_FREQ" {
                preset.vocoder_mid_freq = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "ROBOT_VOCODER_MID_GAIN" {
                preset.vocoder_mid_gain = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name == "ROBOT_VOCODER_MID_BW" {
                preset.vocoder_mid_bw = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "ROBOT_VOCODER_HIGH_FREQ" {
                preset.vocoder_high_freq = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "ROBOT_VOCODER_HIGH_GAIN" {
                preset.vocoder_high_gain = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name == "ROBOT_VOCODER_HIGH_BW" {
                preset.vocoder_high_bw = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            println!("[RobotEffect] Unparsed Child Attribute: {}", attr.name);
        }

        self.preset_map[preset_enum] = preset;
        Ok(())
    }

    pub fn write_robot<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new("robotEffect");

        let mut attributes: HashMap<String, String> = HashMap::default();
        self.colour_map.write_colours(&mut attributes);

        // Write out the attributes etc for this element, but don't close it yet..
        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Start(elem))?;

        // Because all of these are seemingly 'guaranteed' to exist, we can straight dump..
        for preset in Preset::iter() {
            let tag_name = format!("robotEffect{}", preset.get_str("tagSuffix").unwrap());
            let mut sub_elem = BytesStart::new(tag_name.as_str());

            let sub_attributes = self.get_preset_attributes(preset);
            for (key, value) in &sub_attributes {
                sub_elem.push_attribute((key.as_str(), value.as_str()));
            }

            writer.write_event(Event::Empty(sub_elem))?;
        }

        // Finally, close the 'main' tag.
        writer.write_event(Event::End(BytesEnd::new("robotEffect")))?;
        Ok(())
    }

    pub fn get_preset_attributes(&self, preset: Preset) -> HashMap<String, String> {
        let mut attributes = HashMap::new();
        let value = &self.preset_map[preset];

        attributes.insert(
            "robotEffectstate".to_string(),
            if value.state {
                "1".to_string()
            } else {
                "0".to_string()
            },
        );
        attributes.insert(
            "ROBOT_STYLE".to_string(),
            value.style.get_str("uiIndex").unwrap().to_string(),
        );
        attributes.insert(
            "ROBOT_SYNTHOSC_PULSEWIDTH".to_string(),
            format!("{}", value.synthosc_pulse_width),
        );
        attributes.insert(
            "ROBOT_SYNTHOSC_WAVEFORM".to_string(),
            format!("{}", value.synthosc_waveform),
        );
        attributes.insert(
            "ROBOT_VOCODER_GATE_THRESHOLD".to_string(),
            format!("{}", value.vocoder_gate_threshold),
        );
        attributes.insert("ROBOT_DRY_MIX".to_string(), format!("{}", value.dry_mix));
        attributes.insert(
            "ROBOT_VOCODER_LOW_FREQ".to_string(),
            format!("{}", value.vocoder_low_freq),
        );
        attributes.insert(
            "ROBOT_VOCODER_LOW_GAIN".to_string(),
            format!("{}", value.vocoder_low_gain),
        );
        attributes.insert(
            "ROBOT_VOCODER_LOW_BW".to_string(),
            format!("{}", value.vocoder_low_bw),
        );
        attributes.insert(
            "ROBOT_VOCODER_MID_FREQ".to_string(),
            format!("{}", value.vocoder_mid_freq),
        );
        attributes.insert(
            "ROBOT_VOCODER_MID_GAIN".to_string(),
            format!("{}", value.vocoder_mid_gain),
        );
        attributes.insert(
            "ROBOT_VOCODER_MID_BW".to_string(),
            format!("{}", value.vocoder_mid_bw),
        );
        attributes.insert(
            "ROBOT_VOCODER_HIGH_FREQ".to_string(),
            format!("{}", value.vocoder_high_freq),
        );
        attributes.insert(
            "ROBOT_VOCODER_HIGH_GAIN".to_string(),
            format!("{}", value.vocoder_high_gain),
        );
        attributes.insert(
            "ROBOT_VOCODER_HIGH_BW".to_string(),
            format!("{}", value.vocoder_high_bw),
        );

        attributes
    }

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }
    pub fn colour_map_mut(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }

    pub fn get_preset(&self, preset: Preset) -> &RobotEffect {
        &self.preset_map[preset]
    }
    pub fn get_preset_mut(&mut self, preset: Preset) -> &mut RobotEffect {
        &mut self.preset_map[preset]
    }
}

#[derive(Debug, Default)]
pub struct RobotEffect {
    // State here determines if the robot effect is on or off when this preset is loaded.
    state: bool,

    style: RobotStyle,
    synthosc_pulse_width: u8,
    synthosc_waveform: u8,
    vocoder_gate_threshold: i8,
    dry_mix: i8,

    vocoder_low_freq: u8,
    vocoder_low_gain: i8,
    vocoder_low_bw: u8,

    vocoder_mid_freq: u8,
    vocoder_mid_gain: i8,
    vocoder_mid_bw: u8,

    vocoder_high_freq: u8,
    vocoder_high_gain: i8,
    vocoder_high_bw: u8,
}

impl RobotEffect {
    pub fn new() -> Self {
        Self {
            state: false,
            style: Default::default(),

            synthosc_pulse_width: 0,
            synthosc_waveform: 0,
            vocoder_gate_threshold: 0,
            dry_mix: 0,
            vocoder_low_freq: 0,
            vocoder_low_gain: 0,
            vocoder_low_bw: 0,
            vocoder_mid_freq: 0,
            vocoder_mid_gain: 0,
            vocoder_mid_bw: 0,
            vocoder_high_freq: 0,
            vocoder_high_gain: 0,
            vocoder_high_bw: 0,
        }
    }

    pub fn state(&self) -> bool {
        self.state
    }
    pub fn set_state(&mut self, state: bool) {
        self.state = state;
    }

    pub fn style(&self) -> &RobotStyle {
        &self.style
    }
    pub fn set_style(&mut self, style: RobotStyle) -> Result<()> {
        self.style = style;

        let preset = RobotPresets::get_preset(style);
        self.set_synthosc_pulse_width(preset.synthosc_pulse_width)?;
        self.set_synthosc_waveform(preset.synthosc_waveform)?;
        self.set_vocoder_gate_threshold(preset.vocoder_gate_threshold)?;
        self.set_dry_mix(preset.dry_mix)?;
        self.set_vocoder_low_freq(preset.vocoder_low_freq)?;
        self.set_vocoder_low_gain(preset.vocoder_low_gain)?;
        self.set_vocoder_low_bw(preset.vocoder_low_bw)?;
        self.set_vocoder_mid_freq(preset.vocoder_mid_freq)?;
        self.set_vocoder_mid_gain(preset.vocoder_mid_gain)?;
        self.set_vocoder_mid_bw(preset.vocoder_mid_bw)?;
        self.set_vocoder_high_freq(preset.vocoder_high_freq)?;
        self.set_vocoder_high_gain(preset.vocoder_high_gain)?;
        self.set_vocoder_high_bw(preset.vocoder_high_bw)?;

        Ok(())
    }

    pub fn synthosc_pulse_width(&self) -> u8 {
        self.synthosc_pulse_width
    }
    pub fn set_synthosc_pulse_width(&mut self, value: u8) -> Result<()> {
        if value > 100 {
            return Err(anyhow!("Pulse Width must be a percentage"));
        }
        self.synthosc_pulse_width = value;
        Ok(())
    }

    pub fn synthosc_waveform(&self) -> u8 {
        self.synthosc_waveform
    }
    pub fn set_synthosc_waveform(&mut self, value: u8) -> Result<()> {
        if value > 3 {
            return Err(anyhow!(
                "Waveform must be Sawtooth (0), Square (1) or Triangle (2)"
            ));
        }
        self.synthosc_waveform = value;
        Ok(())
    }

    pub fn vocoder_gate_threshold(&self) -> i8 {
        self.vocoder_gate_threshold
    }
    pub fn set_vocoder_gate_threshold(&mut self, value: i8) -> Result<()> {
        if !(-36..=0).contains(&value) {
            return Err(anyhow!("Threshold must be between -36 and 0"));
        }
        self.vocoder_gate_threshold = value;
        Ok(())
    }

    pub fn dry_mix(&self) -> i8 {
        self.dry_mix
    }
    pub fn set_dry_mix(&mut self, value: i8) -> Result<()> {
        if !(-36..=0).contains(&value) {
            return Err(anyhow!("Dry Mix must be between -36 and 0"));
        }
        self.dry_mix = value;
        Ok(())
    }

    pub fn vocoder_low_freq(&self) -> u8 {
        self.vocoder_low_freq
    }
    pub fn set_vocoder_low_freq(&mut self, value: u8) -> Result<()> {
        if value > 88 {
            return Err(anyhow!("Low Freq should be between 0 and 88"));
        }
        self.vocoder_low_freq = value;
        Ok(())
    }

    pub fn vocoder_low_gain(&self) -> i8 {
        self.vocoder_low_gain
    }
    pub fn set_vocoder_low_gain(&mut self, value: i8) -> Result<()> {
        if !(-12..=12).contains(&value) {
            return Err(anyhow!("Low Gain should be between -12 and 12"));
        }
        self.vocoder_low_gain = value;
        Ok(())
    }

    pub fn vocoder_low_bw(&self) -> u8 {
        self.vocoder_low_bw
    }
    pub fn set_vocoder_low_bw(&mut self, value: u8) -> Result<()> {
        if value > 32 {
            return Err(anyhow!("Low Width should be between 0 and 32"));
        }
        self.vocoder_low_bw = value;
        Ok(())
    }

    pub fn vocoder_mid_freq(&self) -> u8 {
        self.vocoder_mid_freq
    }
    pub fn set_vocoder_mid_freq(&mut self, value: u8) -> Result<()> {
        if !(86..=184).contains(&value) {
            return Err(anyhow!("Mid Freq should be between 86 and 184"));
        }
        self.vocoder_mid_freq = value;
        Ok(())
    }

    pub fn vocoder_mid_gain(&self) -> i8 {
        self.vocoder_mid_gain
    }
    pub fn set_vocoder_mid_gain(&mut self, value: i8) -> Result<()> {
        if !(-12..=12).contains(&value) {
            return Err(anyhow!("Mid Gain should be between -12 and 12"));
        }
        self.vocoder_mid_gain = value;
        Ok(())
    }

    pub fn vocoder_mid_bw(&self) -> u8 {
        self.vocoder_mid_bw
    }
    pub fn set_vocoder_mid_bw(&mut self, value: u8) -> Result<()> {
        if value > 32 {
            return Err(anyhow!("Mid Width should be between 0 and 32"));
        }
        self.vocoder_mid_bw = value;
        Ok(())
    }

    pub fn vocoder_high_freq(&self) -> u8 {
        self.vocoder_high_freq
    }
    pub fn set_vocoder_high_freq(&mut self, value: u8) -> Result<()> {
        if !(182..=240).contains(&value) {
            return Err(anyhow!("High Freq should be between 182 and 240"));
        }
        self.vocoder_high_freq = value;
        Ok(())
    }

    pub fn vocoder_high_gain(&self) -> i8 {
        self.vocoder_high_gain
    }
    pub fn set_vocoder_high_gain(&mut self, value: i8) -> Result<()> {
        if !(-12..=12).contains(&value) {
            return Err(anyhow!("High Gain should be between -12 and 12"));
        }
        self.vocoder_high_gain = value;
        Ok(())
    }

    pub fn vocoder_high_bw(&self) -> u8 {
        self.vocoder_high_bw
    }
    pub fn set_vocoder_high_bw(&mut self, value: u8) -> Result<()> {
        if value > 32 {
            return Err(anyhow!("High Width should be between 0 and 32"));
        }
        self.vocoder_high_bw = value;
        Ok(())
    }
}

#[derive(Default, Debug, EnumIter, EnumProperty, Copy, Clone)]
pub enum RobotStyle {
    #[default]
    #[strum(props(uiIndex = "0"))]
    Robot1,

    #[strum(props(uiIndex = "1"))]
    Robot2,

    #[strum(props(uiIndex = "2"))]
    Robot3,
}

struct RobotPresets {
    synthosc_pulse_width: u8,
    synthosc_waveform: u8,
    vocoder_gate_threshold: i8,
    dry_mix: i8,

    vocoder_low_freq: u8,
    vocoder_low_gain: i8,
    vocoder_low_bw: u8,

    vocoder_mid_freq: u8,
    vocoder_mid_gain: i8,
    vocoder_mid_bw: u8,

    vocoder_high_freq: u8,
    vocoder_high_gain: i8,
    vocoder_high_bw: u8,
}

impl RobotPresets {
    fn get_preset(style: RobotStyle) -> RobotPresets {
        match style {
            Robot1 => RobotPresets {
                synthosc_pulse_width: 50,
                synthosc_waveform: 0,
                vocoder_gate_threshold: -36,
                dry_mix: -6,
                vocoder_low_freq: 88,
                vocoder_low_gain: -10,
                vocoder_low_bw: 0,
                vocoder_mid_freq: 173,
                vocoder_mid_gain: 5,
                vocoder_mid_bw: 32,
                vocoder_high_freq: 182,
                vocoder_high_gain: 0,
                vocoder_high_bw: 0,
            },
            RobotStyle::Robot2 => RobotPresets {
                synthosc_pulse_width: 50,
                synthosc_waveform: 1,
                vocoder_gate_threshold: -36,
                dry_mix: -6,
                vocoder_low_freq: 88,
                vocoder_low_gain: -10,
                vocoder_low_bw: 0,
                vocoder_mid_freq: 173,
                vocoder_mid_gain: 5,
                vocoder_mid_bw: 25,
                vocoder_high_freq: 182,
                vocoder_high_gain: 0,
                vocoder_high_bw: 0,
            },
            RobotStyle::Robot3 => RobotPresets {
                synthosc_pulse_width: 50,
                synthosc_waveform: 2,
                vocoder_gate_threshold: -36,
                dry_mix: -6,
                vocoder_low_freq: 87,
                vocoder_low_gain: 3,
                vocoder_low_bw: 32,
                vocoder_mid_freq: 155,
                vocoder_mid_gain: -2,
                vocoder_mid_bw: 23,
                vocoder_high_freq: 240,
                vocoder_high_gain: 12,
                vocoder_high_bw: 0,
            },
        }
    }
}
