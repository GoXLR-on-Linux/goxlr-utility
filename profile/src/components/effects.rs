use std::collections::HashMap;
use std::io::Write;

use anyhow::{anyhow, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Writer;
use strum::EnumProperty;

use crate::components::colours::{Colour, ColourMap, ColourOffStyle};
use crate::profile::Attribute;
use crate::Preset;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("[Effects] Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),

    #[error("[Effects] Expected float: {0}")]
    ExpectedFloat(#[from] std::num::ParseFloatError),

    #[error("[Effects] Expected enum: {0}")]
    ExpectedEnum(#[from] strum::ParseError),

    #[error("[Effects] Invalid colours: {0}")]
    InvalidColours(#[from] crate::components::colours::ParseError),
}

#[derive(Debug)]
pub struct Effects {
    element_name: String,
    colour_map: ColourMap,

    // This is represented only in the UI.
    name: String,
}

impl Effects {
    pub fn new(preset: Preset) -> Self {
        let element_name = preset.get_str("contextTitle").unwrap().to_string();
        let default_name = format!("Effects Group {}", preset.get_str("contextTitle").unwrap());

        let mut colour_map = ColourMap::new(element_name.clone());
        colour_map.set_off_style(ColourOffStyle::Dimmed);
        colour_map.set_blink_on(false);
        colour_map.set_state_on(false);
        colour_map.set_colour(0, Colour::fromrgb("00FFFF").unwrap());
        colour_map.set_colour(1, Colour::fromrgb("000000").unwrap());
        colour_map.set_colour_group("samplesGroup".to_string());

        if preset == Preset::Preset1 {
            colour_map.set_state_on(true);
        }

        Self {
            element_name,
            colour_map,
            name: default_name,
        }
    }

    pub fn parse_effect(&mut self, attributes: &Vec<Attribute>) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.ends_with("Name") {
                self.name = attr.value.clone();
                continue;
            }

            // Send the rest out for colouring..
            if !self.colour_map.read_colours(attr)? {
                println!("[EFFECTS] Unparsed Attribute: {}", attr.name);
            }
        }

        Ok(())
    }

    pub fn write_effects<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert(format!("{}Name", self.element_name), self.name.clone());

        self.colour_map.write_colours(&mut attributes);

        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Empty(elem))?;
        Ok(())
    }

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }
    pub fn colour_map_mut(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn set_name(&mut self, name: String) -> Result<()> {
        // This is an artificial limit by me here..
        if name.len() > 32 {
            return Err(anyhow!("Name must be less than 32 characters"));
        }

        if !name
            .chars()
            .all(|x| x.is_alphanumeric() || x.is_whitespace())
        {
            return Err(anyhow!("Name must be alpha-numeric"));
        }

        // Trim any whitespaces..
        self.name = name.trim().to_string();

        Ok(())
    }
}
