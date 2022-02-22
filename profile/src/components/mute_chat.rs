use std::collections::HashMap;
use std::fs::File;

use enum_map::Enum;
use strum::EnumProperty;
use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use crate::components::colours::{ColourMap, ColourState};
use crate::components::mute::MuteFunction;
use crate::components::mute_chat::CoughToggle::Hold;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),

    #[error("Expected float: {0}")]
    ExpectedFloat(#[from] std::num::ParseFloatError),

    #[error("Expected enum: {0}")]
    ExpectedEnum(#[from] strum::ParseError),

    #[error("Invalid colours: {0}")]
    InvalidColours(#[from] crate::components::colours::ParseError),
}
use std::str::FromStr;

/**
 * These have no special properties, they are literally just button colours..
 */
#[derive(Debug)]
pub struct MuteChat {
    // Ok.
    element_name: String,
    colour_map: ColourMap,

    // ID of the fader the microphone is attached to (4 for 'none')
    mic_fader_id: u8,

    blink: ColourState,
    cough_behaviour: CoughToggle,
    cough_mute_source: MuteFunction,
    cough_button_on: bool,
}

impl MuteChat {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name.clone();
        Self {
            element_name,
            colour_map: ColourMap::new(colour_map),
            mic_fader_id: 4,
            blink: ColourState::Off,
            cough_behaviour: CoughToggle::Hold,
            cough_mute_source: MuteFunction::All,
            cough_button_on: false,
        }
    }

    pub fn parse_mute_chat(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name == "micIsAnActiveFader" {
                self.mic_fader_id = attr.value.parse()?;
                continue;
            }

            if attr.name.local_name == "coughButtonToggleSetting" {
                self.cough_behaviour = if attr.value == "0" {
                    CoughToggle::Hold
                } else {
                    CoughToggle::Toggle
                };
                continue;
            }

            if attr.name.local_name == "coughButtonMuteSourceSelection" {
                self.cough_mute_source = MuteFunction::from_usize(attr.value.parse()?);
                continue;
            }

            if attr.name.local_name == "coughButtonIsOn" {
                self.cough_button_on = attr.value != "0";
                continue;
            }

            if attr.name.local_name == "blink" {
                self.blink = ColourState::from_str(&attr.value)?;
                continue;
            }

            if !self.colour_map.read_colours(attr)? {
                println!("[{}] Unparsed Attribute: {}", self.element_name, attr.name);
            }
        }

        Ok(())
    }

    pub fn write_mute_chat(
        &self,
        writer: &mut EventWriter<&mut File>,
    ) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder =
            XmlWriterEvent::start_element(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();

        attributes.insert(
            "micIsAnActiveFader".to_string(),
            format!("{}", self.mic_fader_id),
        );
        attributes.insert(
            "coughButtonToggleSetting".to_string(),
            if self.cough_behaviour == Hold {
                "0".to_string()
            } else {
                "1".to_string()
            },
        );
        attributes.insert(
            "coughButtonMuteSourceSelection".to_string(),
            self.cough_mute_source
                .get_str("uiIndex")
                .unwrap()
                .to_string(),
        );
        attributes.insert(
            "coughButtonIsOn".to_string(),
            if self.cough_button_on {
                "1".to_string()
            } else {
                "0".to_string()
            },
        );
        attributes.insert("blink".to_string(), self.blink.to_string());

        self.colour_map.write_colours(&mut attributes);

        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;
        writer.write(XmlWriterEvent::end_element())?;
        Ok(())
    }

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }
}

#[derive(PartialEq, Debug)]
enum CoughToggle {
    Hold,
    Toggle,
}
