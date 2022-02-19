use std::collections::HashMap;
use std::fs::File;

use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

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

impl RootElement {
    pub fn new() -> Self {
        Self {
            version: 0,
            loudness: 0,
            device: 0,
        }
    }

    pub fn parse_root(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name == "version" {
                self.version = attr.value.parse()?;
                continue;
            }

            if attr.name.local_name == "loudness" {
                self.loudness = attr.value.parse()?;
                continue;
            }

            if attr.name.local_name == "device" {
                self.device = attr.value.parse()?;
            }
        }

        Ok(())
    }

    pub fn write_initial(
        &self,
        writer: &mut EventWriter<&mut File>,
    ) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder = XmlWriterEvent::start_element("ValueTreeRoot");

        // Create the hashmap of values..
        let mut attributes: HashMap<String, String> = HashMap::default();
        //attributes.insert("version".to_string(), format!("{}", self.version));
        attributes.insert("version".to_string(), "2".to_string());
        attributes.insert("loudness".to_string(), format!("{}", self.loudness));
        attributes.insert("device".to_string(), format!("{}", self.device));

        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }
        writer.write(element)?;

        // WE DO NOT CLOSE THE ELEMENT HERE!!
        Ok(())
    }

    pub fn write_final(
        &self,
        writer: &mut EventWriter<&mut File>,
    ) -> Result<(), xml::writer::Error> {
        // The AppTree element seems to just be a tag containing the device id..
        let mut element: StartElementBuilder = XmlWriterEvent::start_element("AppTree");

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("ConnectedDeviceID".to_string(), format!("{}", &self.device));
        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;
        writer.write(XmlWriterEvent::end_element())?;

        // Finally, close the ValueTreeRoot
        writer.write(XmlWriterEvent::end_element())?;
        Ok(())
    }

    pub fn get_version(&self) -> u8 {
        self.version
    }
}
