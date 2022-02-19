use std::collections::HashMap;
use std::fs::File;

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
 * I've not seen, or been able to get any of the values in browserPreviewTree to actually set..
 * it's possible this is used when previewing samples, as an internal state track there..
 */
#[derive(Debug)]
pub struct BrowserPreviewTree {
    element_name: String,
    colour_map: ColourMap,

    playing: u8,
    file: String,
    play_toggle: u8,
    current_relative_time: u8,
}

impl BrowserPreviewTree {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name.clone();
        Self {
            element_name,
            colour_map: ColourMap::new(colour_map),

            playing: 0,
            file: "".to_string(),
            play_toggle: 0,
            current_relative_time: 0,
        }
    }

    pub fn parse_browser(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name == "playing" {
                self.playing = attr.value.parse()?;
                continue;
            }

            if attr.name.local_name == "playToggle" {
                self.play_toggle = attr.value.parse()?;
                continue;
            }

            if attr.name.local_name == "file" {
                self.file = attr.value.clone();
                continue;
            }

            if attr.name.local_name == "currentRelativeTime" {
                self.current_relative_time = attr.value.parse()?;
                continue;
            }

            if !self.colour_map.read_colours(attr)? {
                println!("[{}] Unparsed Attribute: {}", self.element_name, attr.name);
            }
        }

        Ok(())
    }

    pub fn write_browser(
        &self,
        writer: &mut EventWriter<&mut File>,
    ) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder =
            XmlWriterEvent::start_element(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("playing".to_string(), format!("{}", self.playing));
        attributes.insert("playToggle".to_string(), format!("{}", self.play_toggle));
        attributes.insert("file".to_string(), self.file.clone());
        attributes.insert(
            "currentRelativeTime".to_string(),
            format!("{}", self.current_relative_time),
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
