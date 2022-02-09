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
use crate::components::reverb::ReverbStyle::LIBRARY;

/**
 * This is relatively static, main tag contains standard colour mapping, subtags contain various
 * presets, we'll use an EnumMap to define the 'presets' as they'll be useful for the other various
 * 'types' of presets (encoders and effects).
 */
pub struct ReverbEncoderBase {
    colour_map: ColourMap,
    preset_map: EnumMap<Preset, ReverbEncoder>,
    active_set: u8, // Not sure what this does?
}

impl ReverbEncoderBase {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name.clone();
        Self {
            colour_map: ColourMap::new(colour_map),
            preset_map: EnumMap::default(),
            active_set: 0,
        }
    }

    pub fn parse_reverb_root(&mut self, attributes: &Vec<OwnedAttribute>) {
        for attr in attributes {
            if attr.name.local_name == "active_set" {
                self.active_set = attr.value.parse().unwrap();
                continue;
            }

            if !self.colour_map.read_colours(&attr) {
                println!("[ReverbEncoder] Unparsed Attribute: {}", attr.name);
            }
        }
    }

    pub fn parse_reverb_preset(&mut self, id: u8, attributes: &Vec<OwnedAttribute>) {
        let mut preset = ReverbEncoder::new();
        for attr in attributes {
            if attr.name.local_name == "REVERB_STYLE" {
                for style in ReverbStyle::iter() {
                    if style.get_str("uiIndex").unwrap() == attr.value {
                        preset.style = style;
                        break;
                    }
                }
                continue;
            }

            if attr.name.local_name == "REVERB_KNOB_POSITION" {
                preset.knob_position = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }

            if attr.name.local_name == "REVERB_TYPE" {
                preset.reverb_type = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "REVERB_DECAY" {
                preset.decay = attr.value.parse::<c_float>().unwrap() as u16;
                continue;
            }
            if attr.name.local_name == "REVERB_PREDELAY" {
                preset.predelay = attr.value.parse::<c_float>().unwrap() as u8;
                continue;
            }
            if attr.name.local_name == "REVERB_DIFFUSE" {
                preset.diffuse = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }
            if attr.name.local_name == "REVERB_LOCOLOR" {
                preset.locolor = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }
            if attr.name.local_name == "REVERB_HICOLOR" {
                preset.hicolor = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }
            if attr.name.local_name == "REVERB_HIFACTOR" {
                preset.hifactor = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }
            if attr.name.local_name == "REVERB_MODSPEED" {
                preset.mod_speed = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }
            if attr.name.local_name == "REVERB_MODDEPTH" {
                preset.mod_depth = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }
            if attr.name.local_name == "REVERB_EARLYLEVEL" {
                preset.early_level = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }
            if attr.name.local_name == "REVERB_TAILLEVEL" {
                preset.tail_level = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }
            if attr.name.local_name == "REVERB_DRYLEVEL" {
                preset.dry_level = attr.value.parse::<c_float>().unwrap() as i8;
                continue;
            }

            println!(
                "[ReverbEncoder] Unparsed Child Attribute: {}",
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

    pub fn write_reverb(&self, writer: &mut EventWriter<&mut File>) {
        let mut element: StartElementBuilder = XmlWriterEvent::start_element("reverbEncoder");

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert("active_set".to_string(), format!("{}", self.active_set));
        self.colour_map.write_colours(&mut attributes);

        // Write out the attributes etc for this element, but don't close it yet..
        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element);

        // Because all of these are seemingly 'guaranteed' to exist, we can straight dump..
        for (key, value) in &self.preset_map {
            let mut sub_attributes: HashMap<String, String> = HashMap::default();

            let tag_name = format!("reverbEncoder{}", key.get_str("tagSuffix").unwrap());
            let mut sub_element: StartElementBuilder =
                XmlWriterEvent::start_element(tag_name.as_str());

            sub_attributes.insert(
                "REVERB_KNOB_POSITION".to_string(),
                format!("{}", value.knob_position),
            );
            sub_attributes.insert(
                "REVERB_STYLE".to_string(),
                value.style.get_str("uiIndex").unwrap().to_string(),
            );
            sub_attributes.insert("REVERB_TYPE".to_string(), format!("{}", value.reverb_type));
            sub_attributes.insert("REVERB_DECAY".to_string(), format!("{}", value.decay));
            sub_attributes.insert("REVERB_PREDELAY".to_string(), format!("{}", value.predelay));
            sub_attributes.insert("REVERB_DIFFUSE".to_string(), format!("{}", value.diffuse));
            sub_attributes.insert("REVERB_LOCOLOR".to_string(), format!("{}", value.locolor));
            sub_attributes.insert("REVERB_HICOLOR".to_string(), format!("{}", value.hicolor));
            sub_attributes.insert("REVERB_HIFACTOR".to_string(), format!("{}", value.hifactor));
            sub_attributes.insert(
                "REVERB_MODSPEED".to_string(),
                format!("{}", value.mod_speed),
            );
            sub_attributes.insert(
                "REVERB_MODDEPTH".to_string(),
                format!("{}", value.mod_depth),
            );
            sub_attributes.insert(
                "REVERB_EARLYLEVEL".to_string(),
                format!("{}", value.early_level),
            );
            sub_attributes.insert(
                "REVERB_TAILLEVEL".to_string(),
                format!("{}", value.tail_level),
            );
            sub_attributes.insert(
                "REVERB_DRYLEVEL".to_string(),
                format!("{}", value.dry_level),
            );

            for (key, value) in &sub_attributes {
                sub_element = sub_element.attr(key.as_str(), value.as_str());
            }

            writer.write(sub_element);
            writer.write(XmlWriterEvent::end_element());
        }

        // Finally, close the 'main' tag.
        writer.write(XmlWriterEvent::end_element());
    }
}

#[derive(Debug, Default)]
struct ReverbEncoder {
    knob_position: i8,
    style: ReverbStyle,
    reverb_type: u8, // I have no idea what this maps too..
    decay: u16,      // Reaches 290 when set to max.
    predelay: u8,
    diffuse: i8,
    locolor: i8,
    hicolor: i8,
    hifactor: i8,
    mod_speed: i8,
    mod_depth: i8,
    early_level: i8,
    tail_level: i8,
    dry_level: i8,
}

impl ReverbEncoder {
    pub fn new() -> Self {
        Self {
            knob_position: 0,
            style: ReverbStyle::LIBRARY,
            reverb_type: 0,
            decay: 0,
            predelay: 0,
            diffuse: 0,
            locolor: 0,
            hicolor: 0,
            hifactor: 0,
            mod_speed: 0,
            mod_depth: 0,
            early_level: 0,
            tail_level: 0,
            dry_level: 0,
        }
    }
}

#[derive(Debug, EnumIter, Enum, EnumProperty)]
enum ReverbStyle {
    #[strum(props(uiIndex = "0"))]
    LIBRARY,

    #[strum(props(uiIndex = "1"))]
    DARK_BLOOM,

    #[strum(props(uiIndex = "2"))]
    MUSIC_CLUB,

    #[strum(props(uiIndex = "3"))]
    REAL_PLATE,

    #[strum(props(uiIndex = "4"))]
    CHAPEL,

    #[strum(props(uiIndex = "5"))]
    HOCKEY_ARENA,
}

impl Default for ReverbStyle {
    fn default() -> Self {
        LIBRARY
    }
}
