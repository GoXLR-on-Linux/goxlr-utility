use std::collections::HashMap;
use std::io::Write;

use enum_map::{Enum, EnumMap};
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
pub struct Mixers {
    mixer_table: EnumMap<InputChannels, EnumMap<OutputChannels, u16>>,
    volume_table: EnumMap<FullChannelList, u8>,
    colour_map: ColourMap,
}

impl Default for Mixers {
    fn default() -> Self {
        Self::new()
    }
}

impl Mixers {
    pub fn new() -> Self {
        Self {
            mixer_table: EnumMap::default(),
            volume_table: EnumMap::default(),
            colour_map: ColourMap::new("mixerTree".to_string()),
        }
    }

    pub fn parse_mixers(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            if attr.name.ends_with("Level") {
                let mut found = false;

                // Get the String key..
                let channel = attr.name.as_str();
                let channel = &channel[0..channel.len() - 5];

                let value: u8 = attr.value.parse()?;

                // Find the channel from the Prefix..
                for volume in FullChannelList::iter() {
                    if volume.get_str("Name").unwrap() == channel {
                        // Set the value..
                        self.volume_table[volume] = value;
                        found = true;
                    }
                }

                if !found {
                    println!("Unable to find Channel: {channel}");
                }
                continue;
            }

            if attr.name.contains("To") {
                // Extract the two sides of the string..
                let name = attr.name.as_str();

                if let Some(middle_index) = name.find("To") {
                    let input = &name[0..middle_index];
                    let output = &name[middle_index + 2..];

                    let value: u16 = attr.value.parse()?;

                    // We need to find the two matching channels..
                    for input_channel in InputChannels::iter() {
                        if input_channel.get_str("Name").unwrap() == input {
                            // Borrow this section of the mixer table before checkout outputs..
                            let table = &mut self.mixer_table[input_channel];

                            for output_channel in OutputChannels::iter() {
                                if output_channel.get_str("Name").unwrap() == output {
                                    // Matched the output, store the value.
                                    table[output_channel] = value;
                                }
                            }
                        }
                    }
                }
                continue;
            }

            // Check to see if this is a colour related attribute..
            if !self.colour_map.read_colours(attr)? {
                println!("[MIXER] Unparsed Attribute: {}", attr.name);
            }
        }

        Ok(())
    }

    pub fn write_mixers<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new("mixerTree");

        // Create the values..
        let mut attributes: HashMap<String, String> = HashMap::default();
        for volume in FullChannelList::iter() {
            let key = format!("{}Level", volume.get_str("Name").unwrap());
            let value = format!("{}", self.volume_table[volume]);

            attributes.insert(key, value);
        }

        for input in InputChannels::iter() {
            // Get the map for this channel..
            let input_text = input.get_str("Name").unwrap();
            let table = self.mixer_table[input];

            for output in OutputChannels::iter() {
                let key = format!("{}To{}", input_text, output.get_str("Name").unwrap());
                let value = format!("{}", table[output]);

                attributes.insert(key, value);
            }
        }

        self.colour_map.write_colours(&mut attributes);

        // Set the attributes into the XML object..
        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Empty(elem))?;
        Ok(())
    }

    pub fn mixer_table(&self) -> &EnumMap<InputChannels, EnumMap<OutputChannels, u16>> {
        &self.mixer_table
    }

    pub fn mixer_table_mut(&mut self) -> &mut EnumMap<InputChannels, EnumMap<OutputChannels, u16>> {
        &mut self.mixer_table
    }

    pub fn channel_volume(&self, channel: FullChannelList) -> u8 {
        self.volume_table[channel]
    }

    pub fn set_channel_volume(&mut self, channel: FullChannelList, volume: u8) -> Result<()> {
        // We don't need to validate this, a u8 is between 0 and 255 already :)
        self.volume_table[channel] = volume;
        Ok(())
    }
}

#[derive(Debug, EnumIter, Enum, EnumProperty, Clone, Copy)]
pub enum InputChannels {
    #[strum(props(Name = "mic"))]
    Mic,

    #[strum(props(Name = "chat"))]
    Chat,

    #[strum(props(Name = "music"))]
    Music,

    #[strum(props(Name = "game"))]
    Game,

    #[strum(props(Name = "console"))]
    Console,

    #[strum(props(Name = "lineIn"))]
    LineIn,

    #[strum(props(Name = "system"))]
    System,

    #[strum(props(Name = "sample"))]
    Sample,
}

#[derive(Debug, EnumIter, Enum, EnumProperty)]
pub enum OutputChannels {
    #[strum(props(Name = "HP"))]
    Headphones,

    #[strum(props(Name = "Stream"))]
    Broadcast,

    #[strum(props(Name = "LineOut"))]
    LineOut,

    #[strum(props(Name = "Chat"))]
    ChatMic,

    #[strum(props(Name = "Sampler"))]
    Sampler,
}

/**
 * There are a couple of volumes that aren't part of the general mixer, so this needs mapping..
 */
#[derive(Copy, Clone, Debug, Enum, EnumIter, EnumProperty)]
pub enum FullChannelList {
    // Base Mixer Channels
    #[strum(props(Name = "mic", faderIndex = "0"))]
    Mic,

    #[strum(props(Name = "chat", faderIndex = "1"))]
    Chat,

    #[strum(props(Name = "music", faderIndex = "2"))]
    Music,

    #[strum(props(Name = "game", faderIndex = "3"))]
    Game,

    #[strum(props(Name = "console", faderIndex = "4"))]
    Console,

    #[strum(props(Name = "lineIn", faderIndex = "5"))]
    LineIn,

    #[strum(props(Name = "system", faderIndex = "6"))]
    System,

    #[strum(props(Name = "sample", faderIndex = "7"))]
    Sample,

    // Extra Volume Mixers
    #[strum(props(Name = "headphone", faderIndex = "8"))]
    Headphones,

    // Not Present in the Fader 'Source' List..
    #[strum(props(Name = "mic2headphoneSub", faderIndex = "-1"))]
    MicMonitor,

    #[strum(props(Name = "lineOut", faderIndex = "9"))]
    LineOut,
}
