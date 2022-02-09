use std::collections::HashMap;
use std::fs::File;
use std::os::raw::c_float;

use enum_map::EnumMap;
use strum::{EnumIter, EnumProperty, IntoEnumIterator};
use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use crate::components::colours::ColourMap;
use crate::components::megaphone::Preset;
use crate::components::megaphone::Preset::{Preset1, Preset2, Preset3, Preset4, Preset5, Preset6};
use crate::components::robot::RobotStyle::ROBOT_1;

/**
 * This is relatively static, main tag contains standard colour mapping, subtags contain various
 * presets, we'll use an EnumMap to define the 'presets' as they'll be useful for the other various
 * 'types' of presets (encoders and effects).
 */
pub struct RobotEffectBase {
    colour_map: ColourMap,
    preset_map: EnumMap<Preset, RobotEffect>,
}

impl RobotEffectBase {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name;
        Self {
            colour_map: ColourMap::new(colour_map),
            preset_map: EnumMap::default(),
        }
    }

    pub fn parse_robot_root(&mut self, attributes: &[OwnedAttribute]) {
        for attr in attributes {
            if !self.colour_map.read_colours(attr) {
                println!("[robotEffect] Unparsed Attribute: {}", attr.name);
            }
        }
    }

    pub fn parse_robot_preset(&mut self, id: u8, attributes: &[OwnedAttribute]) {
        let mut preset = RobotEffect::new();
        for attr in attributes {
            if attr.name.local_name == "robotEffectstate" {
                if attr.value == "1" {
                    preset.state = true;
                } else {
                    preset.state = false
                }
                continue;
            }
            if attr.name.local_name == "ROBOT_STYLE" {
                for style in RobotStyle::iter() {
                    if style.get_str("uiIndex").unwrap() == attr.value {
                        preset.style = style;
                        break;
                    }
                }
                continue;
            }

            /* Same as Microphone, I haven't seen any random float values in the config for robot
             * but I'm not gonna rule it out.. */

            if attr.name.local_name == "ROBOT_SYNTHOSC_PULSEWIDTH" {
                preset.synthosc_pulse_width = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "ROBOT_SYNTHOSC_WAVEFORM" {
                preset.synthosc_waveform = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "ROBOT_VOCODER_GATE_THRESHOLD" {
                preset.vocoder_gate_threshold = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }
            if attr.name.local_name == "ROBOT_DRY_MIX" {
                preset.dry_mix = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }
            if attr.name.local_name == "ROBOT_VOCODER_LOW_FREQ" {
                preset.vocoder_low_freq = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "ROBOT_VOCODER_LOW_GAIN" {
                preset.vocoder_low_gain = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }
            if attr.name.local_name == "ROBOT_VOCODER_LOW_BW" {
                preset.vocoder_low_bw = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "ROBOT_VOCODER_MID_FREQ" {
                preset.vocoder_mid_freq = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "ROBOT_VOCODER_MID_GAIN" {
                preset.vocoder_mid_gain = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }
            if attr.name.local_name == "ROBOT_VOCODER_MID_BW" {
                preset.vocoder_mid_bw = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "ROBOT_VOCODER_HIGH_FREQ" {
                preset.vocoder_high_freq = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "ROBOT_VOCODER_HIGH_GAIN" {
                preset.vocoder_high_gain = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }
            if attr.name.local_name == "ROBOT_VOCODER_HIGH_BW" {
                preset.vocoder_high_bw = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            println!("[RobotEffect] Unparsed Child Attribute: {}", attr.name);
        }

        // Ok, we should be able to store this now..
        if id == 1 {
            self.preset_map[Preset1] = preset;
        } else if id == 2 {
            self.preset_map[Preset2] = preset;
        } else if id == 3 {
            self.preset_map[Preset3] = preset;
        } else if id == 4 {
            self.preset_map[Preset4] = preset;
        } else if id == 5 {
            self.preset_map[Preset5] = preset;
        } else if id == 6 {
            self.preset_map[Preset6] = preset;
        }
    }

    pub fn write_robot(
        &self,
        writer: &mut EventWriter<&mut File>,
    ) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder = XmlWriterEvent::start_element("robotEffect");

        let mut attributes: HashMap<String, String> = HashMap::default();
        self.colour_map.write_colours(&mut attributes);

        // Write out the attributes etc for this element, but don't close it yet..
        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;

        // Because all of these are seemingly 'guaranteed' to exist, we can straight dump..
        for (key, value) in &self.preset_map {
            let mut sub_attributes: HashMap<String, String> = HashMap::default();

            let tag_name = format!("robotEffect{}", key.get_str("tagSuffix").unwrap());
            let mut sub_element: StartElementBuilder =
                XmlWriterEvent::start_element(tag_name.as_str());

            sub_attributes.insert(
                "robotEffectstate".to_string(),
                if value.state {
                    "1".to_string()
                } else {
                    "0".to_string()
                },
            );
            sub_attributes.insert(
                "ROBOT_STYLE".to_string(),
                value.style.get_str("uiIndex").unwrap().to_string(),
            );
            sub_attributes.insert(
                "ROBOT_SYNTHOSC_PULSEWIDTH".to_string(),
                format!("{}", value.synthosc_pulse_width),
            );
            sub_attributes.insert(
                "ROBOT_SYNTHOSC_WAVEFORM".to_string(),
                format!("{}", value.synthosc_waveform),
            );
            sub_attributes.insert(
                "ROBOT_VOCODER_GATE_THRESHOLD".to_string(),
                format!("{}", value.vocoder_gate_threshold),
            );
            sub_attributes.insert("ROBOT_DRY_MIX".to_string(), format!("{}", value.dry_mix));
            sub_attributes.insert(
                "ROBOT_VOCODER_LOW_FREQ".to_string(),
                format!("{}", value.vocoder_low_freq),
            );
            sub_attributes.insert(
                "ROBOT_VOCODER_LOW_GAIN".to_string(),
                format!("{}", value.vocoder_low_gain),
            );
            sub_attributes.insert(
                "ROBOT_VOCODER_LOW_BW".to_string(),
                format!("{}", value.vocoder_low_bw),
            );
            sub_attributes.insert(
                "ROBOT_VOCODER_MID_FREQ".to_string(),
                format!("{}", value.vocoder_mid_freq),
            );
            sub_attributes.insert(
                "ROBOT_VOCODER_MID_GAIN".to_string(),
                format!("{}", value.vocoder_mid_gain),
            );
            sub_attributes.insert(
                "ROBOT_VOCODER_MID_BW".to_string(),
                format!("{}", value.vocoder_mid_bw),
            );
            sub_attributes.insert(
                "ROBOT_VOCODER_HIGH_FREQ".to_string(),
                format!("{}", value.vocoder_high_freq),
            );
            sub_attributes.insert(
                "ROBOT_VOCODER_HIGH_GAIN".to_string(),
                format!("{}", value.vocoder_high_gain),
            );
            sub_attributes.insert(
                "ROBOT_VOCODER_HIGH_BW".to_string(),
                format!("{}", value.vocoder_high_bw),
            );

            for (key, value) in &sub_attributes {
                sub_element = sub_element.attr(key.as_str(), value.as_str());
            }

            writer.write(sub_element)?;
            writer.write(XmlWriterEvent::end_element())?;
        }

        // Finally, close the 'main' tag.
        writer.write(XmlWriterEvent::end_element())?;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct RobotEffect {
    // State here determines if the robot effect is on or off when this preset is loaded.
    state: bool,

    style: RobotStyle,
    synthosc_pulse_width: u8,
    synthosc_waveform: u8,
    vocoder_gate_threshold: i8,
    dry_mix: i8,

    vocoder_low_freq: u8,
    vocoder_low_gain: i8,
    vocoder_low_bw: u8,

    vocoder_mid_freq: u8,
    vocoder_mid_gain: i8,
    vocoder_mid_bw: u8,

    vocoder_high_freq: u8,
    vocoder_high_gain: i8,
    vocoder_high_bw: u8,
}

impl RobotEffect {
    pub fn new() -> Self {
        Self {
            state: false,
            style: Default::default(),

            synthosc_pulse_width: 0,
            synthosc_waveform: 0,
            vocoder_gate_threshold: 0,
            dry_mix: 0,
            vocoder_low_freq: 0,
            vocoder_low_gain: 0,
            vocoder_low_bw: 0,
            vocoder_mid_freq: 0,
            vocoder_mid_gain: 0,
            vocoder_mid_bw: 0,
            vocoder_high_freq: 0,
            vocoder_high_gain: 0,
            vocoder_high_bw: 0,
        }
    }
}

#[derive(Debug, EnumIter, EnumProperty)]
enum RobotStyle {
    #[strum(props(uiIndex = "0"))]
    ROBOT_1,

    #[strum(props(uiIndex = "1"))]
    ROBOT_2,

    #[strum(props(uiIndex = "2"))]
    ROBOT_3,
}

impl Default for RobotStyle {
    fn default() -> Self {
        ROBOT_1
    }
}
