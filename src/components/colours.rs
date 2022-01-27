use std::str::FromStr;
use strum_macros::EnumString;
use xml::attribute::OwnedAttribute;

pub struct ColourMap {
    // The colour attribute prefix..
    prefix: String,

    // The Presented Style when object is 'Off'
    off_style: ColourOffStyle,

    // Not sure what this does, present in several places though..
    velocity: Option<u8>,

    // A collection which should all have the same settings (according to the UI)..
    colour_group: Option<String>,

    // The list of Colours, most buttons have 2, Faders have 3..
    colour_list: Option<Vec<Option<Colour>>>,

    // Only present in FaderMeter
    colour_display: Option<ColourDisplay>,
}

impl ColourMap {
    pub fn new(prefix: String) -> Self {
        Self {
            prefix,
            off_style: ColourOffStyle::DIMMED,
            velocity: None,
            colour_group: None,
            colour_list: None,
            colour_display: None
        }
    }

    pub fn read_colours(&mut self, attribute: &OwnedAttribute) -> bool {
        let off_style_attribute = format!("{}offStyle", &self.prefix);
        if attribute.name.local_name == off_style_attribute {
            self.off_style = ColourOffStyle::from_str(&attribute.value).unwrap();
            return true;
        }

        let velocity_attribute = format!("{}velocity", &self.prefix);
        if attribute.name.local_name == velocity_attribute {
            self.velocity = Option::Some(u8::from_str(attribute.value.as_str()).unwrap());
            return true;
        }

        if attribute.name.local_name == "colorGroup" {
            self.colour_group = Option::Some(attribute.value.clone());
            return true;
        }

        let colour_prefix_attribute = format!("{}colour", &self.prefix);
        if attribute.name.local_name.starts_with(colour_prefix_attribute.as_str()) {
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

        let display_prefix_attribute = format!("{}Display", &self.prefix);
        if attribute.name.local_name == display_prefix_attribute {
            
        }




        return false;
    }

    pub fn write_colours() {}
}

#[derive(Debug, EnumString)]
enum ColourOffStyle {
    DIMMED,
}

#[derive(Debug, EnumString)]
enum ColourDisplay {
    GRADIENT,
    METER,
    GRADIENT_METER,
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
}