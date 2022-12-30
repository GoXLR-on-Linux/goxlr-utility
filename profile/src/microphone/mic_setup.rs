use crate::profile::Attribute;
use anyhow::{anyhow, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Writer;
use std::collections::HashMap;
use std::io::Write;
use std::str::FromStr;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),
}

#[derive(Debug)]
pub struct MicSetup {
    mic_type: u8,

    // These are super weird, in the config they're stored as dB * 65536!
    dynamic_mic_gain: u16,
    condenser_mic_gain: u16,
    trs_mic_gain: u16,
}

impl Default for MicSetup {
    fn default() -> Self {
        Self::new()
    }
}

impl MicSetup {
    pub fn new() -> Self {
        Self {
            mic_type: 0,
            dynamic_mic_gain: 0,
            condenser_mic_gain: 0,
            trs_mic_gain: 0,
        }
    }

    pub fn parse_config(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            if attr.name == "MIC_TYPE" {
                self.set_mic_type(u8::from_str(attr.value.as_str())?)?;
                continue;
            }

            if attr.name == "DYNAMIC_MIC_GAIN" {
                self.set_dynamic_mic_gain((u32::from_str(attr.value.as_str())? / 65536) as u16)?;
                continue;
            }

            if attr.name == "CONDENSER_MIC_GAIN" {
                self.set_condenser_mic_gain((u32::from_str(attr.value.as_str())? / 65536) as u16)?;
                continue;
            }

            if attr.name == "TRS_MIC_GAIN" {
                self.set_trs_mic_gain((u32::from_str(attr.value.as_str())? / 65536) as u16)?;
                continue;
            }
        }

        Ok(())
    }

    pub fn write_config<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new("setupTreeMicProfile");

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("MIC_TYPE".to_string(), format!("{}", self.mic_type));
        attributes.insert(
            "DYNAMIC_MIC_GAIN".to_string(),
            format!("{}", (self.dynamic_mic_gain as u32 * 65536)),
        );
        attributes.insert(
            "CONDENSER_MIC_GAIN".to_string(),
            format!("{}", (self.condenser_mic_gain as u32 * 65536)),
        );
        attributes.insert(
            "TRS_MIC_GAIN".to_string(),
            format!("{}", (self.trs_mic_gain as u32 * 65536)),
        );

        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Empty(elem))?;
        Ok(())
    }

    pub fn mic_type(&self) -> u8 {
        self.mic_type
    }

    pub fn dynamic_mic_gain(&self) -> u16 {
        self.dynamic_mic_gain
    }

    pub fn condenser_mic_gain(&self) -> u16 {
        self.condenser_mic_gain
    }

    pub fn trs_mic_gain(&self) -> u16 {
        self.trs_mic_gain
    }

    // TODO: Enum this.
    pub fn set_mic_type(&mut self, mic_type: u8) -> Result<()> {
        if mic_type > 2 {
            return Err(anyhow!("Mic Type should be between 0 and 2"));
        }
        self.mic_type = mic_type;
        Ok(())
    }

    pub fn set_dynamic_mic_gain(&mut self, dynamic_mic_gain: u16) -> Result<()> {
        if dynamic_mic_gain > 72 {
            return Err(anyhow!("Gain should be between 0 and 72dB"));
        }
        self.dynamic_mic_gain = dynamic_mic_gain;
        Ok(())
    }
    pub fn set_condenser_mic_gain(&mut self, condenser_mic_gain: u16) -> Result<()> {
        if condenser_mic_gain > 72 {
            return Err(anyhow!("Gain should be between 0 and 72dB"));
        }

        self.condenser_mic_gain = condenser_mic_gain;
        Ok(())
    }
    pub fn set_trs_mic_gain(&mut self, trs_mic_gain: u16) -> Result<()> {
        if trs_mic_gain > 72 {
            return Err(anyhow!("Gain should be between 0 and 72dB"));
        }

        self.trs_mic_gain = trs_mic_gain;
        Ok(())
    }
}
