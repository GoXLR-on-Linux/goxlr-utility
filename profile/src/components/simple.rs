use std::collections::HashMap;
use std::io::Write;

use anyhow::Result;

use enum_map::Enum;
use quick_xml::Writer;
use quick_xml::events::{BytesStart, Event};
use strum::{Display, EnumIter, EnumString};

use crate::components::colours::{Colour, ColourMap, ColourOffStyle};
use crate::profile::Attribute;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("[SIMPLE] Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),

    #[error("[SIMPLE] Expected float: {0}")]
    ExpectedFloat(#[from] std::num::ParseFloatError),

    #[error("[SIMPLE] Expected enum: {0}")]
    ExpectedEnum(#[from] strum::ParseError),

    #[error("[SIMPLE] Invalid colours: {0}")]
    InvalidColours(#[from] crate::components::colours::ParseError),
}

/**
 * These have no special properties, they are literally just button colours..
 */
#[derive(Debug)]
pub struct SimpleElement {
    // Ok.
    element_name: String,
    colour_map: ColourMap,
}

impl SimpleElement {
    pub fn new(element: SimpleElements) -> Self {
        let element_name = element.to_string();

        let mut colour_map = ColourMap::new(element_name.clone());
        colour_map.set_off_style(ColourOffStyle::Dimmed);
        colour_map.set_blink_on(false);
        colour_map.set_state_on(false);
        colour_map.set_colour(0, Colour::fromrgb("00FFFF").unwrap());
        colour_map.set_colour(1, Colour::fromrgb("FFFFFF").unwrap());

        if element == SimpleElements::SampleBankA {
            colour_map.set_state_on(true);
            colour_map.set_colour(2, Colour::fromrgb("000000").unwrap())
        }

        if element == SimpleElements::SampleBankB || element == SimpleElements::SampleBankC {
            colour_map.set_colour(2, Colour::fromrgb("000000").unwrap())
        }

        if element == SimpleElements::GlobalColour {
            colour_map.set_colour(1, Colour::fromrgb("000000").unwrap());
        }

        if element == SimpleElements::LogoX {
            colour_map.set_velocity(127);
        }

        Self {
            element_name,
            colour_map,
        }
    }

    pub fn parse_simple(&mut self, attributes: &Vec<Attribute>) -> Result<(), ParseError> {
        for attr in attributes {
            if !self.colour_map.read_colours(attr)? {
                println!("[{}] Unparsed Attribute: {}", self.element_name, attr.name);
            }
        }

        Ok(())
    }

    pub fn write_simple<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        self.colour_map.write_colours(&mut attributes);

        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Empty(elem))?;
        Ok(())
    }

    pub fn element_name(&self) -> &str {
        &self.element_name
    }

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }
    pub fn colour_map_mut(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }
}

#[derive(Debug, Display, EnumString, EnumIter, Enum, Clone, Copy, PartialEq)]
pub enum SimpleElements {
    #[strum(to_string = "sampleBankA")]
    SampleBankA,

    #[strum(to_string = "sampleBankB")]
    SampleBankB,

    #[strum(to_string = "sampleBankC")]
    SampleBankC,

    #[strum(to_string = "fxClear")]
    FxClear,

    #[strum(to_string = "swear")]
    Swear,

    #[strum(to_string = "globalColour")]
    GlobalColour,

    #[strum(to_string = "logoX")]
    LogoX,
}
