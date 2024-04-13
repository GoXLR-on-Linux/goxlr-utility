use std::collections::HashMap;
use std::io::Write;

use strum::{EnumProperty, IntoEnumIterator};

use anyhow::Result;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Writer;

use crate::components::colours::{Colour, ColourDisplay, ColourMap, ColourOffStyle};
use crate::components::mixer::FullChannelList;
use crate::profile::Attribute;
use crate::Faders;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("[Fader] Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),

    #[error("[Fader] Expected float: {0}")]
    ExpectedFloat(#[from] std::num::ParseFloatError),

    #[error("[Fader] Expected enum: {0}")]
    ExpectedEnum(#[from] strum::ParseError),

    #[error("[Fader] Invalid colours: {0}")]
    InvalidColours(#[from] crate::components::colours::ParseError),
}

#[derive(Debug)]
pub struct Fader {
    element_name: String,

    colour_map: ColourMap,
    channel: FullChannelList,
}

impl Fader {
    pub fn new(fader: Faders) -> Self {
        let context = fader.get_str("faderContext").unwrap();

        // Build a Default ColourMap..
        let mut colour_map = ColourMap::new(context.to_string());
        colour_map.set_fader_display(ColourDisplay::TwoColour);
        colour_map.set_off_style(ColourOffStyle::Dimmed);
        colour_map.set_colour(0, Colour::fromrgb("000000").unwrap());
        colour_map.set_colour(1, Colour::fromrgb("00FFFF").unwrap());
        colour_map.set_colour_group("faderGroup".to_string());

        Self {
            element_name: context.to_string(),

            colour_map,
            channel: FullChannelList::Mic,
        }
    }

    pub fn parse_fader(&mut self, attributes: &Vec<Attribute>) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.ends_with("listIndex") {
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

    pub fn write_fader<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let element_name = &self.element_name;

        let mut elem = BytesStart::new(element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert(
            format!("{element_name}listIndex"),
            self.channel.get_str("faderIndex").unwrap().to_string(),
        );

        self.colour_map
            .write_colours_with_prefix(element_name.clone(), &mut attributes);

        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Empty(elem))?;
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
