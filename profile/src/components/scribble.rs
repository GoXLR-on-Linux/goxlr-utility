use std::collections::HashMap;
use std::io::Write;
use std::str::FromStr;

use anyhow::Result;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Writer;
use strum::EnumProperty;

use crate::components::colours::{Colour, ColourMap};
use crate::components::scribble::ScribbleStyle::{Inverted, Normal};
use crate::profile::Attribute;
use crate::Faders;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("[SCRIBBLE] Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),

    #[error("[SCRIBBLE] Expected float: {0}")]
    ExpectedFloat(#[from] std::num::ParseFloatError),

    #[error("[SCRIBBLE] Expected enum: {0}")]
    ExpectedEnum(#[from] strum::ParseError),

    #[error("[SCRIBBLE] Invalid colours: {0}")]
    InvalidColours(#[from] crate::components::colours::ParseError),
}

#[derive(Debug)]
pub struct Scribble {
    colour_map: ColourMap,

    // File provided to the GoXLR to handle (no path, just the filename)
    icon_file: Option<String>,

    // This normally is just the channel number, rendered in the top left.
    text_top_left: String,

    // Text to render at the bottom of the display..
    text_bottom_middle: String,

    // Size of the text..
    text_size: u8,

    // Alpha level of.. something.. It should be noted, that this value has *MORE* precision than
    // an f64 in the official app, so we'll lose a little here when saving, but precision that high
    // is pretty up there on the 'wtf' list :D

    // I'm pretty sure the value: 0.80000001192092895508 is supposed to be 0.8, but floating point
    // arithmetic got it..
    alpha: f64,

    // Inverted or otherwise..
    style: ScribbleStyle,

    // Filename in the .goxlr zip file to the prepared bitmap
    bitmap_file: String,
}

impl Scribble {
    pub fn new(fader: Faders) -> Self {
        let element_name = fader.get_str("scribbleContext").unwrap();
        let mut colour_map = ColourMap::new(element_name.to_string());
        colour_map.set_colour(0, Colour::fromrgb("00FFFF").unwrap());

        let text = match fader {
            Faders::A => "Mic",
            Faders::B => "Music",
            Faders::C => "Chat",
            Faders::D => "System",
        };

        Self {
            colour_map,
            icon_file: None,
            text_top_left: "".to_string(),
            text_bottom_middle: text.to_string(),
            text_size: 0,
            alpha: 0.0,
            style: Normal,
            bitmap_file: "".to_string(),
        }
    }

    pub fn parse_scribble(&mut self, attributes: &Vec<Attribute>) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.ends_with("iconFile") {
                if attr.value.clone() == "" {
                    self.icon_file = None;
                } else {
                    self.icon_file = Some(attr.value.clone());
                }
                continue;
            }

            if attr.name.ends_with("string0") {
                self.text_top_left.clone_from(&attr.value);
                continue;
            }

            if attr.name.ends_with("string1") {
                self.text_bottom_middle.clone_from(&attr.value);
                continue;
            }

            if attr.name.ends_with("alpha") {
                self.alpha = f64::from_str(attr.value.as_str())?;
                continue;
            }

            if attr.name.ends_with("textSize") {
                self.text_size = u8::from_str(attr.value.as_str())?;
                continue;
            }

            if attr.name.ends_with("inverted") {
                if attr.value == "0" {
                    self.style = Normal;
                } else {
                    self.style = Inverted;
                }
                continue;
            }

            if attr.name.ends_with("bitmap") {
                self.bitmap_file.clone_from(&attr.value);
                continue;
            }

            // Send the rest out for colouring..
            if !self.colour_map.read_colours(attr)? {
                println!("[SCRIBBLE] Unparsed Attribute: {}", attr.name);
            }
        }

        Ok(())
    }

    pub fn write_scribble<W: Write>(&self, writer: &mut Writer<W>, fader: Faders) -> Result<()> {
        let element_name = fader.get_str("scribbleContext").unwrap();
        let mut elem = BytesStart::new(element_name);

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert(
            format!("{element_name}iconFile"),
            if self.icon_file.is_none() {
                "".to_string()
            } else {
                self.icon_file.clone().unwrap()
            },
        );
        attributes.insert(format!("{element_name}string0"), self.text_top_left.clone());
        attributes.insert(
            format!("{element_name}string1"),
            self.text_bottom_middle.clone(),
        );
        attributes.insert(format!("{element_name}alpha"), format!("{}", self.alpha));
        attributes.insert(
            format!("{element_name}inverted"),
            if self.style == Normal { "0" } else { "1" }.parse()?,
        );
        attributes.insert(
            format!("{element_name}textSize"),
            format!("{}", self.text_size),
        );
        attributes.insert(format!("{element_name}bitmap"), self.bitmap_file.clone());

        self.colour_map
            .write_colours_with_prefix(element_name.into(), &mut attributes);

        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Empty(elem))?;
        Ok(())
    }

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }
    pub fn colour_map_mut(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }

    pub fn icon_file(&self) -> Option<String> {
        self.icon_file.clone()
    }
    pub fn text_top_left(&self) -> Option<String> {
        if self.text_top_left.is_empty() {
            return None;
        }
        Some(self.text_top_left.to_string())
    }
    pub fn text_bottom_middle(&self) -> Option<String> {
        if self.text_bottom_middle.is_empty() {
            return None;
        }
        Some(self.text_bottom_middle.to_string())
    }
    pub fn is_style_invert(&self) -> bool {
        self.style == Inverted
    }

    pub fn style(&self) -> &ScribbleStyle {
        &self.style
    }

    pub fn set_icon_file(&mut self, icon_file: Option<String>) {
        self.icon_file = icon_file;
    }
    pub fn set_text_top_left(&mut self, text_top_left: String) {
        self.text_top_left = text_top_left;
    }
    pub fn set_text_bottom_middle(&mut self, text_bottom_middle: String) {
        self.text_bottom_middle = text_bottom_middle;
    }

    pub fn set_scribble_inverted(&mut self, inverted: bool) {
        self.style = if inverted { Inverted } else { Normal }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum ScribbleStyle {
    Normal,
    Inverted,
}
