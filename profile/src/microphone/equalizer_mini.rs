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

// Mini processes mostly the same way as the main, although has a smaller frequency set.
#[derive(Debug)]
pub struct EqualizerMini {
    eq_90h_gain: i8,
    eq_250h_gain: i8,
    eq_500h_gain: i8,
    eq_1k_gain: i8,
    eq_3k_gain: i8,
    eq_8k_gain: i8,

    eq_90h_freq: f32,
    eq_250h_freq: f32,
    eq_500h_freq: f32,
    eq_1k_freq: f32,
    eq_3k_freq: f32,
    eq_8k_freq: f32,
}

impl EqualizerMini {
    pub fn new() -> Self {
        Self {
            eq_90h_gain: 0,
            eq_250h_gain: 0,
            eq_500h_gain: 0,
            eq_1k_gain: 0,
            eq_3k_gain: 0,
            eq_8k_gain: 0,

            // The mini is weird, trust these defaults..
            eq_90h_freq: 90.0,
            eq_250h_freq: 160.0,
            eq_500h_freq: 480.0,
            eq_1k_freq: 1500.0,
            eq_3k_freq: 4500.0,
            eq_8k_freq: 7800.0,
        }
    }

    // TODO: These may not need to be handled as floats..
    pub fn parse_equaliser(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name == "MIC_MINI_EQ_90HZ_GAIN" {
                self.eq_90h_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_MINI_EQ_250HZ_GAIN" {
                self.eq_250h_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_MINI_EQ_500HZ_GAIN" {
                self.eq_500h_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_MINI_EQ_1KHZ_GAIN" {
                self.eq_1k_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_MINI_EQ_3KHZ_GAIN" {
                self.eq_3k_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_MINI_EQ_8KHZ_GAIN" {
                self.eq_8k_gain = attr.value.parse::<c_float>()? as i8
            }

            if attr.name.local_name == "MIC_MINI_EQ_90HZ_F" {
                self.eq_90h_freq = f32::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_MINI_EQ_250HZ_F" {
                self.eq_250h_freq = f32::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_MINI_EQ_500HZ_F" {
                self.eq_500h_freq = f32::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_MINI_EQ_1KHZ_F" {
                self.eq_1k_freq = f32::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_MINI_EQ_3KHZ_F" {
                self.eq_3k_freq = f32::from_str(attr.value.as_str())?;
            }

            if attr.name.local_name == "MIC_MINI_EQ_8KHZ_F" {
                self.eq_8k_freq = f32::from_str(attr.value.as_str())?;
            }
        }

        Ok(())
    }

    pub fn write_equaliser(&self, attributes: &mut HashMap<String, String>) {
        attributes.insert(
            "MIC_MINI_EQ_90HZ_GAIN".to_string(),
            format!("{}", self.eq_90h_gain),
        );
        attributes.insert(
            "MIC_MINI_EQ_250HZ_GAIN".to_string(),
            format!("{}", self.eq_250h_gain),
        );
        attributes.insert(
            "MIC_MINI_EQ_500HZ_GAIN".to_string(),
            format!("{}", self.eq_500h_gain),
        );
        attributes.insert(
            "MIC_MINI_EQ_1KHZ_GAIN".to_string(),
            format!("{}", self.eq_1k_gain),
        );
        attributes.insert(
            "MIC_MINI_EQ_3KHZ_GAIN".to_string(),
            format!("{}", self.eq_3k_gain),
        );
        attributes.insert(
            "MIC_MINI_EQ_8KHZ_GAIN".to_string(),
            format!("{}", self.eq_8k_gain),
        );

        attributes.insert(
            "MIC_MINI_EQ_90HZ_F".to_string(),
            format!("{}", self.eq_90h_freq),
        );
        attributes.insert(
            "MIC_MINI_EQ_250HZ_F".to_string(),
            format!("{}", self.eq_250h_freq)
        );
        attributes.insert(
            "MIC_MINI_EQ_500HZ_F".to_string(),
            format!("{}", self.eq_500h_freq),
        );
        attributes.insert(
            "MIC_MINI_EQ_1KHZ_F".to_string(),
            format!("{}", self.eq_1k_freq),
        );
        attributes.insert(
            "MIC_MINI_EQ_3KHZ_F".to_string(),
            format!("{}", self.eq_3k_freq),
        );
        attributes.insert(
            "MIC_MINI_EQ_8KHZ_F".to_string(),
            format!("{}", self.eq_8k_freq),
        );
    }


    pub fn eq_90h_gain(&self) -> i8 {
        self.eq_90h_gain
    }
    pub fn eq_250h_gain(&self) -> i8 {
        self.eq_250h_gain
    }
    pub fn eq_500h_gain(&self) -> i8 {
        self.eq_500h_gain
    }
    pub fn eq_1k_gain(&self) -> i8 {
        self.eq_1k_gain
    }
    pub fn eq_3k_gain(&self) -> i8 {
        self.eq_3k_gain
    }
    pub fn eq_8k_gain(&self) -> i8 {
        self.eq_8k_gain
    }
    pub fn eq_90h_freq(&self) -> f32 {
        self.eq_90h_freq
    }
    pub fn eq_250h_freq(&self) -> f32 {
        self.eq_250h_freq
    }
    pub fn eq_500h_freq(&self) -> f32 {
        self.eq_500h_freq
    }
    pub fn eq_1k_freq(&self) -> f32 {
        self.eq_1k_freq
    }
    pub fn eq_3k_freq(&self) -> f32 {
        self.eq_3k_freq
    }
    pub fn eq_8k_freq(&self) -> f32 {
        self.eq_8k_freq
    }

    pub fn set_eq_90h_gain(&mut self, eq_90h_gain: i8) {
        self.eq_90h_gain = eq_90h_gain;
    }
    pub fn set_eq_250h_gain(&mut self, eq_250h_gain: i8) {
        self.eq_250h_gain = eq_250h_gain;
    }
    pub fn set_eq_500h_gain(&mut self, eq_500h_gain: i8) {
        self.eq_500h_gain = eq_500h_gain;
    }
    pub fn set_eq_1k_gain(&mut self, eq_1k_gain: i8) {
        self.eq_1k_gain = eq_1k_gain;
    }
    pub fn set_eq_3k_gain(&mut self, eq_3k_gain: i8) {
        self.eq_3k_gain = eq_3k_gain;
    }
    pub fn set_eq_8k_gain(&mut self, eq_8k_gain: i8) {
        self.eq_8k_gain = eq_8k_gain;
    }
    pub fn set_eq_90h_freq(&mut self, eq_90h_freq: f32) {
        self.eq_90h_freq = eq_90h_freq;
    }
    pub fn set_eq_250h_freq(&mut self, eq_250h_freq: f32) {
        self.eq_250h_freq = eq_250h_freq;
    }
    pub fn set_eq_500h_freq(&mut self, eq_500h_freq: f32) {
        self.eq_500h_freq = eq_500h_freq;
    }
    pub fn set_eq_1k_freq(&mut self, eq_1k_freq: f32) {
        self.eq_1k_freq = eq_1k_freq;
    }
    pub fn set_eq_3k_freq(&mut self, eq_3k_freq: f32) {
        self.eq_3k_freq = eq_3k_freq;
    }
    pub fn set_eq_8k_freq(&mut self, eq_8k_freq: f32) {
        self.eq_8k_freq = eq_8k_freq;
    }
}
