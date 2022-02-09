use std::collections::HashMap;
use std::fs::File;

use enum_map::{Enum, EnumMap};
use strum::{EnumIter, EnumProperty, IntoEnumIterator};
use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use crate::components::colours::ColourMap;

pub struct Mixers {
    mixer_table: EnumMap<InputChannels, EnumMap<OutputChannels, u16>>,
    volume_table: EnumMap<FullChannelList, u8>,
    colour_map: ColourMap,
}

impl Mixers {
    pub fn new() -> Self {
        Self {
            mixer_table: EnumMap::default(),
            volume_table: EnumMap::default(),
            colour_map: ColourMap::new("mixerTree".to_string()),
        }
    }

    pub fn parse_mixers(&mut self, attributes: &[OwnedAttribute]) {
        for attr in attributes {
            if attr.name.local_name.ends_with("Level") {
                let mut found = false;

                // Get the String key..
                let channel = attr.name.local_name.as_str();
                let channel = &channel[0..channel.len() - 5];

                let value: u8 = attr.value.parse().unwrap();

                // Find the channel from the Prefix..
                for volume in FullChannelList::iter() {
                    if volume.get_str("Name").unwrap() == channel {
                        // Set the value..
                        self.volume_table[volume] = value;
                        found = true;
                    }
                }

                if !found {
                    println!("Unable to find Channel: {}", channel);
                }
                continue;
            }

            if attr.name.local_name.contains("To") {
                // Extract the two sides of the string..
                let name = attr.name.local_name.as_str();

                let middle_index = name.find("To").unwrap();
                let input = &name[0..middle_index];
                let output = &name[middle_index + 2..];

                let value: u16 = attr.value.parse().unwrap();

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
                continue;
            }

            // Check to see if this is a colour related attribute..
            if !self.colour_map.read_colours(attr) {
                println!("[MIXER] Unparsed Attribute: {}", attr.name);
            }
        }
    }

    pub fn write_mixers(&self, writer: &mut EventWriter<&mut File>) {
        let mut element: StartElementBuilder = XmlWriterEvent::start_element("mixerTree");

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
            element = element.attr(key.as_str(), value.as_str());
        }

        // Write and close the tag...
        writer.write(element);
        writer.write(XmlWriterEvent::end_element());
    }
}

#[derive(Debug, EnumIter, Enum, EnumProperty)]
pub enum InputChannels {
    #[strum(props(Name = "mic"))]
    MIC,

    #[strum(props(Name = "chat"))]
    CHAT,

    #[strum(props(Name = "music"))]
    MUSIC,

    #[strum(props(Name = "game"))]
    GAME,

    #[strum(props(Name = "console"))]
    CONSOLE,

    #[strum(props(Name = "lineIn"))]
    LINE_IN,

    #[strum(props(Name = "system"))]
    SYSTEM,

    #[strum(props(Name = "sample"))]
    SAMPLE,
}

#[derive(Debug, EnumIter, Enum, EnumProperty)]
pub enum OutputChannels {
    #[strum(props(Name = "HP"))]
    HEADPHONES,

    #[strum(props(Name = "Stream"))]
    BROADCAST,

    #[strum(props(Name = "LineOut"))]
    LINE_OUT,

    #[strum(props(Name = "Chat"))]
    CHAT_MIC,

    #[strum(props(Name = "Sampler"))]
    SAMPLER,
}

/**
 * There are a couple of volumes that aren't part of the general mixer, so this needs mapping..
 */
#[derive(Debug, Enum, EnumIter, EnumProperty)]
pub enum FullChannelList {
    // Base Mixer Channels
    #[strum(props(Name = "mic", faderIndex = "0"))]
    MIC,

    #[strum(props(Name = "chat", faderIndex = "1"))]
    CHAT,

    #[strum(props(Name = "music", faderIndex = "2"))]
    MUSIC,

    #[strum(props(Name = "game", faderIndex = "3"))]
    GAME,

    #[strum(props(Name = "console", faderIndex = "4"))]
    CONSOLE,

    #[strum(props(Name = "lineIn", faderIndex = "5"))]
    LINE_IN,

    #[strum(props(Name = "system", faderIndex = "6"))]
    SYSTEM,

    #[strum(props(Name = "sample", faderIndex = "7"))]
    SAMPLE,

    // Extra Volume Mixers
    #[strum(props(Name = "headphone", faderIndex = "8"))]
    HEADPHONE,

    // Not Present in the Fader 'Source' List..
    #[strum(props(Name = "mic2headphoneSub", faderIndex = "-1"))]
    MIC_MONITOR,

    #[strum(props(Name = "lineOut", faderIndex = "9"))]
    LINE_OUT,
}
