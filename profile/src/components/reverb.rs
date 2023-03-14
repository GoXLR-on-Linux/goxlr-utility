use std::collections::HashMap;
use std::io::Write;
use std::os::raw::c_float;

use enum_map::{Enum, EnumMap};
use strum::{EnumIter, EnumProperty, IntoEnumIterator};

use anyhow::{anyhow, Result};
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::Writer;

use crate::components::colours::ColourMap;
use crate::components::reverb::ReverbStyle::Library;
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
pub struct ReverbEncoderBase {
    colour_map: ColourMap,
    preset_map: EnumMap<Preset, ReverbEncoder>,
    active_set: u8, // Not sure what this does?
}

impl ReverbEncoderBase {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name;
        Self {
            colour_map: ColourMap::new(colour_map),
            preset_map: EnumMap::default(),
            active_set: 0,
        }
    }

    pub fn parse_reverb_root(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            if attr.name == "active_set" {
                self.active_set = attr.value.parse()?;
                continue;
            }

            if !self.colour_map.read_colours(attr)? {
                println!("[ReverbEncoder] Unparsed Attribute: {}", attr.name);
            }
        }

        Ok(())
    }

    pub fn parse_reverb_preset(
        &mut self,
        preset_enum: Preset,
        attributes: &Vec<Attribute>,
    ) -> Result<()> {
        let mut preset = ReverbEncoder::new();
        for attr in attributes {
            if attr.name == "REVERB_STYLE" {
                for style in ReverbStyle::iter() {
                    if style.get_str("uiIndex").unwrap() == attr.value {
                        preset.style = style;
                        break;
                    }
                }
                continue;
            }

            if attr.name == "REVERB_KNOB_POSITION" {
                preset.set_knob_position(attr.value.parse::<c_float>()? as i8)?;
                continue;
            }

            if attr.name == "REVERB_TYPE" {
                preset.reverb_type = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "REVERB_DECAY" {
                preset.decay = attr.value.parse::<c_float>()? as u16;
                continue;
            }
            if attr.name == "REVERB_PREDELAY" {
                preset.pre_delay = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "REVERB_DIFFUSE" {
                preset.diffuse = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name == "REVERB_LOCOLOR" {
                preset.low_color = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name == "REVERB_HICOLOR" {
                preset.high_color = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name == "REVERB_HIFACTOR" {
                preset.high_factor = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name == "REVERB_MODSPEED" {
                preset.mod_speed = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name == "REVERB_MODDEPTH" {
                preset.mod_depth = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name == "REVERB_EARLYLEVEL" {
                preset.early_level = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name == "REVERB_TAILLEVEL" {
                preset.tail_level = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name == "REVERB_DRYLEVEL" {
                preset.dry_level = attr.value.parse::<c_float>()? as i8;
                continue;
            }

            println!("[ReverbEncoder] Unparsed Child Attribute: {}", &attr.name);
        }

        // Ok, we should be able to store this now..
        self.preset_map[preset_enum] = preset;
        Ok(())
    }

    pub fn write_reverb<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new("reverbEncoder");

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("active_set".to_string(), format!("{}", self.active_set));
        self.colour_map.write_colours(&mut attributes);

        // Write out the attributes etc for this element, but don't close it yet..
        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Start(elem))?;

        // Because all of these are seemingly 'guaranteed' to exist, we can straight dump..
        for preset in Preset::iter() {
            let tag_name = format!("reverbEncoder{}", preset.get_str("tagSuffix").unwrap());
            let mut sub_elem = BytesStart::new(tag_name.as_str());

            let sub_attributes = self.get_preset_attributes(preset);
            for (key, value) in &sub_attributes {
                sub_elem.push_attribute((key.as_str(), value.as_str()));
            }

            writer.write_event(Event::Empty(sub_elem))?;
        }

        // Finally, close the 'main' tag.
        writer.write_event(Event::End(BytesEnd::new("reverbEncoder")))?;
        Ok(())
    }

    pub fn get_preset_attributes(&self, preset: Preset) -> HashMap<String, String> {
        let mut attributes = HashMap::new();
        let value = &self.preset_map[preset];

        attributes.insert(
            "REVERB_KNOB_POSITION".to_string(),
            format!("{}", value.knob_position),
        );
        attributes.insert(
            "REVERB_STYLE".to_string(),
            value.style.get_str("uiIndex").unwrap().to_string(),
        );
        attributes.insert("REVERB_TYPE".to_string(), format!("{}", value.reverb_type));
        attributes.insert("REVERB_DECAY".to_string(), format!("{}", value.decay));
        attributes.insert(
            "REVERB_PREDELAY".to_string(),
            format!("{}", value.pre_delay),
        );
        attributes.insert("REVERB_DIFFUSE".to_string(), format!("{}", value.diffuse));
        attributes.insert("REVERB_LOCOLOR".to_string(), format!("{}", value.low_color));
        attributes.insert(
            "REVERB_HICOLOR".to_string(),
            format!("{}", value.high_color),
        );
        attributes.insert(
            "REVERB_HIFACTOR".to_string(),
            format!("{}", value.high_factor),
        );
        attributes.insert(
            "REVERB_MODSPEED".to_string(),
            format!("{}", value.mod_speed),
        );
        attributes.insert(
            "REVERB_MODDEPTH".to_string(),
            format!("{}", value.mod_depth),
        );
        attributes.insert(
            "REVERB_EARLYLEVEL".to_string(),
            format!("{}", value.early_level),
        );
        attributes.insert(
            "REVERB_TAILLEVEL".to_string(),
            format!("{}", value.tail_level),
        );
        attributes.insert(
            "REVERB_DRYLEVEL".to_string(),
            format!("{}", value.dry_level),
        );

        attributes
    }

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }
    pub fn colour_map_mut(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }

    pub fn get_preset(&self, preset: Preset) -> &ReverbEncoder {
        &self.preset_map[preset]
    }
    pub fn get_preset_mut(&mut self, preset: Preset) -> &mut ReverbEncoder {
        &mut self.preset_map[preset]
    }
}

#[derive(Debug, Default)]
pub struct ReverbEncoder {
    knob_position: i8,
    style: ReverbStyle,
    reverb_type: u8,
    decay: u16, // Reaches 290 when set to max.
    pre_delay: u8,
    diffuse: i8,
    low_color: i8,
    high_color: i8,
    high_factor: i8,
    mod_speed: i8,
    mod_depth: i8,
    early_level: i8,
    tail_level: i8,
    dry_level: i8, // Dry level exists in the config, but is never sent?
}

impl ReverbEncoder {
    pub fn new() -> Self {
        Self {
            knob_position: 0,
            style: Library,
            reverb_type: 0,
            decay: 0,
            pre_delay: 0,
            diffuse: 0,
            low_color: 0,
            high_color: 0,
            high_factor: 0,
            mod_speed: 0,
            mod_depth: 0,
            early_level: 0,
            tail_level: 0,
            dry_level: 0,
        }
    }

    pub fn amount(&self) -> i8 {
        ((36 * self.knob_position as i32) / 24 - 36) as i8
    }

    // TODO: As with echo, we probably shouldn't do this!
    pub fn get_percentage_amount(&self) -> u8 {
        // Knob Position and Amount are two very different things, so is percentage :)
        ((self.knob_position as u16 * 100) / 24) as u8
    }
    pub fn set_percentage_amount(&mut self, percentage: u8) -> Result<()> {
        if percentage > 100 {
            return Err(anyhow!("Value must be a percentage"));
        }
        self.set_knob_position(((percentage as i16 * 24) / 100) as i8)?;
        Ok(())
    }

    pub fn knob_position(&self) -> i8 {
        self.knob_position
    }
    pub fn set_knob_position(&mut self, knob_position: i8) -> Result<()> {
        if !(0..=24).contains(&knob_position) {
            return Err(anyhow!("Reverb Knob Position should be between 0 and 24"));
        }

        self.knob_position = knob_position;
        Ok(())
    }

    pub fn style(&self) -> &ReverbStyle {
        &self.style
    }
    pub fn set_style(&mut self, style: ReverbStyle) -> Result<()> {
        self.style = style;

        let preset = ReverbPreset::get_preset(style);
        self.set_reverb_type(preset.reverb_type);
        self.set_decay(preset.decay);
        self.set_predelay(preset.pre_delay)?;
        self.set_diffuse(preset.diffuse)?;
        self.set_low_color(preset.low_color)?;
        self.set_hi_color(preset.high_color)?;
        self.set_hi_factor(preset.high_factor)?;
        self.set_mod_speed(preset.mod_speed)?;
        self.set_mod_depth(preset.mod_depth)?;
        self.set_early_level(preset.early_level)?;
        self.set_tail_level(preset.tail_level)?;

        Ok(())
    }

    pub fn reverb_type(&self) -> u8 {
        self.reverb_type
    }
    fn set_reverb_type(&mut self, value: u8) {
        self.reverb_type = value;
    }

    pub fn decay(&self) -> u16 {
        self.decay
    }
    fn set_decay(&mut self, value: u16) {
        self.decay = value;
    }

    pub fn get_decay_millis(&self) -> u16 {
        let decay = self.decay;
        if decay <= 100 {
            return decay * 10;
        }

        let base = 1000;
        let current = decay - 100;
        let addition = current * 100;
        base + addition
    }
    pub fn set_decay_millis(&mut self, milliseconds: u16) -> Result<()> {
        // We're going to handle the conversion here directly..
        if !(10..=20000).contains(&milliseconds) {
            return Err(anyhow!("Decay should be between 10 and 20000 milliseconds"));
        }

        // For everything below 1000ms, the division is ms / 10..
        if milliseconds <= 1000 {
            self.decay = milliseconds / 10;
            return Ok(());
        }

        // Once we pass 1000, all additions are value / 100
        let base = 100;

        // Remove the first second from the value..
        let current = milliseconds - 1000;

        // Divide anything remaining by 100..
        let addition = current / 100;

        // Add it onto the 100 base..
        self.decay = base + addition;

        // And done?
        Ok(())
    }

    pub fn predelay(&self) -> u8 {
        self.pre_delay
    }
    pub fn set_predelay(&mut self, value: u8) -> Result<()> {
        if value > 100 {
            return Err(anyhow!("Predelay must be between 0 and 100ms"));
        }
        self.pre_delay = value;
        Ok(())
    }

    pub fn diffuse(&self) -> i8 {
        self.diffuse
    }
    pub fn set_diffuse(&mut self, value: i8) -> Result<()> {
        if !(-50..=50).contains(&value) {
            return Err(anyhow!("Diffuse should be between -50 and 50"));
        }
        self.diffuse = value;
        Ok(())
    }

    pub fn low_color(&self) -> i8 {
        self.low_color
    }
    pub fn set_low_color(&mut self, value: i8) -> Result<()> {
        if !(-50..=50).contains(&value) {
            return Err(anyhow!("LoColour should be between -50 and 50"));
        }
        self.low_color = value;
        Ok(())
    }

    pub fn high_color(&self) -> i8 {
        self.high_color
    }
    pub fn set_hi_color(&mut self, value: i8) -> Result<()> {
        if !(-50..=50).contains(&value) {
            return Err(anyhow!("HiColour should be between -50 and 50"));
        }
        self.high_color = value;
        Ok(())
    }

    pub fn hifactor(&self) -> i8 {
        self.high_factor
    }
    pub fn set_hi_factor(&mut self, value: i8) -> Result<()> {
        if !(-25..=25).contains(&value) {
            return Err(anyhow!("HiFactor should be between -25 and 25"));
        }
        self.high_factor = value;
        Ok(())
    }

    pub fn mod_speed(&self) -> i8 {
        self.mod_speed
    }
    pub fn set_mod_speed(&mut self, value: i8) -> Result<()> {
        if !(-25..=25).contains(&value) {
            return Err(anyhow!("Mod Speed should be between -25 and 25"));
        }
        self.mod_speed = value;
        Ok(())
    }

    pub fn mod_depth(&self) -> i8 {
        self.mod_depth
    }
    pub fn set_mod_depth(&mut self, value: i8) -> Result<()> {
        if !(-25..=25).contains(&value) {
            return Err(anyhow!("Mod Depth should be between -25 and 25"));
        }
        self.mod_depth = value;
        Ok(())
    }

    pub fn early_level(&self) -> i8 {
        self.early_level
    }
    pub fn set_early_level(&mut self, value: i8) -> Result<()> {
        if !(-25..=0).contains(&value) {
            return Err(anyhow!("Early Level should be between -25 and 0"));
        }
        self.early_level = value;
        Ok(())
    }

    pub fn tail_level(&self) -> i8 {
        // This value is never actually sent to the GoXLR, but is stored in config.
        self.tail_level
    }
    pub fn set_tail_level(&mut self, value: i8) -> Result<()> {
        if !(-25..=0).contains(&value) {
            return Err(anyhow!("Tail Level should be between -25 and 0"));
        }
        self.tail_level = value;
        Ok(())
    }

    pub fn dry_level(&self) -> i8 {
        self.dry_level
    }
}

#[derive(Default, Debug, EnumIter, Enum, EnumProperty, Copy, Clone)]
pub enum ReverbStyle {
    #[default]
    #[strum(props(uiIndex = "0"))]
    Library,

    #[strum(props(uiIndex = "1"))]
    DarkBloom,

    #[strum(props(uiIndex = "2"))]
    MusicClub,

    #[strum(props(uiIndex = "3"))]
    RealPlate,

    #[strum(props(uiIndex = "4"))]
    Chapel,

    #[strum(props(uiIndex = "5"))]
    HockeyArena,
}

struct ReverbPreset {
    reverb_type: u8,
    decay: u16,
    pre_delay: u8,
    diffuse: i8,
    low_color: i8,
    high_color: i8,
    high_factor: i8,
    mod_speed: i8,
    mod_depth: i8,
    early_level: i8,
    tail_level: i8,
}

impl ReverbPreset {
    fn get_preset(style: ReverbStyle) -> ReverbPreset {
        match style {
            Library => ReverbPreset {
                reverb_type: 9,
                decay: 77,
                pre_delay: 0,
                diffuse: 0,
                low_color: 0,
                high_color: -32,
                high_factor: -6,
                mod_speed: 0,
                mod_depth: 0,
                early_level: -1,
                tail_level: 0,
            },
            ReverbStyle::DarkBloom => ReverbPreset {
                reverb_type: 5,
                decay: 96,
                pre_delay: 0,
                diffuse: -50,
                low_color: 0,
                high_color: -50,
                high_factor: -25,
                mod_speed: -25,
                mod_depth: -25,
                early_level: -25,
                tail_level: 0,
            },
            ReverbStyle::MusicClub => ReverbPreset {
                reverb_type: 12,
                decay: 106,
                pre_delay: 15,
                diffuse: 0,
                low_color: 0,
                high_color: 0,
                high_factor: 0,
                mod_speed: 0,
                mod_depth: 0,
                early_level: 0,
                tail_level: 0,
            },
            ReverbStyle::RealPlate => ReverbPreset {
                reverb_type: 9,
                decay: 115,
                pre_delay: 15,
                diffuse: 0,
                low_color: 21,
                high_color: -17,
                high_factor: -22,
                mod_speed: -3,
                mod_depth: 8,
                early_level: 0,
                tail_level: 0,
            },
            ReverbStyle::Chapel => ReverbPreset {
                reverb_type: 0,
                decay: 118,
                pre_delay: 15,
                diffuse: 0,
                low_color: -23,
                high_color: -35,
                high_factor: 13,
                mod_speed: 0,
                mod_depth: 0,
                early_level: -25,
                tail_level: 0,
            },
            ReverbStyle::HockeyArena => ReverbPreset {
                reverb_type: 1,
                decay: 150,
                pre_delay: 100,
                diffuse: 0,
                low_color: 10,
                high_color: -39,
                high_factor: 21,
                mod_speed: -3,
                mod_depth: 21,
                early_level: 0,
                tail_level: 0,
            },
        }
    }
}
