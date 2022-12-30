use std::collections::HashMap;
use std::os::raw::c_float;

use crate::profile::Attribute;
use anyhow::{anyhow, Result};

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
    makeup_gain: i8,
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
            ratio: 9,
            attack: 1,
            release: 9,
            makeup_gain: 0,
        }
    }

    pub fn parse_compressor(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            if attr.name == "MIC_COMP_THRESHOLD" {
                self.set_threshold(attr.value.parse::<c_float>()? as i8)?;
                continue;
            }

            if attr.name == "MIC_COMP_RATIO" {
                let value = attr.value.parse::<c_float>()?;
                if value > 14. {
                    continue;
                }
                self.set_ratio(value as u8)?;
                continue;
            }

            if attr.name == "MIC_COMP_ATTACK" {
                let value = attr.value.parse::<c_float>()?;
                if value > 19. {
                    continue;
                }
                self.set_attack(value as u8)?;
                continue;
            }

            if attr.name == "MIC_COMP_RELEASE" {
                let value = attr.value.parse::<c_float>()?;
                if value > 19. {
                    continue;
                }
                self.set_release(value as u8)?;
                continue;
            }

            if attr.name == "MIC_COMP_MAKEUPGAIN" {
                self.set_makeup_gain(attr.value.parse::<c_float>()? as i8)?;
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
    pub fn makeup(&self) -> i8 {
        self.makeup_gain
    }

    // TODO: We should probably Enum some of these for clarity.
    pub fn set_threshold(&mut self, threshold: i8) -> Result<()> {
        if !(-40..=0).contains(&threshold) {
            return Err(anyhow!("Compressor Threshold must be between -40 and 0 dB"));
        }

        self.threshold = threshold;
        Ok(())
    }

    pub fn set_ratio(&mut self, ratio: u8) -> Result<()> {
        if ratio > 14 {
            return Err(anyhow!("Compressor Ratio should be between 0 and 14"));
        }
        self.ratio = ratio;
        Ok(())
    }
    pub fn set_attack(&mut self, attack: u8) -> Result<()> {
        if attack > 19 {
            return Err(anyhow!("Compressor Attack should be between 0 and 19"));
        }
        self.attack = attack;
        Ok(())
    }
    pub fn set_release(&mut self, release: u8) -> Result<()> {
        if release > 19 {
            return Err(anyhow!("Compressor Release should be between 0 and 19"));
        }
        self.release = release;
        Ok(())
    }
    pub fn set_makeup_gain(&mut self, makeup_gain: i8) -> Result<()> {
        if !(-6..=24).contains(&makeup_gain) {
            return Err(anyhow!("Makeup Gain should be between -6 and 24dB"));
        }
        self.makeup_gain = makeup_gain;
        Ok(())
    }
}
