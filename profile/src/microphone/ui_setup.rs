use anyhow::Result;
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

impl Default for UiSetup {
    fn default() -> Self {
        Self::new()
    }
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

    pub fn parse_ui(&mut self, attributes: &[OwnedAttribute]) -> Result<()> {
        for attr in attributes {
            if attr.name.local_name == "eqAdvanced" {
                self.eq_advanced = matches!(attr.value.as_str(), "1");
                continue;
            }

            if attr.name.local_name == "compAdvanced" {
                self.comp_advanced = matches!(attr.value.as_str(), "1");
                continue;
            }

            if attr.name.local_name == "gateAdvanced" {
                self.gate_advanced = matches!(attr.value.as_str(), "1");
                continue;
            }

            if attr.name.local_name == "eqFineTuneEnabled" {
                self.eq_fine_tune = matches!(attr.value.as_str(), "1");
                continue;
            }
        }

        Ok(())
    }

    pub fn write_ui<W: Write>(&self, writer: &mut EventWriter<&mut W>) -> Result<()> {
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

    pub fn eq_advanced(&self) -> bool {
        self.eq_advanced
    }
    pub fn comp_advanced(&self) -> bool {
        self.comp_advanced
    }
    pub fn gate_advanced(&self) -> bool {
        self.gate_advanced
    }
    pub fn eq_fine_tune(&self) -> bool {
        self.eq_fine_tune
    }

    pub fn set_eq_advanced(&mut self, eq_advanced: bool) {
        self.eq_advanced = eq_advanced;
    }
    pub fn set_comp_advanced(&mut self, comp_advanced: bool) {
        self.comp_advanced = comp_advanced;
    }
    pub fn set_gate_advanced(&mut self, gate_advanced: bool) {
        self.gate_advanced = gate_advanced;
    }
    pub fn set_eq_fine_tune(&mut self, eq_fine_tune: bool) {
        self.eq_fine_tune = eq_fine_tune;
    }
}
