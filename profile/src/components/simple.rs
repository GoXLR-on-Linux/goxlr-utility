use std::collections::HashMap;
use std::io::Write;

use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use enum_map::Enum;
use strum::{Display, EnumIter, EnumString};

use crate::components::colours::ColourMap;

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
pub struct SimpleElement {
    // Ok.
    element_name: String,
    colour_map: ColourMap,
}

impl SimpleElement {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name.clone();
        Self {
            element_name,
            colour_map: ColourMap::new(colour_map),
        }
    }

    pub fn parse_simple(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if !self.colour_map.read_colours(attr)? {
                println!("[{}] Unparsed Attribute: {}", self.element_name, attr.name);
            }
        }

        Ok(())
    }

    pub fn write_simple<W: Write>(
        &self,
        writer: &mut EventWriter<&mut W>,
    ) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder =
            XmlWriterEvent::start_element(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        self.colour_map.write_colours(&mut attributes);

        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;
        writer.write(XmlWriterEvent::end_element())?;
        Ok(())
    }

    pub fn element_name(&self) -> &str {
        &self.element_name
    }

    pub fn colour_map_mut(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }
}

#[derive(Debug, Display, EnumString, EnumIter, Enum, Clone, Copy)]
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
