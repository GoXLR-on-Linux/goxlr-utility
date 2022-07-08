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

impl Default for Compressor {
    fn default() -> Self {
        Self::new()
    }
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

    pub fn threshold(&self) -> i8 {
        self.threshold
    }
    pub fn ratio(&self) -> u8 {
        self.ratio
    }
    pub fn attack(&self) -> u8 {
        self.attack
    }
    pub fn release(&self) -> u8 {
        self.release
    }
    pub fn makeup(&self) -> u8 {
        self.makeup_gain
    }

    pub fn set_threshold(&mut self, threshold: i8) {
        self.threshold = threshold;
    }
    pub fn set_ratio(&mut self, ratio: u8) {
        self.ratio = ratio;
    }
    pub fn set_attack(&mut self, attack: u8) {
        self.attack = attack;
    }
    pub fn set_release(&mut self, release: u8) {
        self.release = release;
    }
    pub fn set_makeup_gain(&mut self, makeup_gain: u8) {
        self.makeup_gain = makeup_gain;
    }
}
