use std::collections::HashMap;
use std::fs::File;

use enum_map_derive::Enum;
use strum::{EnumIter, EnumProperty, IntoEnumIterator};
use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use crate::components::colours::ColourMap;

pub struct MuteButton {
    element_name: String,
    colour_map: ColourMap,
    mute_function: MuteFunction,
    previous_volume: u8,

    // Labelled as 'fromMuteAllFlag' in the XML, honestly, not sure what this does either,
    // it's either 1, 0 or simply not there.
    from_mute_all: Option<bool>,
}

impl MuteButton {
    pub fn new(id: u8) -> Self {
        let element_name = format!("mute{}", id);
        let colour_prefix = format!("mute{}", id);
        Self {
            element_name,
            colour_map: ColourMap::new(colour_prefix),
            mute_function: MuteFunction::MUTE_ALL,
            previous_volume: 0,
            from_mute_all: None,
        }
    }

    pub fn parse_button(&mut self, attributes: &Vec<OwnedAttribute>) {
        for attr in attributes {
            if attr.name.local_name.ends_with("Function") {
                let mut found = false;

                // First catch this seemingly legacy value..
                if attr.value == "All" {
                    self.mute_function = MuteFunction::MUTE_ALL;
                    continue;
                }

                for function in MuteFunction::iter() {
                    if function.get_str("Value").unwrap() == attr.value {
                        self.mute_function = function;
                        found = true;
                        break;
                    }
                }
                if !found {
                    println!("Couldn't find Mute Function: {}", attr.value);
                }
                continue;
            }

            if attr.name.local_name.ends_with("prevLevel") {
                // Simple, parse this into a u8 :)
                let value: u8 = attr.value.parse().unwrap();
                self.previous_volume = value;
                continue;
            }

            if attr.name.local_name == "fromMuteAllFlag" {
                if attr.value == "0" {
                    self.from_mute_all = Option::Some(false);
                } else {
                    self.from_mute_all = Option::Some(true);
                }
                continue;
            }

            // Check to see if this is a colour related attribute..
            if !self.colour_map.read_colours(attr) {
                println!("[MUTE BUTTON] Unparsed Attribute: {}", attr.name);
            }
        }
    }

    pub fn write_button(&self, writer: &mut EventWriter<&mut File>) {
        let mut element: StartElementBuilder =
            XmlWriterEvent::start_element(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert(
            format!("{}Function", self.element_name),
            self.mute_function.get_str("Value").unwrap().to_string(),
        );
        attributes.insert(
            format!("{}prevLevel", self.element_name),
            format!("{}", self.previous_volume),
        );

        if self.from_mute_all.is_some() {
            attributes.insert(
                "fromMuteAllFlag".to_string(),
                format!("{}", self.from_mute_all.unwrap() as u8),
            );
        }

        self.colour_map.write_colours(&mut attributes);

        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element);
        writer.write(XmlWriterEvent::end_element());
    }
}

// MuteChat
#[derive(Debug, Enum, EnumProperty, EnumIter)]
pub enum MuteFunction {
    #[strum(props(Value = "Mute All", uiIndex = "0"))]
    MUTE_ALL,

    #[strum(props(Value = "Mute to Stream", uiIndex = "1"))]
    MUTE_TO_STREAM,

    #[strum(props(Value = "Mute to Voice Chat", uiIndex = "2"))]
    MUTE_TO_VOICE_CHAT,

    #[strum(props(Value = "Mute to Phones", uiIndex = "3"))]
    MUTE_TO_PHONES,

    #[strum(props(Value = "Mute to Line Out", uiIndex = "4"))]
    MUTE_TO_LINE_OUT,
}
