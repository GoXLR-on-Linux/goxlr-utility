use anyhow::Result;
use std::collections::HashMap;
use std::io::Write;

use crate::components::mixer::OutputChannels;
use crate::profile::Attribute;
use enum_map::{Enum, EnumMap};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Writer;
use strum::{EnumIter, EnumProperty, IntoEnumIterator};

#[derive(Debug)]
pub struct MixRoutingTree {
    mix: EnumMap<OutputChannels, Mix>,
}

impl Default for MixRoutingTree {
    fn default() -> Self {
        Self::new()
    }
}

impl MixRoutingTree {
    pub fn new() -> Self {
        Self {
            mix: Default::default(),
        }
    }

    pub fn parse_mix_tree(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            // Normally, I'd add some fancy code to iterate the OutputChannel, but for the tree here
            // they have different names to anywhere else, so we'll do it by hand.

            // Firstly, work out the Mix value..
            if let Some(value) = Mix::iter().nth(attr.value.parse::<usize>()? - 1) {
                if attr.name == "headphone" {
                    self.mix[OutputChannels::Headphones] = value;
                    continue;
                }

                if attr.name == "lineout" {
                    self.mix[OutputChannels::LineOut] = value;
                    continue;
                }

                if attr.name == "chat" {
                    self.mix[OutputChannels::ChatMic] = value;
                    continue;
                }

                if attr.name == "sampler" {
                    self.mix[OutputChannels::Sampler] = value;
                    continue;
                }

                if attr.name == "stream" {
                    self.mix[OutputChannels::Broadcast] = value;
                    continue;
                }
            }
        }
        Ok(())
    }

    pub fn write_mix_tree<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new("mixRoutingTree");

        // This one's actually incredibly straight forward :)
        let mut attributes: HashMap<String, String> = HashMap::default();

        attributes.insert(
            String::from("headphone"),
            (self.mix[OutputChannels::Headphones] as u8 + 1).to_string(),
        );

        attributes.insert(
            String::from("lineout"),
            (self.mix[OutputChannels::LineOut] as u8 + 1).to_string(),
        );

        attributes.insert(
            String::from("chat"),
            (self.mix[OutputChannels::ChatMic] as u8 + 1).to_string(),
        );

        attributes.insert(
            String::from("sampler"),
            (self.mix[OutputChannels::Sampler] as u8 + 1).to_string(),
        );

        attributes.insert(
            String::from("stream"),
            (self.mix[OutputChannels::Broadcast] as u8 + 1).to_string(),
        );

        // Set the attributes into the XML object..
        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Empty(elem))?;

        Ok(())
    }

    pub fn mix(&self) -> EnumMap<OutputChannels, Mix> {
        self.mix
    }

    pub fn get_assignment(&self, channel: OutputChannels) -> Mix {
        self.mix[channel]
    }

    pub fn set_assignment(&mut self, channel: OutputChannels, mix: Mix) -> Result<()> {
        self.mix[channel] = mix;
        Ok(())
    }
}

#[derive(Default, Debug, Copy, Clone, EnumIter, Enum, EnumProperty)]
pub enum Mix {
    #[default]
    A,
    B,
}
