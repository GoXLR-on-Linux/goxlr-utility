use std::collections::HashMap;
use std::os::raw::c_float;
use std::str::FromStr;
use xml::attribute::OwnedAttribute;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),

    #[error("Expected float: {0}")]
    ExpectedFloat(#[from] std::num::ParseFloatError),
}

// The EQ has a crap load of values (20 total), we could consider splitting
// them into Gain and Freq to keep stuff tidy?
#[derive(Debug)]
pub struct Equalizer {
    eq_31h_gain: i8,
    eq_63h_gain: i8,
    eq_125h_gain: i8,
    eq_250h_gain: i8,
    eq_500h_gain: i8,
    eq_1k_gain: i8,
    eq_2k_gain: i8,
    eq_4k_gain: i8,
    eq_8k_gain: i8,
    eq_16k_gain: i8,

    eq_31h_freq: f64,
    eq_63h_freq: f64,
    eq_125h_freq: f64,
    eq_250h_freq: f64,
    eq_500h_freq: f64,
    eq_1k_freq: f64,
    eq_2k_freq: f64,
    eq_4k_freq: f64,
    eq_8k_freq: f64,
    eq_16k_freq: f64,
}

impl Equalizer {
    pub fn new() -> Self {
        Self {
            eq_31h_gain: 0,
            eq_63h_gain: 0,
            eq_125h_gain: 0,
            eq_250h_gain: 0,
            eq_500h_gain: 0,
            eq_1k_gain: 0,
            eq_2k_gain: 0,
            eq_4k_gain: 0,
            eq_8k_gain: 0,
            eq_16k_gain: 0,
            eq_31h_freq: 31.5,
            eq_63h_freq: 63.0,
            eq_125h_freq: 125.0,
            eq_250h_freq: 250.0,
            eq_500h_freq: 500.0,
            eq_1k_freq: 1000.0,
            eq_2k_freq: 2000.0,
            eq_4k_freq: 4000.0,
            eq_8k_freq: 8000.0,
            eq_16k_freq: 16000.0
        }
    }

    pub fn parse_equaliser(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name == "MIC_EQ_31.5HZ_GAIN" {
                self.eq_31h_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_EQ_63HZ_GAIN" {
                self.eq_63h_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_EQ_125HZ_GAIN" {
                self.eq_125h_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_EQ_250HZ_GAIN" {
                self.eq_250h_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_EQ_500HZ_GAIN" {
                self.eq_500h_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_EQ_1KHZ_GAIN" {
                self.eq_1k_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_EQ_2KHZ_GAIN" {
                self.eq_2k_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_EQ_4KHZ_GAIN" {
                self.eq_4k_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_EQ_8KHZ_GAIN" {
                self.eq_8k_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_EQ_16KHZ_GAIN" {
                self.eq_16k_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_EQ_31.5HZ_F" {
                self.eq_31h_freq = f64::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_EQ_63HZ_F" {
                self.eq_63h_freq = f64::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_EQ_125HZ_F" {
                self.eq_125h_freq = f64::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_EQ_250HZ_F" {
                self.eq_250h_freq = f64::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_EQ_500HZ_F" {
                self.eq_500h_freq = f64::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_EQ_1KHZ_F" {
                self.eq_1k_freq = f64::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_EQ_2KHZ_F" {
                self.eq_2k_freq = f64::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_EQ_4KHZ_F" {
                self.eq_4k_freq = f64::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_EQ_8KHZ_F" {
                self.eq_8k_freq = f64::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_EQ_16KHZ_F" {
                self.eq_16k_freq = f64::from_str(attr.value.as_str())?;
            }
        }

        Ok(())
    }

    pub fn write_equaliser(&self, attributes: &mut HashMap<String, String>) {
        attributes.insert("MIC_EQ_31.5HZ_GAIN".to_string(), format!("{}", self.eq_31h_gain));
        attributes.insert("MIC_EQ_63HZ_GAIN".to_string(), format!("{}", self.eq_63h_gain));
        attributes.insert("MIC_EQ_125HZ_GAIN".to_string(), format!("{}", self.eq_125h_gain));
        attributes.insert("MIC_EQ_250HZ_GAIN".to_string(), format!("{}", self.eq_250h_gain));
        attributes.insert("MIC_EQ_500HZ_GAIN".to_string(), format!("{}", self.eq_500h_gain));
        attributes.insert("MIC_EQ_1KHZ_GAIN".to_string(), format!("{}", self.eq_1k_gain));
        attributes.insert("MIC_EQ_2KHZ_GAIN".to_string(), format!("{}", self.eq_2k_gain));
        attributes.insert("MIC_EQ_4KHZ_GAIN".to_string(), format!("{}", self.eq_4k_gain));
        attributes.insert("MIC_EQ_8KHZ_GAIN".to_string(), format!("{}", self.eq_8k_gain));
        attributes.insert("MIC_EQ_16KHZ_GAIN".to_string(), format!("{}", self.eq_16k_gain));

        attributes.insert("MIC_EQ_31.5HZ_F".to_string(), format!("{}", self.eq_31h_freq));
        attributes.insert("MIC_EQ_63HZ_F".to_string(), format!("{}", self.eq_63h_freq));
        attributes.insert("MIC_EQ_125HZ_F".to_string(), format!("{}", self.eq_125h_freq));
        attributes.insert("MIC_EQ_250HZ_F".to_string(), format!("{}", self.eq_250h_freq));
        attributes.insert("MIC_EQ_500HZ_F".to_string(), format!("{}", self.eq_500h_freq));
        attributes.insert("MIC_EQ_1KHZ_F".to_string(), format!("{}", self.eq_1k_freq));
        attributes.insert("MIC_EQ_2KHZ_F".to_string(), format!("{}", self.eq_2k_freq));
        attributes.insert("MIC_EQ_4KHZ_F".to_string(), format!("{}", self.eq_4k_freq));
        attributes.insert("MIC_EQ_8KHZ_F".to_string(), format!("{}", self.eq_8k_freq));
        attributes.insert("MIC_EQ_16KHZ_F".to_string(), format!("{}", self.eq_16k_freq));
    }
}
