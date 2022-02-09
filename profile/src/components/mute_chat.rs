use std::collections::HashMap;
use std::fs::File;

use enum_map::Enum;
use strum::EnumProperty;
use xml::attribute::OwnedAttribute;
use xml::EventWriter;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;

use crate::components::colours::{ColourMap, ColourState};
use crate::components::mute::MuteFunction;
use crate::components::mute_chat::CoughToggle::HOLD;

/**
 * These have no special properties, they are literally just button colours..
 */
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
            blink: ColourState::OFF,
            cough_behaviour: CoughToggle::HOLD,
            cough_mute_source: MuteFunction::MUTE_ALL,
            cough_button_on: false
        }
    }

    pub fn parse_mute_chat(&mut self, attributes: &Vec<OwnedAttribute>) {
        for attr in attributes {
            if attr.name.local_name == "micIsAnActiveFader" {
                self.mic_fader_id = attr.value.parse().unwrap();
                continue;
            }

            if attr.name.local_name == "coughButtonToggleSetting" {
                self.cough_behaviour = if attr.value == "0" { CoughToggle::HOLD } else { CoughToggle::TOGGLE };
                continue;
            }

            if attr.name.local_name == "coughButtonMuteSourceSelection" {
                self.cough_mute_source = MuteFunction::from_usize(attr.value.parse().unwrap());
                continue;
            }

            if attr.name.local_name == "coughButtonIsOn" {
                self.cough_button_on = if attr.value == "0" { false } else { true };
                continue;
            }

            if attr.name.local_name == "blink" {
                self.blink = if attr.value == "0" { ColourState::OFF } else { ColourState::ON };
                continue;
            }

            if !self.colour_map.read_colours(&attr) {
                println!("[{}] Unparsed Attribute: {}", self.element_name, attr.name);
            }
        }
    }

    pub fn write_mute_chat(&self, writer: &mut EventWriter<&mut File>) {
        let mut element: StartElementBuilder = XmlWriterEvent::start_element(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();

        attributes.insert("micIsAnActiveFader".to_string(), format!("{}", self.mic_fader_id));
        attributes.insert("coughButtonToggleSetting".to_string(), if self.cough_behaviour == HOLD { "0".to_string() } else { "1".to_string() });
        attributes.insert("coughButtonMuteSourceSelection".to_string(), self.cough_mute_source.get_str("uiIndex").unwrap().to_string());
        attributes.insert("coughButtonIsOn".to_string(), if self.cough_button_on { "1".to_string() } else { "0".to_string() });
        attributes.insert("blink".to_string(), if self.blink == ColourState::OFF { "0".to_string() } else { "1".to_string() });

        self.colour_map.write_colours(&mut attributes);

        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element);
        writer.write(XmlWriterEvent::end_element());
    }
}

#[derive(PartialEq)]
enum CoughToggle {
    HOLD,
    TOGGLE
}