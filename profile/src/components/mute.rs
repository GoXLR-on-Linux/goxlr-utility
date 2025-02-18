use std::collections::HashMap;
use std::io::Write;

use enum_map_derive::Enum;
use strum::{Display, EnumIter, EnumProperty, IntoEnumIterator};

use anyhow::Result;
use log::warn;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Writer;

use crate::components::colours::{Colour, ColourMap, ColourOffStyle};
use crate::profile::Attribute;
use crate::Faders;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("[MUTE] Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),

    #[error("[MUTE] Expected float: {0}")]
    ExpectedFloat(#[from] std::num::ParseFloatError),

    #[error("[MUTE] Expected enum: {0}")]
    ExpectedEnum(#[from] strum::ParseError),

    #[error("[MUTE] Invalid colours: {0}")]
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
    pub fn new(fader: Faders) -> Self {
        let context = fader.get_str("muteContext").unwrap();

        let mut colour_map = ColourMap::new(context.to_string());
        colour_map.set_off_style(ColourOffStyle::Dimmed);
        colour_map.set_blink_on(false);
        colour_map.set_state_on(false);
        colour_map.set_colour(0, Colour::fromrgb("00FFFF").unwrap());
        colour_map.set_colour(1, Colour::fromrgb("000000").unwrap());
        colour_map.set_colour_group("muteGroup".to_string());

        Self {
            colour_map,
            mute_function: MuteFunction::All,
            previous_volume: 0,

            from_mute_all: None,
        }
    }

    pub fn parse_button(&mut self, attributes: &Vec<Attribute>) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.ends_with("Function") {
                let mut found = false;

                // First catch this seemingly legacy value..
                if attr.value == "All" {
                    self.mute_function = MuteFunction::All;
                    continue;
                }

                // Legacy values from GoXLR App 1.3 and Pre-Mix 2
                let value = if attr.value == "Mute to Chat Mic" {
                    String::from("Mute to Voice Chat")
                } else if attr.value == "Mute to Stream" {
                    String::from("Mute to Stream 1")
                } else {
                    attr.value.clone()
                };

                for function in MuteFunction::iter() {
                    if function.get_str("Value").unwrap() == value {
                        self.mute_function = function;
                        found = true;
                        break;
                    }
                }
                if !found {
                    warn!("Couldn't find Mute Function: {}", value);
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

    pub fn write_button<W: Write>(&self, writer: &mut Writer<W>, fader: Faders) -> Result<()> {
        let element_name = fader.get_str("muteContext").unwrap();
        let mut elem = BytesStart::new(element_name);

        let mut attributes: HashMap<String, String> = HashMap::default();
        let mute_value = if self.mute_function == MuteFunction::ToVoiceChat {
            String::from("Mute to Chat Mic")
        } else {
            self.mute_function.get_str("Value").unwrap().to_string()
        };
        attributes.insert(format!("{element_name}Function"), mute_value);
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
            .write_colours_with_prefix(element_name.into(), &mut attributes);

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
#[derive(Debug, Display, Copy, Clone, Enum, EnumProperty, EnumIter, PartialEq, Eq)]
pub enum MuteFunction {
    #[strum(props(Value = "Mute All", uiIndex = "0"))]
    All,

    #[strum(props(Value = "Mute to Stream 1", uiIndex = "1"))]
    ToStream,

    #[strum(props(Value = "Mute to Voice Chat", uiIndex = "2"))]
    ToVoiceChat,

    #[strum(props(Value = "Mute to Phones", uiIndex = "3"))]
    ToPhones,

    #[strum(props(Value = "Mute to Line Out", uiIndex = "4"))]
    ToLineOut,

    #[strum(props(Value = "Mute to Stream 2", uiIndex = "5"))]
    ToStream2,

    #[strum(props(Value = "Mute to Streams 1 + 2", uiIndex = "6"))]
    ToStreams,
}
