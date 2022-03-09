use std::collections::HashMap;
use std::fs::File;
use std::str::FromStr;
use xml::attribute::OwnedAttribute;
use xml::EventWriter;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),
}

pub struct MicSetup {
    mic_type: u8,

    // These are super weird, in the config they're stored as dB * 65536!
    dynamic_mic_gain: u8,
    condenser_mic_gain: u8,
    trs_mic_gain: u8,
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

    pub fn parse_config(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name == "MIC_TYPE" {
                self.mic_type = u8::from_str(attr.value.as_str())?;
                continue;
            }

            if attr.name.local_name == "DYNAMIC_MIC_GAIN" {
                self.dynamic_mic_gain = (u32::from_str(attr.value.as_str())? / 65536) as u8;
                continue;
            }

            if attr.name.local_name == "CONDENSER_MIC_GAIN" {
                self.condenser_mic_gain = (u32::from_str(attr.value.as_str())? / 65536) as u8;
                continue;
            }

            if attr.name.local_name == "TRS_MIC_GAIN" {
                self.trs_mic_gain = (u32::from_str(attr.value.as_str())? / 65536) as u8;
                continue;
            }
        }

        Ok(())
    }

    pub fn write_config(&self, writer: &mut EventWriter<&mut File>) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder = XmlWriterEvent::start_element("setupTreeMicProfile");

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("MIC_TYPE".to_string(), format!("{}", self.mic_type));
        attributes.insert("DYNAMIC_MIC_GAIN".to_string(), format!("{}", (self.dynamic_mic_gain as u32 * 65536)));
        attributes.insert("CONDENSER_MIC_GAIN".to_string(), format!("{}", (self.condenser_mic_gain as u32 * 65536)));
        attributes.insert("TRS_MIC_GAIN".to_string(), format!("{}", (self.trs_mic_gain as u32 * 65536)));

        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;
        writer.write(XmlWriterEvent::end_element())?;
        Ok(())
    }
}
