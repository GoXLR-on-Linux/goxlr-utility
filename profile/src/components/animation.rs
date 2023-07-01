use crate::profile::Attribute;
use anyhow::Result;
use log::warn;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Writer;
use std::collections::HashMap;
use std::io::Write;
use std::os::raw::c_float;
use strum::{EnumIter, IntoEnumIterator};

#[derive(Debug, Default)]
pub struct AnimationTree {
    element_name: String,

    mode: AnimationMode,
    mod1: u8,
    mod2: u8,
    waterfall: WaterfallDirection,
}

impl AnimationTree {
    pub fn new(element_name: String) -> Self {
        Self {
            element_name,
            ..Default::default()
        }
    }

    pub fn parse_animation(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            if attr.name == "animationMode" {
                match AnimationMode::iter().nth(attr.value.parse()?) {
                    None => warn!("Unknown Animation Mode, using Default."),
                    Some(value) => self.mode = value,
                }
                continue;
            }
            if attr.name == "mod1" {
                self.mod1 = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "mod2" {
                self.mod2 = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name == "mod3" {
                match WaterfallDirection::iter().nth(attr.value.parse()?) {
                    None => warn!("Unknown Waterfall Mode, using Default."),
                    Some(value) => self.waterfall = value,
                }
                continue;
            }
            warn!("Unmatched Attribute: {}", attr.name);
        }

        Ok(())
    }

    pub fn write_animation<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        //<animationTree animationMode="3" mod1="39.0" mod2="39.0" mod3="0"/>

        let mut elem = BytesStart::new(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("animationTree".to_string(), format!("{}", self.mode as u8));
        attributes.insert("mod1".to_string(), format!("{}", self.mod1));
        attributes.insert("mod2".to_string(), format!("{}", self.mod1));
        attributes.insert("mod3".to_string(), format!("{}", self.waterfall as u8));

        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }
        writer.write_event(Event::Empty(elem))?;
        Ok(())
    }

    pub fn mode(&self) -> AnimationMode {
        self.mode
    }
    pub fn mod1(&self) -> u8 {
        self.mod1
    }
    pub fn mod2(&self) -> u8 {
        self.mod2
    }
    pub fn waterfall(&self) -> WaterfallDirection {
        self.waterfall
    }
}

#[derive(Debug, Default, Copy, Clone, EnumIter)]
pub enum AnimationMode {
    RetroRainbow,
    RainbowDark,
    RainbowBright,
    Simple,
    Ripple,

    #[default]
    None,
}

#[derive(Debug, Default, Copy, Clone, EnumIter)]
pub enum WaterfallDirection {
    #[default]
    Down,
    Up,
    Off,
}
