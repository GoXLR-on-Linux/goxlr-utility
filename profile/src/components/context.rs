use std::collections::HashMap;
use std::io::Write;

use strum::EnumProperty;
use strum::IntoEnumIterator;

use anyhow::Result;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Writer;

use crate::components::colours::ColourMap;
use crate::components::sample::SampleBank;
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
 * These have no special properties, they are literally just button colours..
 */
#[derive(Debug)]
pub struct Context {
    // Ok.
    element_name: String,
    colour_map: ColourMap,

    selected: u8,
    selected_id: Option<u8>,
    selected_sample: SampleBank, // These two should probably map to enums somewhere, matched up against
    selected_effects: Preset,    // the relevant sections of the tags (for quickly pulling presets)
}

impl Context {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name.clone();
        Self {
            element_name,
            colour_map: ColourMap::new(colour_map),

            selected: 0,
            selected_id: None,
            selected_sample: SampleBank::A,
            selected_effects: Preset::Preset1,
        }
    }

    pub fn parse_context(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            if attr.name == "numselected" {
                self.selected = attr.value.parse()?;
                continue;
            }

            if attr.name == "selectedID" {
                if !attr.value.is_empty() {
                    self.selected_id = Some(attr.value.parse()?);
                }
                continue;
            }

            if attr.name == "selectedSampleStack" {
                let value = attr.value.clone();
                for bank in SampleBank::iter() {
                    if bank.get_str("contextTitle").unwrap() == value {
                        self.selected_sample = bank;
                    }
                }
                continue;
            }

            if attr.name == "selectedEffectBank" {
                let value = attr.value.clone();

                // Ok, which preset do we match?
                for preset in Preset::iter() {
                    if preset.get_str("contextTitle").unwrap() == value {
                        self.selected_effects = preset;
                    }
                }
                continue;
            }

            if !self.colour_map.read_colours(attr)? {
                println!("[{}] Unparsed Attribute: {}", self.element_name, attr.name);
            }
        }

        Ok(())
    }

    pub fn write_context<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("numselected".to_string(), format!("{}", self.selected));

        if let Some(selected_id) = self.selected_id {
            attributes.insert("selectedID".to_string(), format!("{selected_id}"));
        } else {
            attributes.insert("selectedID".to_string(), "".to_string());
        }

        attributes.insert(
            "selectedSampleStack".to_string(),
            self.selected_sample
                .get_str("contextTitle")
                .unwrap()
                .to_string(),
        );
        attributes.insert(
            "selectedEffectBank".to_string(),
            self.selected_effects
                .get_str("contextTitle")
                .unwrap()
                .to_string(),
        );

        self.colour_map.write_colours(&mut attributes);

        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Empty(elem))?;
        Ok(())
    }

    pub fn set_selected_effects(&mut self, selected_effects: Preset) {
        self.selected_effects = selected_effects;
    }
    pub fn selected_effects(&self) -> Preset {
        self.selected_effects
    }

    pub fn selected_sample(&self) -> SampleBank {
        self.selected_sample
    }
    pub fn set_selected_sample(&mut self, selected_sample: SampleBank) {
        self.selected_sample = selected_sample;
    }
}
