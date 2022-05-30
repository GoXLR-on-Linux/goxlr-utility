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
use crate::components::megaphone::MegaphoneStyle::Megaphone;
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
pub struct MegaphoneEffectBase {
    colour_map: ColourMap,
    preset_map: EnumMap<Preset, MegaphoneEffect>,
}

impl MegaphoneEffectBase {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name;
        Self {
            colour_map: ColourMap::new(colour_map),
            preset_map: EnumMap::default(),
        }
    }

    pub fn parse_megaphone_root(
        &mut self,
        attributes: &[OwnedAttribute],
    ) -> Result<(), ParseError> {
        for attr in attributes {
            if !self.colour_map.read_colours(attr)? {
                println!("[megaphoneEffect] Unparsed Attribute: {}", attr.name);
            }
        }

        Ok(())
    }

    pub fn parse_megaphone_preset(
        &mut self,
        id: u8,
        attributes: &[OwnedAttribute],
    ) -> Result<(), ParseError> {
        let mut preset = MegaphoneEffect::new();
        for attr in attributes {
            if attr.name.local_name == "megaphoneEffectstate" {
                if attr.value == "1" {
                    preset.state = true;
                } else {
                    preset.state = false
                }
                continue;
            }
            if attr.name.local_name == "MEGAPHONE_STYLE" {
                for style in MegaphoneStyle::iter() {
                    if style.get_str("uiIndex").unwrap() == attr.value {
                        preset.style = style;
                        break;
                    }
                }
                continue;
            }

            /*
             * As batshit as the below code seems, in some cases the Windows UI will spit out
             * values as floats, despite those floats representing whole numbers (eg 5.00000), so
             * for all cases here, we're going to read the numbers in as floats, then convert them
             * across to their correct type.
             */

            if attr.name.local_name == "TRANS_DIST_AMT" {
                preset.trans_dist_amt = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "TRANS_HP" {
                preset.trans_hp = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "TRANS_LP" {
                preset.trans_lp = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "TRANS_PREGAIN" {
                preset.trans_pregain = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "TRANS_POSTGAIN" {
                preset.trans_postgain = attr.value.parse::<c_float>()? as i8;
                continue;
            }
            if attr.name.local_name == "TRANS_DIST_TYPE" {
                preset.trans_dist_type = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "TRANS_PRESENCE_GAIN" {
                preset.trans_presence_gain = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "TRANS_PRESENCE_FC" {
                preset.trans_presence_fc = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "TRANS_PRESENCE_BW" {
                preset.trans_presence_bw = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "TRANS_BEATBOX_ENABLE" {
                preset.trans_beatbox_enabled = attr.value != "0";
                continue;
            }
            if attr.name.local_name == "TRANS_FILTER_CONTROL" {
                preset.trans_filter_control = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "TRANS_FILTER" {
                preset.trans_filter = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "TRANS_DRIVE_POT_GAIN_COMP_MID" {
                preset.trans_drive_pot_gain_comp_mid = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            if attr.name.local_name == "TRANS_DRIVE_POT_GAIN_COMP_MAX" {
                preset.trans_drive_pot_gain_comp_max = attr.value.parse::<c_float>()? as u8;
                continue;
            }
            println!(
                "[MegaphoneEffect] Unparsed Child Attribute: {}",
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

    pub fn write_megaphone<W: Write>(
        &self,
        writer: &mut EventWriter<&mut W>,
    ) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder = XmlWriterEvent::start_element("megaphoneEffect");

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

            let tag_name = format!("megaphoneEffect{}", key.get_str("tagSuffix").unwrap());
            let mut sub_element: StartElementBuilder =
                XmlWriterEvent::start_element(tag_name.as_str());

            sub_attributes.insert(
                "megaphoneEffectstate".to_string(),
                if value.state {
                    "1".to_string()
                } else {
                    "0".to_string()
                },
            );
            sub_attributes.insert(
                "MEGAPHONE_STYLE".to_string(),
                value.style.get_str("uiIndex").unwrap().to_string(),
            );
            sub_attributes.insert(
                "TRANS_DIST_AMT".to_string(),
                format!("{}", value.trans_dist_amt),
            );
            sub_attributes.insert("TRANS_HP".to_string(), format!("{}", value.trans_hp));
            sub_attributes.insert("TRANS_LP".to_string(), format!("{}", value.trans_lp));
            sub_attributes.insert(
                "TRANS_PREGAIN".to_string(),
                format!("{}", value.trans_pregain),
            );
            sub_attributes.insert(
                "TRANS_POSTGAIN".to_string(),
                format!("{}", value.trans_postgain),
            );
            sub_attributes.insert(
                "TRANS_DIST_TYPE".to_string(),
                format!("{}", value.trans_dist_type),
            );
            sub_attributes.insert(
                "TRANS_PRESENCE_GAIN".to_string(),
                format!("{}", value.trans_presence_gain),
            );
            sub_attributes.insert(
                "TRANS_PRESENCE_FC".to_string(),
                format!("{}", value.trans_presence_fc),
            );
            sub_attributes.insert(
                "TRANS_PRESENCE_BW".to_string(),
                format!("{}", value.trans_presence_bw),
            );
            sub_attributes.insert(
                "TRANS_BEATBOX_ENABLE".to_string(),
                if value.trans_beatbox_enabled {
                    "1".to_string()
                } else {
                    "0".to_string()
                },
            );
            sub_attributes.insert(
                "TRANS_FILTER_CONTROL".to_string(),
                format!("{}", value.trans_filter_control),
            );
            sub_attributes.insert(
                "TRANS_FILTER".to_string(),
                format!("{}", value.trans_filter),
            );
            sub_attributes.insert(
                "TRANS_DRIVE_POT_GAIN_COMP_MID".to_string(),
                format!("{}", value.trans_drive_pot_gain_comp_mid),
            );
            sub_attributes.insert(
                "TRANS_DRIVE_POT_GAIN_COMP_MAX".to_string(),
                format!("{}", value.trans_drive_pot_gain_comp_max),
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

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }

    pub fn colour_map_mut(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }

    pub fn get_preset(&self, preset: Preset) -> &MegaphoneEffect {
        &self.preset_map[preset]
    }

    pub fn get_preset_mut(&mut self, preset: Preset) -> &mut MegaphoneEffect {
        &mut self.preset_map[preset]
    }

}

/**
 * Couple of interesting points, firstly, the UI only has 3 options with regards to the
 * megaphone configuration, Style, 'Amount', and 'Post Gain', yet these three options
 * ultimately translate into *MANY* different settings, so some investigation as to how
 * and why these map will be necessary. I'm currently assuming that each 'style' is backed
 * by several values, but still need to work out the mapping.
 *
 */
#[derive(Debug, Default)]
pub struct MegaphoneEffect {
    // State here determines if the megaphone is on or off when this preset is loaded.
    state: bool,

    style: MegaphoneStyle,
    trans_dist_amt: u8,
    trans_hp: u8,
    trans_lp: u8,
    trans_pregain: u8,
    trans_postgain: i8,
    trans_dist_type: u8,
    trans_presence_gain: u8,
    trans_presence_fc: u8,
    trans_presence_bw: u8,
    trans_beatbox_enabled: bool,
    trans_filter_control: u8,
    trans_filter: u8,
    trans_drive_pot_gain_comp_mid: u8,
    trans_drive_pot_gain_comp_max: u8,
}

impl MegaphoneEffect {
    pub fn new() -> Self {
        Self {
            state: false,
            style: MegaphoneStyle::Megaphone,
            trans_dist_amt: 0,
            trans_hp: 0,
            trans_lp: 0,
            trans_pregain: 0,
            trans_postgain: 0,
            trans_dist_type: 0,
            trans_presence_gain: 0,
            trans_presence_fc: 0,
            trans_presence_bw: 0,
            trans_beatbox_enabled: false,
            trans_filter_control: 0,
            trans_filter: 0,
            trans_drive_pot_gain_comp_mid: 0,
            trans_drive_pot_gain_comp_max: 0,
        }
    }


    pub fn state(&self) -> bool {
        self.state
    }

    pub fn set_state(&mut self, state: bool) {
        self.state = state;
    }
}

#[derive(Debug, EnumIter, EnumProperty)]
enum MegaphoneStyle {
    #[strum(props(uiIndex = "0"))]
    Megaphone,

    #[strum(props(uiIndex = "1"))]
    Radio,

    #[strum(props(uiIndex = "2"))]
    OnThePhone,

    #[strum(props(uiIndex = "3"))]
    Overdrive,

    #[strum(props(uiIndex = "4"))]
    BuzzCut,

    #[strum(props(uiIndex = "5"))]
    Tweed,
}

impl Default for MegaphoneStyle {
    fn default() -> Self {
        Megaphone
    }
}

// TODO: Move this.
// In addition, 'contextTitle' refers to how this is represented in the <selectedContext tag
#[derive(Debug, EnumIter, Enum, EnumProperty, Copy, Clone)]
pub enum Preset {
    #[strum(props(tagSuffix = "preset1", contextTitle = "effects1"))]
    #[strum(to_string = "PRESET_1")]
    Preset1,

    #[strum(props(tagSuffix = "preset2", contextTitle = "effects2"))]
    #[strum(to_string = "PRESET_2")]
    Preset2,

    #[strum(props(tagSuffix = "preset3", contextTitle = "effects3"))]
    #[strum(to_string = "PRESET_3")]
    Preset3,

    #[strum(props(tagSuffix = "preset4", contextTitle = "effects4"))]
    #[strum(to_string = "PRESET_4")]
    Preset4,

    #[strum(props(tagSuffix = "preset5", contextTitle = "effects5"))]
    #[strum(to_string = "PRESET_5")]
    Preset5,

    #[strum(props(tagSuffix = "preset6", contextTitle = "effects6"))]
    #[strum(to_string = "PRESET_6")]
    Preset6,
}
