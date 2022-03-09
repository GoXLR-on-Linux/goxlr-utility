use std::collections::HashMap;
use std::os::raw::c_float;
use xml::attribute::OwnedAttribute;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),

    #[error("Expected float: {0}")]
    ExpectedFloat(#[from] std::num::ParseFloatError),
}

pub struct Gate {
    amount: u8,
    threshold: i8,
    attack: u8,
    release: u8,
    enabled: bool,
    attenuation: u8
}

impl Gate {
    pub fn new() -> Self {
        Self {
            amount: 0,
            threshold: 0,
            attack: 0,
            release: 0,
            enabled: false,
            attenuation: 0
        }
    }

    pub fn parse_gate(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name == "MIC_GATE_MACRO_AMOUNT" {
                self.amount = attr.value.parse::<c_float>()? as u8;
                continue;
            }

            if attr.name.local_name == "MIC_GATE_THRESOLD" {
                self.threshold = attr.value.parse::<c_float>()? as i8;
                continue;
            }

            if attr.name.local_name == "MIC_GATE_ATTACK" {
                self.attack = attr.value.parse::<c_float>()? as u8;
                continue;
            }

            if attr.name.local_name == "MIC_GATE_RELEASE" {
                self.release = attr.value.parse::<c_float>()? as u8;
                continue;
            }

            if attr.name.local_name == "MIC_GATE_ENABLE" {
                if attr.value == "0" {
                    self.enabled = false;
                } else {
                    self.enabled = true;
                }
                continue;
            }

            if attr.name.local_name == "MIC_GATE_ATTEN" {
                self.attenuation = attr.value.parse::<c_float>()? as u8;
                continue;
            }
        }

        Ok(())
    }

    pub fn write_gate(&self, attributes: &mut HashMap<String, String>) {
        attributes.insert("MIC_GATE_MACRO_AMOUNT".to_string(), format!("{}", self.amount));
        attributes.insert("MIC_GATE_THRESOLD".to_string(), format!("{}", self.threshold));
        attributes.insert("MIC_GATE_ATTACK".to_string(), format!("{}", self.attack));
        attributes.insert("MIC_GATE_RELEASE".to_string(), format!("{}", self.release));
        attributes.insert("MIC_GATE_ENABLE".to_string(), format!("{}", self.enabled as u8));
        attributes.insert("MIC_GATE_ATTEN".to_string(), format!("{}", self.attenuation));
    }
}
