use std::collections::HashMap;
use std::fs::File;
use std::sync::mpsc::TrySendError::Full;
use xml::attribute::OwnedAttribute;
use xml::EventWriter;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use strum::{IntoEnumIterator, EnumProperty};
use crate::components::colours::ColourMap;
use crate::components::mixer::FullChannelList;

pub struct Effects {
    id: u8,
    element_name: String,
    colour_map: ColourMap,

    // This is represented only in the UI.
    name: String,
}

impl Effects {
    pub fn new(id: u8) -> Self {
        let element_name = format!("effects{}", id);
        let colour_map = format!("effects{}", id);
        let default_name = format!("Effects Group {}", id);
        Self {
            id,
            element_name,
            colour_map: ColourMap::new(colour_map),
            name: default_name
        }
    }

    pub fn parse_effect(&mut self, attributes: &Vec<OwnedAttribute>) {
        for attr in attributes {
            if attr.name.local_name.ends_with("Name") {
                self.name = attr.value.clone();
                continue;
            }

            // Send the rest out for colouring..
            if !self.colour_map.read_colours(&attr) {
                println!("[EFFECTS] Unparsed Attribute: {}", attr.name);
            }
        }
    }

    pub fn write_effects(&self, mut writer: &mut EventWriter<&mut File>) {
        let mut element: StartElementBuilder = XmlWriterEvent::start_element(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert(format!("{}Name", self.element_name), self.name.clone());

        self.colour_map.write_colours(&mut attributes);

        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element);
        writer.write(XmlWriterEvent::end_element());
    }
}