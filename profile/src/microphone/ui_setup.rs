use std::collections::HashMap;
use std::io::Write;
use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),
}

/**
 * This simply needs to exist for cross-platform compatibility, it's unlikely we'll use
 * these on Linux!
 */

#[derive(Debug)]
pub struct UiSetup {
    eq_advanced: bool,
    comp_advanced: bool,
    gate_advanced: bool,
    eq_fine_tune: bool,
}

impl UiSetup {
    pub fn new() -> Self {
        Self {
            eq_advanced: false,
            comp_advanced: false,
            gate_advanced: false,
            eq_fine_tune: false,
        }
    }

    pub fn parse_ui(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name == "eqAdvanced" {
                if attr.value == "1" {
                    self.eq_advanced = true;
                } else {
                    self.eq_advanced = false;
                }
                continue;
            }

            if attr.name.local_name == "compAdvanced" {
                if attr.value == "1" {
                    self.comp_advanced = true;
                } else {
                    self.comp_advanced = false;
                }
                continue;
            }

            if attr.name.local_name == "gateAdvanced" {
                if attr.value == "1" {
                    self.gate_advanced = true;
                } else {
                    self.gate_advanced = false;
                }
                continue;
            }

            if attr.name.local_name == "eqFineTuneEnabled" {
                if attr.value == "1" {
                    self.eq_fine_tune = true;
                } else {
                    self.eq_fine_tune = false;
                }
                continue;
            }
        }

        Ok(())
    }

    pub fn write_ui<W: Write>(&self, writer: &mut EventWriter<&mut W>) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder =
            XmlWriterEvent::start_element("micProfileUIMicProfile");

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert(
            "eqAdvanced".to_string(),
            format!("{}", self.eq_advanced as u8),
        );
        attributes.insert(
            "compAdvanced".to_string(),
            format!("{}", self.comp_advanced as u8),
        );
        attributes.insert(
            "gateAdvanced".to_string(),
            format!("{}", self.gate_advanced as u8),
        );
        attributes.insert(
            "eqFineTuneEnabled".to_string(),
            format!("{}", self.eq_fine_tune as u8),
        );

        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;
        writer.write(XmlWriterEvent::end_element())?;
        Ok(())
    }
}
