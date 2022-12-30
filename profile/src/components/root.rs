use std::collections::HashMap;
use std::io::Write;

use anyhow::Result;
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::Writer;

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

/**
 * These have no special properties, they are literally just button colours..
 */
#[derive(Debug)]
pub struct RootElement {
    // Ok.
    version: u8,
    loudness: u8,
    device: u64,
}

impl Default for RootElement {
    fn default() -> Self {
        Self::new()
    }
}

impl RootElement {
    pub fn new() -> Self {
        Self {
            version: 0,
            loudness: 0,
            device: 0,
        }
    }

    pub fn parse_root(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            if attr.name == "version" {
                self.version = attr.value.parse()?;
                continue;
            }

            if attr.name == "loudness" {
                self.loudness = attr.value.parse()?;
                continue;
            }

            if attr.name == "device" {
                self.device = attr.value.parse()?;
            }
        }

        Ok(())
    }

    pub fn write_initial<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new("ValueTreeRoot");

        // Create the hashmap of values..
        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("version".to_string(), "2".to_string());
        attributes.insert("loudness".to_string(), format!("{}", self.loudness));
        attributes.insert("device".to_string(), format!("{}", self.device));

        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }
        writer.write_event(Event::Start(elem))?;

        // WE DO NOT CLOSE THE ELEMENT HERE!!
        Ok(())
    }

    pub fn write_final<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new("AppTree");

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("ConnectedDeviceID".to_string(), format!("{}", &self.device));
        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Empty(elem))?;
        writer.write_event(Event::End(BytesEnd::new("ValueTreeRoot")))?;
        Ok(())
    }

    pub fn get_version(&self) -> u8 {
        self.version
    }
}
