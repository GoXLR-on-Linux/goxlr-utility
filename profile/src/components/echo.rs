use std::collections::HashMap;
use std::io::Write;
use std::os::raw::c_float;

use enum_map::{Enum, EnumMap};
use strum::{EnumIter, EnumProperty, IntoEnumIterator};
use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use crate::components::colours::ColourMap;
use crate::components::megaphone::Preset;
use crate::components::megaphone::Preset::{Preset1, Preset2, Preset3, Preset4, Preset5, Preset6};

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

/**
 * This is relatively static, main tag contains standard colour mapping, subtags contain various
 * presets, we'll use an EnumMap to define the 'presets' as they'll be useful for the other various
 * 'types' of presets (encoders and effects).
 */
#[derive(Debug)]
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

    pub fn parse_echo_root(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name == "active_set" {
                self.active_set = attr.value.parse()?;
                continue;
            }

            if !self.colour_map.read_colours(attr)? {
                println!("[EchoEncoder] Unparsed Attribute: {}", attr.name);
            }
        }

        Ok(())
    }

    pub fn parse_echo_preset(
        &mut self,
        id: u8,
        attributes: &[OwnedAttribute],
    ) -> Result<(), ParseError> {
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
                preset.knob_position = attr.value.parse::<c_float>()? as i8;
                continue;
            }

            if attr.name.local_name == "DELAY_SOURCE" {
                preset.source = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_DIV_L" {
                preset.div_l = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_DIV_R" {
                preset.div_r = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_FB_L" {
                preset.feedback_left = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_FB_R" {
                preset.feedback_right = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_XFB_L_R" {
                preset.xfb_l_to_r = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_XFB_R_L" {
                preset.xfb_r_to_l = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_FB_CONTROL" {
                preset.feedback_control = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_FILTER_STYLE" {
                preset.filter_style = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "DELAY_TIME_L" {
                preset.time_left = attr.value.parse::<c_float>()? as u16;
                continue;
            }
            if attr.name.local_name == "DELAY_TIME_R" {
                preset.time_right = attr.value.parse::<c_float>()? as u16;
                continue;
            }
            if attr.name.local_name == "DELAY_TEMPO" {
                preset.tempo = attr.value.parse::<c_float>()? as u16;
                continue;
            }

            println!(
                "[EchoEncoder] Unparsed Child Attribute: {}",
                &attr.name.local_name
            );
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

        Ok(())
    }

    pub fn write_echo<W: Write>(
        &self,
        writer: &mut EventWriter<&mut W>,
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

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
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
            style: EchoStyle::Quarter,
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
    #[strum(to_string = "QUARTER")]
    Quarter,

    #[strum(props(uiIndex = "1"))]
    #[strum(to_string = "EIGHTH")]
    Eighth,

    #[strum(props(uiIndex = "2"))]
    #[strum(to_string = "TRIPLET")]
    Triplet,

    #[strum(props(uiIndex = "3"))]
    #[strum(to_string = "PING_PONG")]
    PingPong,

    #[strum(props(uiIndex = "4"))]
    #[strum(to_string = "CLASSIC_SLAP")]
    ClassicSlap,

    #[strum(props(uiIndex = "5"))]
    #[strum(to_string = "MULTI_TAP")]
    MultiTap,
}

impl Default for EchoStyle {
    fn default() -> Self {
        EchoStyle::Quarter
    }
}
