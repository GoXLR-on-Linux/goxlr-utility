use crate::profile::Attribute;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::os::raw::c_float;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),

    #[error("Expected float: {0}")]
    ExpectedFloat(#[from] std::num::ParseFloatError),
}

#[derive(Debug)]
pub struct Gate {
    amount: u8,
    threshold: i8,
    attack: u8,
    release: u8,
    enabled: bool,
    attenuation: u8,
}

impl Default for Gate {
    fn default() -> Self {
        Self::new()
    }
}

impl Gate {
    pub fn new() -> Self {
        Self {
            amount: 0,
            threshold: -30,
            attack: 0,
            release: 19,
            enabled: false,
            attenuation: 100,
        }
    }

    pub fn parse_gate(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            if attr.name == "MIC_GATE_MACRO_AMOUNT" {
                self.amount = attr.value.parse::<c_float>()? as u8;
                continue;
            }

            if attr.name == "MIC_GATE_THRESOLD" {
                self.set_threshold(attr.value.parse::<c_float>()? as i8)?;
                continue;
            }

            if attr.name == "MIC_GATE_ATTACK" {
                let value = attr.value.parse::<c_float>()?;
                if value > 45. {
                    // If the value is out of range, use the default.
                    continue;
                }
                self.set_attack(value as u8)?;
                continue;
            }

            if attr.name == "MIC_GATE_RELEASE" {
                let value = attr.value.parse::<c_float>()?;
                if value > 45. {
                    continue;
                }
                self.set_release(value as u8)?;
                continue;
            }

            // Read and handle as a percentage.
            if attr.name == "MIC_GATE_ATTEN" {
                self.set_attenuation(attr.value.parse::<c_float>()? as u8)?;
                continue;
            }

            if attr.name == "MIC_GATE_ENABLE" {
                if attr.value == "0" {
                    self.set_enabled(false)?;
                } else {
                    self.set_enabled(true)?;
                }
                continue;
            }
        }

        Ok(())
    }

    pub fn write_gate(&self, attributes: &mut HashMap<String, String>) {
        attributes.insert(
            "MIC_GATE_MACRO_AMOUNT".to_string(),
            format!("{}", self.amount),
        );
        attributes.insert(
            "MIC_GATE_THRESOLD".to_string(),
            format!("{}", self.threshold),
        );
        attributes.insert("MIC_GATE_ATTACK".to_string(), format!("{}", self.attack));
        attributes.insert("MIC_GATE_RELEASE".to_string(), format!("{}", self.release));
        attributes.insert(
            "MIC_GATE_ENABLE".to_string(),
            format!("{}", self.enabled as u8),
        );
        attributes.insert(
            "MIC_GATE_ATTEN".to_string(),
            //format!("{}", ((self.attenuation as f32 / -61 as f32) * 100 as f32) as u8),
            format!("{}", self.attenuation),
        );
    }

    pub fn amount(&self) -> u8 {
        self.amount
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn threshold(&self) -> i8 {
        self.threshold
    }
    pub fn attack(&self) -> u8 {
        self.attack
    }
    pub fn release(&self) -> u8 {
        self.release
    }
    pub fn attenuation(&self) -> u8 {
        self.attenuation
    }

    pub fn set_amount(&mut self, amount: u8) -> Result<()> {
        // TODO: Is amount actually amount? O_o
        self.amount = amount;
        Ok(())
    }
    pub fn set_threshold(&mut self, threshold: i8) -> Result<()> {
        if !(-59..=0).contains(&threshold) {
            return Err(anyhow!("Gate Threshold must be between -59 and 0"));
        }
        self.threshold = threshold;
        Ok(())
    }
    pub fn set_attack(&mut self, attack: u8) -> Result<()> {
        if attack > 45 {
            return Err(anyhow!("Gate Attack must be 45 or less"));
        }
        self.attack = attack;
        Ok(())
    }
    pub fn set_release(&mut self, release: u8) -> Result<()> {
        if release > 45 {
            return Err(anyhow!("Gate Release must be 45 or less"));
        }

        self.release = release;
        Ok(())
    }
    pub fn set_enabled(&mut self, enabled: bool) -> Result<()> {
        self.enabled = enabled;
        Ok(())
    }
    pub fn set_attenuation(&mut self, attenuation: u8) -> Result<()> {
        if attenuation > 100 {
            return Err(anyhow!("Gate Attenuation must be a percentage"));
        }
        self.attenuation = attenuation;
        Ok(())
    }
}
