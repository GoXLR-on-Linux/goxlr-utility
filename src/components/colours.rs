use std::collections::HashMap;
use std::str::FromStr;
use strum_macros::{EnumString, Display};
use xml::attribute::OwnedAttribute;

pub struct ColourMap {
    // The colour attribute prefix (for parsing)..
    prefix: String,

    // I honestly have no idea what this attribute does, I suspect that it might be an internal
    // state that notes that the current object is being pressed, but saving that would be crazy..
    // I'll place this here for now, despite it seemingly always being 0.
    selected: Option<u8>,
    
    // The Presented Style when object is 'Off'
    off_style: ColourOffStyle,

    // Whether a button is currently 'On'
    state: Option<ColourState>,

    // Whether a button is actively blinking
    blink: Option<ColourState>,

    // Not sure what this does, present in several places though.
    // I'm setting this to i8, because the value I'm seeing is 127.
    velocity: Option<i8>,

    // A collection which should all have the same settings (according to the UI)..
    colour_group: Option<String>,

    // The list of Colours, most buttons have 2, Faders have 3..
    colour_list: Option<Vec<Option<Colour>>>,

    // Only present in FaderMeter
    colour_display: Option<ColourDisplay>,
}

impl ColourMap {
    // In hindsight, the prefix should probably be a ref, it's generally stored elsewhere..
    pub fn new(prefix: String) -> Self {
        Self {
            prefix,
            selected: None,
            off_style: ColourOffStyle::DIMMED,
            state: None,
            blink: None,
            velocity: None,
            colour_group: None,
            colour_list: None,
            colour_display: None
        }
    }

    pub fn read_colours(&mut self, attribute: &OwnedAttribute) -> bool {
        let mut attr_key = format!("{}offStyle", &self.prefix);

        if attribute.name.local_name == attr_key {
            self.off_style = ColourOffStyle::from_str(&attribute.value).unwrap();
            return true;
        }

        attr_key = format!("{}selected", &self.prefix);
        if attribute.name.local_name == attr_key {
            self.selected = Option::Some(u8::from_str(attribute.value.as_str()).unwrap());
            return true;
        }

        attr_key = format!("{}velocity", &self.prefix);
        if attribute.name.local_name == attr_key {
            self.velocity = Option::Some(i8::from_str(attribute.value.as_str()).unwrap());
            return true;
        }

        attr_key = format!("{}state", &self.prefix);
        if attribute.name.local_name == attr_key {
            if attribute.value == "0" {
                self.state = Option::Some(ColourState::OFF);
            } else {
                self.state = Option::Some(ColourState::ON);
            }
            return true;
        }

        attr_key = format!("{}blink", &self.prefix);
        if attribute.name.local_name == attr_key {
            if attribute.value == "0" {
                self.blink = Option::Some(ColourState::OFF);
            } else {
                self.blink = Option::Some(ColourState::ON);
            }
            return true;
        }

        // This attribute is spelt wrong.. >:(
        if attribute.name.local_name == "colorGroup" {
            self.colour_group = Option::Some(attribute.value.clone());
            return true;
        }

        attr_key = format!("{}colour", &self.prefix);
        if attribute.name.local_name.starts_with(attr_key.as_str()) {
            if self.colour_list.is_none() {
                // We've not seen a colour here yet, so we should create the Vector..
                self.colour_list = Option::Some(Vec::new());
                self.colour_list.as_mut().unwrap().resize_with(3, || None);
            }

            // TODO: Tidy this monster up..
            let index = usize::from_str(attribute.name.local_name.chars().last().unwrap().to_string().as_str()).unwrap();
            self.colour_list.as_mut().unwrap().insert(index, Option::Some(Colour::new(&attribute.value)));

            return true;
        }

        attr_key = format!("{}Display", &self.prefix);
        if attribute.name.local_name == attr_key {
            self.colour_display = Option::Some(ColourDisplay::from_str(&attribute.value).unwrap());
            return true;
        }

        return false;
    }

    pub fn write_colours(&self, mut attributes: &mut HashMap<String, String>) {
        // Add the 'OffStyle'
        let mut key = format!("{}offStyle", self.prefix);
        attributes.insert(key, self.off_style.to_string());

        if self.selected.is_some() {
            attributes.insert(format!("{}selected", self.prefix), format!("{}", self.selected.unwrap()));
        }

        if self.velocity.is_some() {
            key = format!("{}velocity", self.prefix);
            attributes.insert(key, format!("{}", self.velocity.unwrap()));
        }

        if self.state.is_some() {
            key = format!("{}state", self.prefix);
            let mut output;
            if self.state.as_ref().unwrap() == &ColourState::OFF {
                output = "0".to_string();
            } else {
                output = "1".to_string();
            }
            attributes.insert(key, output);
        }

        if self.blink.is_some() {
            key = format!("{}blink", self.prefix);
            let mut output;
            if self.blink.as_ref().unwrap() == &ColourState::OFF {
                output = "0".to_string();
            } else {
                output = "1".to_string();
            }
            attributes.insert(key, output);
        }


        if self.colour_group.is_some() {
            key = format!("{}colorGroup", self.prefix);
            let colour = self.colour_group.as_ref().unwrap().clone();
            attributes.insert(key, colour);
        }

        if self.colour_list.is_some() {
            let vector = self.colour_list.as_ref().unwrap();
            for i in 0..3 {
                if vector.get(i).is_some() {
                    let colour = vector.get(i).unwrap();
                    if colour.is_some() {
                        key = format!("{}color{}", self.prefix, i);
                        attributes.insert(key, colour.as_ref().unwrap().to_rgba());
                    }
                }
            }
        }

        if self.colour_display.is_some() {
            key = format!("{}Display", self.prefix);
            attributes.insert(key, self.colour_display.as_ref().unwrap().to_string());
        }
    }
}

#[derive(Debug, EnumString, Display)]
enum ColourOffStyle {
    DIMMED,
    COLOUR2,
    DIMMEDCOLOUR2,
}

#[derive(Debug, EnumString, Display)]
enum ColourDisplay {
    GRADIENT,
    METER,
    GRADIENT_METER,
}

#[derive(Debug, PartialEq)]
enum ColourState {
    OFF,
    ON,
}

struct Colour {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Colour {
    pub fn new(rgba: &String) -> Self {
        Self {
            red: u8::from_str_radix(&rgba[0..2], 16).unwrap(),
            green: u8::from_str_radix(&rgba[2..4], 16).unwrap(),
            blue: u8::from_str_radix(&rgba[4..6], 16).unwrap(),
            alpha: u8::from_str_radix(&rgba[6..8], 16).unwrap()
        }
    }

    pub fn to_rgba(&self) -> String {
        return format!("{:02X}{:02X}{:02X}{:02X}", self.red, self.green, self.blue, self.alpha);
    }
}