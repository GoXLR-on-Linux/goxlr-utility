use std::collections::HashMap;
use std::fs::File;
use std::str::FromStr;

use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use crate::components::colours::ColourMap;
use crate::components::scribble::ScribbleStyle::{Inverted, Normal};

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
pub struct Scribble {
    element_name: String,
    colour_map: ColourMap,

    // File provided to the GoXLR to handle (no path, just the filename)
    icon_file: String,

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
    pub fn new(id: u8) -> Self {
        let element_name = format!("scribble{}", id);
        let colour_map = format!("scribble{}", id);
        Self {
            element_name,
            colour_map: ColourMap::new(colour_map),
            icon_file: "".to_string(),
            text_top_left: "".to_string(),
            text_bottom_middle: "".to_string(),
            text_size: 0,
            alpha: 0.0,
            style: ScribbleStyle::Normal,
            bitmap_file: "".to_string(),
        }
    }

    pub fn parse_scribble(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name.ends_with("iconFile") {
                self.icon_file = attr.value.clone();
                continue;
            }

            if attr.name.local_name.ends_with("string0") {
                self.text_top_left = attr.value.clone();
                continue;
            }

            if attr.name.local_name.ends_with("string1") {
                self.text_bottom_middle = attr.value.clone();
                continue;
            }

            if attr.name.local_name.ends_with("alpha") {
                self.alpha = f64::from_str(attr.value.as_str())?;
                continue;
            }

            if attr.name.local_name.ends_with("textSize") {
                self.text_size = u8::from_str(attr.value.as_str())?;
                continue;
            }

            if attr.name.local_name.ends_with("inverted") {
                if attr.value == "0" {
                    self.style = Normal;
                } else {
                    self.style = Inverted;
                }
                continue;
            }

            if attr.name.local_name.ends_with("bitmap") {
                self.bitmap_file = attr.value.clone();
                continue;
            }

            // Send the rest out for colouring..
            if !self.colour_map.read_colours(attr)? {
                println!("[SCRIBBLE] Unparsed Attribute: {}", attr.name);
            }
        }

        Ok(())
    }

    pub fn write_scribble(
        &self,
        writer: &mut EventWriter<&mut File>,
    ) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder =
            XmlWriterEvent::start_element(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert(
            format!("{}iconFile", self.element_name),
            self.icon_file.clone(),
        );
        attributes.insert(
            format!("{}string0", self.element_name),
            self.text_top_left.clone(),
        );
        attributes.insert(
            format!("{}string1", self.element_name),
            self.text_bottom_middle.clone(),
        );
        attributes.insert(
            format!("{}alpha", self.element_name),
            format!("{}", self.alpha),
        );
        attributes.insert(
            format!("{}inverted", self.element_name),
            if self.style == Normal { "0" } else { "1" }
                .parse()
                .unwrap(),
        );
        attributes.insert(
            format!("{}textSize", self.element_name),
            format!("{}", self.text_size),
        );
        attributes.insert(
            format!("{}bitmap", self.element_name),
            self.bitmap_file.clone(),
        );

        self.colour_map.write_colours(&mut attributes);

        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;
        writer.write(XmlWriterEvent::end_element())?;
        Ok(())
    }

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }
}

#[derive(PartialEq, Debug)]
enum ScribbleStyle {
    Normal,
    Inverted,
}
