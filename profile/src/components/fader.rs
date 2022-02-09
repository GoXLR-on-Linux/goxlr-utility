use std::collections::HashMap;
use std::fs::File;

use strum::{EnumProperty, IntoEnumIterator};
use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use crate::components::colours::ColourMap;
use crate::components::mixer::FullChannelList;

pub struct Fader {
    element_name: String,
    colour_map: ColourMap,
    channel: FullChannelList,
}

impl Fader {
    pub fn new(id: u8) -> Self {
        let element_name = format!("FaderMeter{}", id);
        let colour_map = format!("FaderMeter{}", id);
        Self {
            element_name,
            colour_map: ColourMap::new(colour_map),
            channel: FullChannelList::Mic,
        }
    }

    pub fn parse_fader(&mut self, attributes: &[OwnedAttribute]) {
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
            if !self.colour_map.read_colours(attr) {
                println!("[FADER] Unparsed Attribute: {}", attr.name);
            }
        }
    }

    pub fn write_fader(
        &self,
        writer: &mut EventWriter<&mut File>,
    ) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder =
            XmlWriterEvent::start_element(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert(
            format!("{}listIndex", self.element_name),
            self.channel.get_str("faderIndex").unwrap().to_string(),
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
