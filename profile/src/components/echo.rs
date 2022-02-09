use std::collections::HashMap;
use std::fs::File;
use std::os::raw::c_float;

use enum_map::{Enum, EnumMap};
use strum::{EnumIter, EnumProperty, IntoEnumIterator};
use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use crate::components::colours::ColourMap;
use crate::components::megaphone::Preset;
use crate::components::megaphone::Preset::{
    PRESET_1, PRESET_2, PRESET_3, PRESET_4, PRESET_5, PRESET_6,
};

/**
 * This is relatively static, main tag contains standard colour mapping, subtags contain various
 * presets, we'll use an EnumMap to define the 'presets' as they'll be useful for the other various
 * 'types' of presets (encoders and effects).
 */
pub struct EchoEncoderBase {
    colour_map: ColourMap,
    preset_map: EnumMap<Preset, EchoEncoder>,
    active_set: u8, // Not sure what this does?
}

impl EchoEncoderBase {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name;
        Self {
            colour_map: ColourMap::new(colour_map),
            preset_map: EnumMap::default(),
            active_set: 0,
        }
    }

    pub fn parse_echo_root(&mut self, attributes: &[OwnedAttribute]) {
        for attr in attributes {
            if attr.name.local_name == "active_set" {
                self.active_set = attr.value.parse().unwrap();
                continue;
            }

            if !self.colour_map.read_colours(attr) {
                println!("[EchoEncoder] Unparsed Attribute: {}", attr.name);
            }
        }
    }

    pub fn parse_echo_preset(&mut self, id: u8, attributes: &[OwnedAttribute]) {
        let mut preset = EchoEncoder::new();
        for attr in attributes {
            if attr.name.local_name == "DELAY_STYLE" {
                for style in EchoStyle::iter() {
                    if style.get_str("uiIndex").unwrap() == attr.value {
                        preset.style = style;
                        break;
                    }
                }
                continue;
            }

            if attr.name.local_name == "DELAY_KNOB_POSITION" {
                preset.knob_position = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }

            if attr.name.local_name == "DELAY_SOURCE" {
                preset.source = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_DIV_L" {
                preset.div_l = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_DIV_R" {
                preset.div_r = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_FB_L" {
                preset.feedback_left = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_FB_R" {
                preset.feedback_right = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_XFB_L_R" {
                preset.xfb_l_to_r = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_XFB_R_L" {
                preset.xfb_r_to_l = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_FB_CONTROL" {
                preset.feedback_control = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_FILTER_STYLE" {
                preset.filter_style = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_TIME_L" {
                preset.time_left = attr.value.parse::<c_float>().unwrap() as u16;
                continue;
            }
            if attr.name.local_name == "DELAY_TIME_R" {
                preset.time_right = attr.value.parse::<c_float>().unwrap() as u16;
                continue;
            }
            if attr.name.local_name == "DELAY_TEMPO" {
                preset.tempo = attr.value.parse::<c_float>().unwrap() as u16;
                continue;
            }

            println!(
                "[EchoEncoder] Unparsed Child Attribute: {}",
                &attr.name.local_name
            );
        }

        // Ok, we should be able to store this now..
        if id == 1 {
            self.preset_map[PRESET_1] = preset;
        } else if id == 2 {
            self.preset_map[PRESET_2] = preset;
        } else if id == 3 {
            self.preset_map[PRESET_3] = preset;
        } else if id == 4 {
            self.preset_map[PRESET_4] = preset;
        } else if id == 5 {
            self.preset_map[PRESET_5] = preset;
        } else if id == 6 {
            self.preset_map[PRESET_6] = preset;
        }
    }

    pub fn write_echo(
        &self,
        writer: &mut EventWriter<&mut File>,
    ) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder = XmlWriterEvent::start_element("echoEncoder");

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("active_set".to_string(), format!("{}", self.active_set));
        self.colour_map.write_colours(&mut attributes);

        // Write out the attributes etc for this element, but don't close it yet..
        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;

        // Because all of these are seemingly 'guaranteed' to exist, we can straight dump..
        for (key, value) in &self.preset_map {
            let mut sub_attributes: HashMap<String, String> = HashMap::default();

            let tag_name = format!("echoEncoder{}", key.get_str("tagSuffix").unwrap());
            let mut sub_element: StartElementBuilder =
                XmlWriterEvent::start_element(tag_name.as_str());

            sub_attributes.insert(
                "DELAY_KNOB_POSITION".to_string(),
                format!("{}", value.knob_position),
            );
            sub_attributes.insert(
                "DELAY_STYLE".to_string(),
                value.style.get_str("uiIndex").unwrap().to_string(),
            );
            sub_attributes.insert("DELAY_SOURCE".to_string(), format!("{}", value.source));
            sub_attributes.insert("DELAY_DIV_L".to_string(), format!("{}", value.div_l));
            sub_attributes.insert("DELAY_DIV_R".to_string(), format!("{}", value.div_r));
            sub_attributes.insert("DELAY_FB_L".to_string(), format!("{}", value.feedback_left));
            sub_attributes.insert(
                "DELAY_FB_R".to_string(),
                format!("{}", value.feedback_right),
            );
            sub_attributes.insert("DELAY_XFB_L_R".to_string(), format!("{}", value.xfb_l_to_r));
            sub_attributes.insert("DELAY_XFB_R_L".to_string(), format!("{}", value.xfb_r_to_l));
            sub_attributes.insert(
                "DELAY_FB_CONTROL".to_string(),
                format!("{}", value.feedback_control),
            );
            sub_attributes.insert(
                "DELAY_FILTER_STYLE".to_string(),
                format!("{}", value.filter_style),
            );
            sub_attributes.insert("DELAY_TIME_L".to_string(), format!("{}", value.time_left));
            sub_attributes.insert("DELAY_TIME_R".to_string(), format!("{}", value.time_right));
            sub_attributes.insert("DELAY_TEMPO".to_string(), format!("{}", value.tempo));

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
struct EchoEncoder {
    knob_position: i8,
    style: EchoStyle,
    source: u8,
    div_l: u8,
    div_r: u8,
    feedback_left: u8,
    feedback_right: u8,
    feedback_control: u8,
    xfb_l_to_r: u8,
    xfb_r_to_l: u8,
    filter_style: u8,
    time_left: u16,
    time_right: u16,
    tempo: u16,
}

impl EchoEncoder {
    pub fn new() -> Self {
        Self {
            knob_position: 0,
            style: EchoStyle::QUARTER,
            source: 0,
            div_l: 0,
            div_r: 0,
            feedback_left: 0,
            feedback_right: 0,
            feedback_control: 0,
            xfb_l_to_r: 0,
            xfb_r_to_l: 0,
            filter_style: 0,
            time_left: 0,
            time_right: 0,
            tempo: 0,
        }
    }
}

#[derive(Debug, EnumIter, Enum, EnumProperty)]
enum EchoStyle {
    #[strum(props(uiIndex = "0"))]
    QUARTER,

    #[strum(props(uiIndex = "1"))]
    EIGHTH,

    #[strum(props(uiIndex = "2"))]
    TRIPLET,

    #[strum(props(uiIndex = "3"))]
    PING_PONG,

    #[strum(props(uiIndex = "4"))]
    CLASSIC_SLAP,

    #[strum(props(uiIndex = "5"))]
    MULTI_TAP,
}

impl Default for EchoStyle {
    fn default() -> Self {
        EchoStyle::QUARTER
    }
}
