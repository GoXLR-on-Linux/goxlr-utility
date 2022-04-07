use std::collections::HashMap;
use std::io::Write;

use enum_map_derive::Enum;
use strum::{EnumIter, EnumProperty, IntoEnumIterator};
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

#[derive(Debug)]
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

    pub fn parse_button(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
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
                let value: u8 = attr.value.parse()?;
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
            if !self.colour_map.read_colours(attr)? {
                println!("[MUTE BUTTON] Unparsed Attribute: {}", attr.name);
            }
        }

        Ok(())
    }

    pub fn write_button<W: Write>(
        &self,
        writer: &mut EventWriter<&mut W>,
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

    pub fn colour_map(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }

    pub fn mute_function(&self) -> &MuteFunction {
        &self.mute_function
    }



    pub fn set_previous_volume(&mut self, previous_volume: u8) {
        self.previous_volume = previous_volume;
    }
    pub fn previous_volume(&self) -> u8 {
        self.previous_volume
    }
}

// MuteChat
#[derive(Debug, Copy, Clone, Enum, EnumProperty, EnumIter, PartialEq)]
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
