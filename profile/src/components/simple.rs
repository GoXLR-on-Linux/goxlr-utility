use std::collections::HashMap;
use std::fs::File;

use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use crate::components::colours::ColourMap;

/**
 * These have no special properties, they are literally just button colours..
 */
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

    pub fn parse_simple(&mut self, attributes: &Vec<OwnedAttribute>) {
        for attr in attributes {
            if !self.colour_map.read_colours(&attr) {
                println!("[{}] Unparsed Attribute: {}", self.element_name, attr.name);
            }
        }
    }

    pub fn write_simple(&self, writer: &mut EventWriter<&mut File>) {
        let mut element: StartElementBuilder =
            XmlWriterEvent::start_element(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        self.colour_map.write_colours(&mut attributes);

        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element);
        writer.write(XmlWriterEvent::end_element());
    }
}
