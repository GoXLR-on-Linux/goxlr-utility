use std::collections::HashMap;
use std::io::Write;

use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

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
pub struct Context {
    // Ok.
    element_name: String,
    colour_map: ColourMap,

    selected: u8,
    selected_id: Option<u8>,
    selected_sample: String, // These two should probably map to enums somewhere, matched up against
    selected_effects: String, // the relevant sections of the tags (for quickly pulling presets)
}

impl Context {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name.clone();
        Self {
            element_name,
            colour_map: ColourMap::new(colour_map),

            selected: 0,
            selected_id: None,
            selected_sample: "".to_string(),
            selected_effects: "".to_string(),
        }
    }

    pub fn parse_context(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name == "numselected" {
                self.selected = attr.value.parse()?;
                continue;
            }

            if attr.name.local_name == "selectedID" {
                if !attr.value.is_empty() {
                    self.selected_id = Some(attr.value.parse()?);
                }
                continue;
            }

            if attr.name.local_name == "selectedSampleStack" {
                self.selected_sample = attr.value.clone();
                continue;
            }

            if attr.name.local_name == "selectedEffectBank" {
                self.selected_effects = attr.value.clone();
                continue;
            }

            if !self.colour_map.read_colours(attr)? {
                println!("[{}] Unparsed Attribute: {}", self.element_name, attr.name);
            }
        }

        Ok(())
    }

    pub fn write_context<W: Write>(
        &self,
        writer: &mut EventWriter<&mut W>,
    ) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder =
            XmlWriterEvent::start_element(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("numselected".to_string(), format!("{}", self.selected));

        if let Some(selected_id) = self.selected_id {
            attributes.insert("selectedID".to_string(), format!("{}", selected_id));
        } else {
            attributes.insert("selectedID".to_string(), "".to_string());
        }

        attributes.insert(
            "selectedSampleStack".to_string(),
            self.selected_sample.clone(),
        );
        attributes.insert(
            "selectedEffectBank".to_string(),
            self.selected_effects.clone(),
        );

        self.colour_map.write_colours(&mut attributes);

        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;
        writer.write(XmlWriterEvent::end_element())?;
        Ok(())
    }
}
