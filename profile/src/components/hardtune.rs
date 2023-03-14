use std::collections::HashMap;
use std::io::Write;
use std::os::raw::c_float;
use std::str::FromStr;

use enum_map::EnumMap;
use strum::{Display, EnumIter, EnumProperty, EnumString, IntoEnumIterator};

use anyhow::{anyhow, Result};
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::Writer;

use crate::components::colours::ColourMap;
use crate::components::hardtune::HardTuneSource::All;
use crate::components::hardtune::HardTuneStyle::Natural;
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
pub struct HardtuneEffectBase {
    colour_map: ColourMap,
    preset_map: EnumMap<Preset, HardTuneEffect>,
    source: HardTuneSource,
}

impl HardtuneEffectBase {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name;
        Self {
            colour_map: ColourMap::new(colour_map),
            preset_map: EnumMap::default(),
            source: Default::default(),
        }
    }

    pub fn parse_hardtune_root(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            // I honestly have no idea why this lives here :D
            if attr.name == "HARDTUNE_SOURCE" {
                self.source = HardTuneSource::from_str(&attr.value)?;
                continue;
            }

            if !self.colour_map.read_colours(attr)? {
                println!("[hardTuneEffect] Unparsed Attribute: {}", attr.name);
            }
        }

        Ok(())
    }

    pub fn parse_hardtune_preset(
        &mut self,
        preset_enum: Preset,
        attributes: &Vec<Attribute>,
    ) -> Result<()> {
        let mut preset = HardTuneEffect::new();
        for attr in attributes {
            if attr.name == "hardtuneEffectstate" {
                preset.state = matches!(attr.value.as_str(), "1");
                continue;
            }
            if attr.name == "HARDTUNE_STYLE" {
                for style in HardTuneStyle::iter() {
                    if style.get_str("uiIndex").unwrap() == attr.value {
                        preset.style = style;
                        break;
                    }
                }
                continue;
            }

            if attr.name == "HARDTUNE_KEYSOURCE" {
                preset.key_source = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "HARDTUNE_AMOUNT" {
                preset.amount = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "HARDTUNE_WINDOW" {
                preset.window = attr.value.parse::<c_float>()? as u16;
                continue;
            }
            if attr.name == "HARDTUNE_RATE" {
                preset.rate = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "HARDTUNE_SCALE" {
                preset.scale = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "HARDTUNE_PITCH_AMT" {
                preset.pitch_amt = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "HARDTUNE_SOURCE" {
                preset.source = Some(HardTuneSource::from_str(&attr.value)?);
                continue;
            }

            println!("[HardTuneEffect] Unparsed Child Attribute: {}", &attr.name);
        }

        self.preset_map[preset_enum] = preset;
        Ok(())
    }

    pub fn write_hardtune<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new("hardtuneEffect");

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("HARDTUNE_SOURCE".to_string(), self.source.to_string());
        self.colour_map.write_colours(&mut attributes);

        // Write out the attributes etc for this element, but don't close it yet..
        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Start(elem))?;

        // Because all of these are seemingly 'guaranteed' to exist, we can straight dump..
        for preset in Preset::iter() {
            let tag_name = format!("hardtuneEffect{}", preset.get_str("tagSuffix").unwrap());
            let mut sub_elem = BytesStart::new(tag_name.as_str());

            let sub_attributes = self.get_preset_attributes(preset);
            for (key, value) in &sub_attributes {
                sub_elem.push_attribute((key.as_str(), value.as_str()));
            }

            writer.write_event(Event::Empty(sub_elem))?;
        }

        // Finally, close the 'main' tag.
        writer.write_event(Event::End(BytesEnd::new("hardtuneEffect")))?;
        Ok(())
    }

    pub fn get_preset_attributes(&self, preset: Preset) -> HashMap<String, String> {
        let mut attributes = HashMap::new();
        let value = &self.preset_map[preset];

        attributes.insert(
            "hardtuneEffectstate".to_string(),
            if value.state {
                "1".to_string()
            } else {
                "0".to_string()
            },
        );
        attributes.insert(
            "HARDTUNE_STYLE".to_string(),
            value.style.get_str("uiIndex").unwrap().to_string(),
        );
        attributes.insert(
            "HARDTUNE_KEYSOURCE".to_string(),
            format!("{}", value.key_source),
        );
        attributes.insert("HARDTUNE_AMOUNT".to_string(), format!("{}", value.amount));
        attributes.insert("HARDTUNE_WINDOW".to_string(), format!("{}", value.window));
        attributes.insert("HARDTUNE_RATE".to_string(), format!("{}", value.rate));
        attributes.insert("HARDTUNE_SCALE".to_string(), format!("{}", value.scale));
        attributes.insert(
            "HARDTUNE_PITCH_AMT".to_string(),
            format!("{}", value.pitch_amt),
        );

        if let Some(source) = &value.source {
            attributes.insert("HARDTUNE_SOURCE".to_string(), source.to_string());
        }

        attributes
    }

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }

    pub fn colour_map_mut(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }

    pub fn get_preset(&self, preset: Preset) -> &HardTuneEffect {
        &self.preset_map[preset]
    }

    pub fn get_preset_mut(&mut self, preset: Preset) -> &mut HardTuneEffect {
        &mut self.preset_map[preset]
    }
}

#[derive(Debug, Default)]
pub struct HardTuneEffect {
    // State here determines if the hardtune is on or off when this preset is loaded.
    state: bool,

    style: HardTuneStyle,
    key_source: u8,
    amount: u8,
    window: u16,
    rate: u8,
    scale: u8,
    pitch_amt: u8,
    source: Option<HardTuneSource>,
}

impl HardTuneEffect {
    pub fn new() -> Self {
        Self {
            state: false,
            style: Default::default(),
            key_source: 0,
            amount: 0,
            window: 0,
            rate: 0,
            scale: 0,
            pitch_amt: 0,
            source: None,
        }
    }

    pub fn state(&self) -> bool {
        self.state
    }
    pub fn set_state(&mut self, state: bool) {
        self.state = state;
    }

    pub fn style(&self) -> &HardTuneStyle {
        &self.style
    }
    pub fn set_style(&mut self, style: HardTuneStyle) -> Result<()> {
        self.style = style;

        let preset = HardtunePreset::get_preset(style);
        self.set_amount(preset.amount)?;
        self.set_window(preset.window)?;
        self.set_rate(preset.rate)?;
        self.set_scale(preset.scale);
        self.set_pitch_amt(preset.pitch_amt)?;

        Ok(())
    }

    pub fn key_source(&self) -> u8 {
        self.key_source
    }

    pub fn amount(&self) -> u8 {
        self.amount
    }
    pub fn set_amount(&mut self, value: u8) -> Result<()> {
        if value > 100 {
            return Err(anyhow!("Amount should be a percentage"));
        }
        self.amount = value;
        Ok(())
    }

    pub fn window(&self) -> u16 {
        self.window
    }
    pub fn set_window(&mut self, value: u16) -> Result<()> {
        if value > 600 {
            return Err(anyhow!("Window should be between 0 and 600"));
        }
        self.window = value;
        Ok(())
    }

    pub fn rate(&self) -> u8 {
        self.rate
    }
    pub fn set_rate(&mut self, value: u8) -> Result<()> {
        if value > 100 {
            return Err(anyhow!("Rate should be a percentage"));
        }
        self.rate = value;
        Ok(())
    }

    pub fn scale(&self) -> u8 {
        self.scale
    }
    fn set_scale(&mut self, value: u8) {
        self.scale = value;
    }

    pub fn pitch_amt(&self) -> u8 {
        self.pitch_amt
    }
    fn set_pitch_amt(&mut self, value: u8) -> Result<()> {
        if value != 0 {
            return Err(anyhow!("Hardtune Pitch Amt should be 0.."));
        }
        self.pitch_amt = value;
        Ok(())
    }

    pub fn source(&self) -> &Option<HardTuneSource> {
        &self.source
    }
    pub fn get_source_value(&self) -> HardTuneSource {
        if let Some(source) = self.source {
            return source;
        }
        All
    }
    pub fn set_source(&mut self, source: HardTuneSource) {
        self.source = Some(source);
    }

    pub fn get_source(&self) -> HardTuneSource {
        if let Some(source) = self.source {
            return source;
        }
        All
    }
}

#[derive(Default, Debug, EnumIter, EnumProperty, Clone, Copy)]
pub enum HardTuneStyle {
    #[default]
    #[strum(props(uiIndex = "0"))]
    Natural,

    #[strum(props(uiIndex = "1"))]
    Medium,

    #[strum(props(uiIndex = "2"))]
    Hard,
}

#[derive(Default, Debug, Display, EnumString, PartialEq, Eq, Copy, Clone)]
pub enum HardTuneSource {
    #[default]
    #[strum(to_string = "ALL")]
    All,

    #[strum(to_string = "MUSIC")]
    Music,

    #[strum(to_string = "GAME")]
    Game,

    #[strum(to_string = "LINEIN")]
    LineIn,

    #[strum(to_string = "SYSTEM")]
    System,
}

struct HardtunePreset {
    amount: u8,
    window: u16,
    rate: u8,
    scale: u8,
    pitch_amt: u8,
}

impl HardtunePreset {
    fn get_preset(style: HardTuneStyle) -> HardtunePreset {
        match style {
            Natural => HardtunePreset {
                amount: 70,
                window: 20,
                rate: 20,
                scale: 5,
                pitch_amt: 0,
            },
            HardTuneStyle::Medium => HardtunePreset {
                amount: 53,
                window: 20,
                rate: 99,
                scale: 5,
                pitch_amt: 0,
            },
            HardTuneStyle::Hard => HardtunePreset {
                amount: 100,
                window: 60,
                rate: 100,
                scale: 5,
                pitch_amt: 0,
            },
        }
    }
}
