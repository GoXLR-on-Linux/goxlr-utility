use std::collections::HashMap;
use std::io::Write;

use strum::{EnumProperty, IntoEnumIterator};
use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use anyhow::Result;

use crate::components::colours::ColourMap;
use crate::components::mixer::FullChannelList;

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

#[derive(Debug)]
pub struct Fader {
    colour_map: ColourMap,
    channel: FullChannelList,
}

impl Fader {
    pub fn new(id: u8) -> Self {
        let colour_map = format!("FaderMeter{}", id);
        Self {
            colour_map: ColourMap::new(colour_map),
            channel: FullChannelList::Mic,
        }
    }

    pub fn parse_fader(&mut self, attributes: &[OwnedAttribute]) -> Result<()> {
        for attr in attributes {
            if attr.name.local_name.ends_with("listIndex") {
                let mut found = false;

                // Iterate over the channels, and see which one this matches..
                for channel in FullChannelList::iter() {
                    if channel.get_str("faderIndex").unwrap() == attr.value {
                        self.channel = channel;
                        found = true;
                        break;
                    }
                }

                if !found {
                    println!("Cannot Find Fader Index: {}", attr.value);
                }
                continue;
            }

            // Send the rest out for colouring..
            if !self.colour_map.read_colours(attr)? {
                println!("[FADER] Unparsed Attribute: {}", attr.name);
            }
        }

        Ok(())
    }

    pub fn write_fader<W: Write>(
        &self,
        element_name: String,
        writer: &mut EventWriter<&mut W>,
    ) -> Result<()> {
        let mut element: StartElementBuilder = XmlWriterEvent::start_element(element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert(
            format!("{}listIndex", element_name),
            self.channel.get_str("faderIndex").unwrap().to_string(),
        );

        self.colour_map
            .write_colours_with_prefix(element_name.clone(), &mut attributes);

        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;
        writer.write(XmlWriterEvent::end_element())?;
        Ok(())
    }

    pub fn channel(&self) -> FullChannelList {
        self.channel
    }
    pub fn set_channel(&mut self, channel: FullChannelList) {
        self.channel = channel;
    }

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }
    pub fn colour_map_mut(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }
}
