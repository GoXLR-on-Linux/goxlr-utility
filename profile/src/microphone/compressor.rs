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

#[derive(Debug)]
pub struct Compressor {
    threshold: i8,
    ratio: u8,
    attack: u8,
    release: u8,
    makeup_gain: u8,
}

impl Compressor {
    pub fn new() -> Self {
        Self {
            threshold: 0,
            ratio: 0,
            attack: 0,
            release: 0,
            makeup_gain: 0,
        }
    }

    pub fn parse_compressor(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name == "MIC_COMP_THRESHOLD" {
                self.threshold = attr.value.parse::<c_float>()? as i8;
                continue;
            }

            if attr.name.local_name == "MIC_COMP_RATIO" {
                self.ratio = attr.value.parse::<c_float>()? as u8;
                continue;
            }

            if attr.name.local_name == "MIC_COMP_ATTACK" {
                self.attack = attr.value.parse::<c_float>()? as u8;
                continue;
            }

            if attr.name.local_name == "MIC_COMP_RELEASE" {
                self.release = attr.value.parse::<c_float>()? as u8;
                continue;
            }

            if attr.name.local_name == "MIC_COMP_MAKEUPGAIN" {
                self.makeup_gain = attr.value.parse::<c_float>()? as u8;
                continue;
            }
        }

        Ok(())
    }

    pub fn write_compressor(&self, attributes: &mut HashMap<String, String>) {
        attributes.insert(
            "MIC_COMP_THRESHOLD".to_string(),
            format!("{}", self.threshold),
        );
        attributes.insert("MIC_COMP_RATIO".to_string(), format!("{}", self.ratio));
        attributes.insert("MIC_COMP_ATTACK".to_string(), format!("{}", self.attack));
        attributes.insert("MIC_COMP_RELEASE".to_string(), format!("{}", self.release));
        attributes.insert(
            "MIC_COMP_MAKEUPGAIN".to_string(),
            format!("{}", self.makeup_gain),
        );
    }
}
