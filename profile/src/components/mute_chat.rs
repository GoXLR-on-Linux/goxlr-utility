use std::collections::HashMap;
use std::io::Write;

use enum_map::Enum;
use strum::EnumProperty;

use anyhow::{anyhow, Result};

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
use crate::profile::Attribute;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Writer;
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
            cough_behaviour: Hold,
            cough_mute_source: MuteFunction::All,
            cough_button_on: false,
        }
    }

    pub fn parse_mute_chat(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            if attr.name == "micIsAnActiveFader" {
                self.mic_fader_id = attr.value.parse()?;
                continue;
            }

            if attr.name == "coughButtonToggleSetting" {
                self.cough_behaviour = if attr.value == "0" {
                    Hold
                } else {
                    CoughToggle::Toggle
                };
                continue;
            }

            if attr.name == "coughButtonMuteSourceSelection" {
                self.cough_mute_source = MuteFunction::from_usize(attr.value.parse()?);
                continue;
            }

            if attr.name == "coughButtonIsOn" {
                self.cough_button_on = attr.value != "0";
                continue;
            }

            if attr.name == "blink" {
                self.blink = ColourState::from_str(&attr.value)?;
                continue;
            }

            if !self.colour_map.read_colours(attr)? {
                println!("[{}] Unparsed Attribute: {}", self.element_name, attr.name);
            }
        }

        Ok(())
    }

    pub fn write_mute_chat<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new(self.element_name.as_str());

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
            elem.push_attribute((key.as_str(), value.as_str()));
        }

        writer.write_event(Event::Empty(elem))?;
        Ok(())
    }

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }
    pub fn colour_map_mut(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }
    pub fn is_cough_toggle(&self) -> bool {
        self.cough_behaviour == CoughToggle::Toggle
    }

    pub fn mic_fader_id(&self) -> u8 {
        self.mic_fader_id
    }
    pub fn blink(&self) -> &ColourState {
        &self.blink
    }
    pub fn cough_behaviour(&self) -> &CoughToggle {
        &self.cough_behaviour
    }

    pub fn cough_mute_source(&self) -> &MuteFunction {
        &self.cough_mute_source
    }
    pub fn cough_button_on(&self) -> bool {
        self.cough_button_on
    }

    pub fn set_blink(&mut self, blink: ColourState) {
        self.blink = blink;
    }
    pub fn set_blink_on(&mut self, blink: bool) {
        if blink {
            self.blink = ColourState::On;
        } else {
            self.blink = ColourState::Off;
        }
    }
    pub fn get_blink_on(&self) -> bool {
        self.blink == ColourState::On
    }

    pub fn set_cough_mute_source(&mut self, cough_mute_source: MuteFunction) {
        self.cough_mute_source = cough_mute_source;
    }
    pub fn set_cough_button_on(&mut self, cough_button_on: bool) {
        self.cough_button_on = cough_button_on;
    }
    pub fn get_cough_button_on(&self) -> bool {
        self.cough_button_on
    }

    pub fn set_mic_fader_id(&mut self, mic_fader_id: u8) -> Result<()> {
        if !(0..=4).contains(&mic_fader_id) {
            return Err(anyhow!("Mic Fader id should be between 0 and 4"));
        }

        self.mic_fader_id = mic_fader_id;
        Ok(())
    }

    pub fn clear_mic_fader_id(&mut self) {
        self.mic_fader_id = 4;
    }

    pub fn set_cough_behaviour(&mut self, cough_behaviour: CoughToggle) {
        self.cough_behaviour = cough_behaviour;
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum CoughToggle {
    Hold,
    Toggle,
}
