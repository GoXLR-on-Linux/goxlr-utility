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
            mute_function: MuteFunction::All,
            previous_volume: 0,
            from_mute_all: None,
        }
    }

    pub fn parse_button(&mut self, attributes: &[OwnedAttribute]) {
        for attr in attributes {
            if attr.name.local_name.ends_with("Function") {
                let mut found = false;

                // First catch this seemingly legacy value..
                if attr.value == "All" {
                    self.mute_function = MuteFunction::All;
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

    pub fn write_button(
        &self,
        writer: &mut EventWriter<&mut File>,
    ) -> Result<(), xml::writer::Error> {
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

        if let Some(from_mute_all) = self.from_mute_all {
            attributes.insert(
                "fromMuteAllFlag".to_string(),
                format!("{}", from_mute_all as u8),
            );
        }

        self.colour_map.write_colours(&mut attributes);

        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;
        writer.write(XmlWriterEvent::end_element())?;
        Ok(())
    }
}

// MuteChat
#[derive(Debug, Enum, EnumProperty, EnumIter)]
pub enum MuteFunction {
    #[strum(props(Value = "Mute All", uiIndex = "0"))]
    All,

    #[strum(props(Value = "Mute to Stream", uiIndex = "1"))]
    ToStream,

    #[strum(props(Value = "Mute to Voice Chat", uiIndex = "2"))]
    ToVoiceChat,

    #[strum(props(Value = "Mute to Phones", uiIndex = "3"))]
    ToPhones,

    #[strum(props(Value = "Mute to Line Out", uiIndex = "4"))]
    ToLineOut,
}
