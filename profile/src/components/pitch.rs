use std::collections::HashMap;
use std::io::Write;
use std::os::raw::c_float;

use enum_map::{Enum, EnumMap};
use strum::{EnumIter, EnumProperty, IntoEnumIterator};
use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use crate::components::colours::ColourMap;
use crate::components::megaphone::Preset;
use crate::components::megaphone::Preset::{Preset1, Preset2, Preset3, Preset4, Preset5, Preset6};

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
pub struct PitchEncoderBase {
    colour_map: ColourMap,
    preset_map: EnumMap<Preset, PitchEncoder>,
    active_set: u8, // Not sure what this does?
}

impl PitchEncoderBase {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name;
        Self {
            colour_map: ColourMap::new(colour_map),
            preset_map: EnumMap::default(),
            active_set: 0,
        }
    }

    pub fn parse_pitch_root(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name == "active_set" {
                self.active_set = attr.value.parse()?;
                continue;
            }

            if !self.colour_map.read_colours(attr)? {
                println!("[PitchEncoder] Unparsed Attribute: {}", attr.name);
            }
        }

        Ok(())
    }

    pub fn parse_pitch_preset(
        &mut self,
        id: u8,
        attributes: &[OwnedAttribute],
    ) -> Result<(), ParseError> {
        let mut preset = PitchEncoder::new();
        for attr in attributes {
            if attr.name.local_name == "PITCH_STYLE" {
                for style in PitchStyle::iter() {
                    if style.get_str("uiIndex").unwrap() == attr.value {
                        preset.style = style;
                        break;
                    }
                }
                continue;
            }

            if attr.name.local_name == "PITCH_KNOB_POSITION" {
                preset.knob_position = attr.value.parse::<c_float>()? as i8;
                continue;
            }

            if attr.name.local_name == "PITCH_RANGE" {
                preset.range = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "PITCH_SHIFT_THRESHOLD" {
                preset.threshold = attr.value.parse::<c_float>()? as i8;
                continue;
            }

            if attr.name.local_name == "PITCH_SHIFT_INST_RATIO" {
                preset.inst_ratio = Option::Some(attr.value.parse::<c_float>()? as u8);
                continue;
            }

            println!(
                "[PitchEncoder] Unparsed Child Attribute: {}",
                &attr.name.local_name
            );
        }

        // Ok, we should be able to store this now..
        if id == 1 {
            self.preset_map[Preset1] = preset;
        } else if id == 2 {
            self.preset_map[Preset2] = preset;
        } else if id == 3 {
            self.preset_map[Preset3] = preset;
        } else if id == 4 {
            self.preset_map[Preset4] = preset;
        } else if id == 5 {
            self.preset_map[Preset5] = preset;
        } else if id == 6 {
            self.preset_map[Preset6] = preset;
        }

        Ok(())
    }

    pub fn write_pitch<W: Write>(
        &self,
        writer: &mut EventWriter<&mut W>,
    ) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder = XmlWriterEvent::start_element("pitchEncoder");

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("active_set".to_string(), format!("{}", self.active_set));
        self.colour_map.write_colours(&mut attributes);

        // Write out the attributes etc for this element, but don't close it yet..
        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;

        // Because all of these are seemingly 'guaranteed' to exist, we can straight dump..
        for (key, value) in &self.preset_map {
            let mut sub_attributes: HashMap<String, String> = HashMap::default();

            let tag_name = format!("pitchEncoder{}", key.get_str("tagSuffix").unwrap());
            let mut sub_element: StartElementBuilder =
                XmlWriterEvent::start_element(tag_name.as_str());

            sub_attributes.insert(
                "PITCH_KNOB_POSITION".to_string(),
                format!("{}", value.knob_position),
            );
            sub_attributes.insert(
                "PITCH_STYLE".to_string(),
                value.style.get_str("uiIndex").unwrap().to_string(),
            );
            sub_attributes.insert("PITCH_RANGE".to_string(), format!("{}", value.range));
            sub_attributes.insert(
                "PITCH_SHIFT_THRESHOLD".to_string(),
                format!("{}", value.threshold),
            );

            if let Some(inst_ratio) = value.inst_ratio {
                sub_attributes.insert(
                    "PITCH_SHIFT_INST_RATIO".to_string(),
                    format!("{}", inst_ratio),
                );
            }

            for (key, value) in &sub_attributes {
                sub_element = sub_element.attr(key.as_str(), value.as_str());
            }

            writer.write(sub_element)?;
            writer.write(XmlWriterEvent::end_element())?;
        }

        // Finally, close the 'main' tag.
        writer.write(XmlWriterEvent::end_element())?;
        Ok(())
    }

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }

    pub fn colour_map_mut(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }

    pub fn get_preset(&self, preset: Preset) -> &PitchEncoder {
        &self.preset_map[preset]
    }

    pub fn get_preset_mut(&mut self, preset: Preset) -> &mut PitchEncoder {
        &mut self.preset_map[preset]
    }
}

#[derive(Debug, Default)]
pub struct PitchEncoder {
    knob_position: i8,
    style: PitchStyle,
    range: u8,
    threshold: i8,
    inst_ratio: Option<u8>,
}

impl PitchEncoder {
    pub fn new() -> Self {
        Self {
            knob_position: 0,
            style: PitchStyle::Narrow,
            range: 0,
            threshold: 0,
            inst_ratio: None,
        }
    }


    pub fn knob_position(&self) -> i8 {
        self.knob_position
    }

    pub fn set_knob_position(&mut self, knob_position: i8) {
        self.knob_position = knob_position;
    }

    pub fn style(&self) -> &PitchStyle {
        &self.style
    }

    pub fn range(&self) -> u8 {
        self.range
    }
    pub fn threshold(&self) -> i8 {
        self.threshold
    }
    pub fn inst_ratio(&self) -> Option<u8> {
        self.inst_ratio
    }
    pub fn inst_ratio_value(&self) -> u8 {
        if let Some(value) = self.inst_ratio {
            return value;
        }
        return 0;
    }
}

#[derive(Debug, PartialEq, EnumIter, Enum, EnumProperty, Copy, Clone)]
pub enum PitchStyle {
    #[strum(props(uiIndex = "0"))]
    Narrow,

    #[strum(props(uiIndex = "1"))]
    Wide,
}

impl Default for PitchStyle {
    fn default() -> Self {
        PitchStyle::Narrow
    }
}
