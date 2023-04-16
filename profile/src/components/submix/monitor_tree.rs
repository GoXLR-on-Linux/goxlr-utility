use crate::components::mixer::InputChannels;
use crate::profile::Attribute;
use anyhow::Result;
use enum_map::EnumMap;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Writer;
use std::collections::HashMap;
use std::io::Write;
use strum::{EnumProperty, IntoEnumIterator};

#[derive(Debug)]
pub struct MonitorTree {
    monitored_output: u8,
    headphone_mix: u8,
    routing: EnumMap<InputChannels, u16>,
}

impl Default for MonitorTree {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorTree {
    pub fn new() -> Self {
        Self {
            monitored_output: 0,
            headphone_mix: 1,
            routing: MonitorTree::get_default_routing(),
        }
    }

    fn get_default_routing() -> EnumMap<InputChannels, u16> {
        let mut routing: EnumMap<InputChannels, u16> = Default::default();
        for channel in InputChannels::iter() {
            routing[channel] = 8192;
        }
        routing
    }

    pub fn parse_monitor_tree(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            if attr.name == "monitoredOutput" {
                self.monitored_output = attr.value.parse()?;
                continue;
            }

            if attr.name == "headphoneMix" {
                self.headphone_mix = attr.value.parse()?;
                continue;
            }

            // The monitor Mix only has <Channel>ToHP..
            if attr.name.ends_with("ToHP") {
                // Extract the two sides of the string..
                let name = attr.name.as_str();

                let input = &name[0..name.len() - 4];
                let value: u16 = attr.value.parse()?;

                // We need to find the two matching channels..
                for input_channel in InputChannels::iter() {
                    if input_channel.get_str("Name").unwrap() == input {
                        self.routing[input_channel] = value;
                        break;
                    }
                }

                continue;
            }
        }
        Ok(())
    }

    pub fn write_monitor_tree<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new("monitorTree");

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert(
            String::from("monitoredOutput"),
            self.monitored_output.to_string(),
        );

        attributes.insert(String::from("headphoneMix"), self.headphone_mix.to_string());
        for channel in InputChannels::iter() {
            let key = format!("{}ToHP", channel.get_str("Name").unwrap());
            attributes.insert(key, self.routing[channel].to_string());
        }

        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Empty(elem))?;
        Ok(())
    }
}
