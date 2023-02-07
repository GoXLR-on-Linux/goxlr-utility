use std::collections::HashMap;
use std::io::Write;

use enum_map_derive::Enum;
use strum::{EnumIter, EnumProperty, IntoEnumIterator};

use anyhow::Result;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Writer;

use crate::components::colours::ColourMap;
use crate::profile::Attribute;

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
    colour_map: ColourMap,
    mute_function: MuteFunction,
    previous_volume: u8,

    // Labelled as 'fromMuteAllFlag' in the XML, honestly, not sure what this does either,
    // it's either 1, 0 or simply not there.
    from_mute_all: Option<bool>,
}

impl MuteButton {
    pub fn new(id: u8) -> Self {
        let colour_prefix = format!("mute{id}");
        Self {
            colour_map: ColourMap::new(colour_prefix),
            mute_function: MuteFunction::All,
            previous_volume: 0,
            from_mute_all: None,
        }
    }

    pub fn parse_button(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            if attr.name.ends_with("Function") {
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

            if attr.name.ends_with("prevLevel") {
                // Simple, parse this into a u8 :)
                let value: u8 = attr.value.parse()?;
                self.previous_volume = value;
                continue;
            }

            if attr.name == "fromMuteAllFlag" {
                if attr.value == "0" {
                    self.from_mute_all = Some(false);
                } else {
                    self.from_mute_all = Some(true);
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
        element_name: String,
        writer: &mut Writer<W>,
    ) -> Result<()> {
        let mut elem = BytesStart::new(element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert(
            format!("{element_name}Function"),
            self.mute_function.get_str("Value").unwrap().to_string(),
        );
        attributes.insert(
            format!("{element_name}prevLevel"),
            format!("{}", self.previous_volume),
        );

        if let Some(from_mute_all) = self.from_mute_all {
            attributes.insert(
                "fromMuteAllFlag".to_string(),
                format!("{}", from_mute_all as u8),
            );
        }

        self.colour_map
            .write_colours_with_prefix(element_name.clone(), &mut attributes);

        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Empty(elem))?;
        Ok(())
    }

    pub fn colour_map_mut(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }
    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }

    pub fn mute_function(&self) -> &MuteFunction {
        &self.mute_function
    }
    pub fn set_mute_function(&mut self, mute_function: MuteFunction) {
        self.mute_function = mute_function;
    }

    pub fn set_previous_volume(&mut self, previous_volume: u8) -> Result<()> {
        self.previous_volume = previous_volume;
        Ok(())
    }
    pub fn previous_volume(&self) -> u8 {
        self.previous_volume
    }
}

// MuteChat
#[derive(Debug, Copy, Clone, Enum, EnumProperty, EnumIter, PartialEq, Eq)]
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
