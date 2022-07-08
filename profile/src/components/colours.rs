use std::collections::HashMap;
use std::str::FromStr;

#[derive(thiserror::Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    #[error("Expected int: {0}")]
    ExpectedInt(#[from] std::num::ParseIntError),

    #[error("Expected float: {0}")]
    ExpectedFloat(#[from] std::num::ParseFloatError),

    #[error("Expected enum: {0}")]
    ExpectedEnum(#[from] strum::ParseError),
}
use crate::components::colours::ColourDisplay::{Gradient, GradientMeter, Meter};
use strum::{Display, EnumString};
use xml::attribute::OwnedAttribute;

#[derive(Debug)]
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
            off_style: ColourOffStyle::Dimmed,
            state: None,
            blink: None,
            velocity: None,
            colour_group: None,
            colour_list: None,
            colour_display: None,
        }
    }

    pub fn read_colours(&mut self, attribute: &OwnedAttribute) -> Result<bool, ParseError> {
        let mut attr_key = format!("{}offStyle", &self.prefix);

        if attribute.name.local_name == attr_key {
            self.off_style = ColourOffStyle::from_str(&attribute.value)?;
            return Ok(true);
        }

        attr_key = format!("{}selected", &self.prefix);
        if attribute.name.local_name == attr_key {
            self.selected = Option::Some(u8::from_str(attribute.value.as_str())?);
            return Ok(true);
        }

        attr_key = format!("{}velocity", &self.prefix);
        if attribute.name.local_name == attr_key {
            self.velocity = Option::Some(i8::from_str(attribute.value.as_str())?);
            return Ok(true);
        }

        attr_key = format!("{}state", &self.prefix);
        if attribute.name.local_name == attr_key {
            self.state = Some(ColourState::from_str(&attribute.value)?);
            return Ok(true);
        }

        attr_key = format!("{}blink", &self.prefix);
        if attribute.name.local_name == attr_key {
            self.blink = Some(ColourState::from_str(&attribute.value)?);
            return Ok(true);
        }

        // This attribute is spelt wrong.. >:(
        if attribute.name.local_name == "colorGroup" {
            self.colour_group = Option::Some(attribute.value.clone());
            return Ok(true);
        }

        attr_key = format!("{}colour", &self.prefix);
        if attribute.name.local_name.starts_with(attr_key.as_str()) {
            let color_list = self.colour_list.get_or_insert_with(|| {
                let mut default = Vec::new();
                default.resize_with(3, || None);
                default
            });

            // TODO: Tidy this monster up..
            if let Some(index) = attribute
                .name
                .local_name
                .chars()
                .last()
                .map(|s| usize::from_str(&s.to_string()))
                .transpose()?
            {
                color_list[index] = Option::Some(Colour::new(&attribute.value)?);
            }

            return Ok(true);
        }

        attr_key = format!("{}Display", &self.prefix);
        if attribute.name.local_name == attr_key {
            self.colour_display = Option::Some(ColourDisplay::from_str(&attribute.value)?);
            return Ok(true);
        }

        Ok(false)
    }

    pub fn write_colours(&self, attributes: &mut HashMap<String, String>) {
        // Add the 'OffStyle'
        let mut key = format!("{}offStyle", self.prefix);
        attributes.insert(key, self.off_style.to_string());

        if let Some(selected) = self.selected {
            attributes.insert(format!("{}selected", self.prefix), format!("{}", selected));
        }

        if let Some(velocity) = &self.velocity {
            key = format!("{}velocity", self.prefix);
            attributes.insert(key, format!("{}", velocity));
        }

        if let Some(state) = &self.state {
            key = format!("{}state", self.prefix);
            attributes.insert(key, state.to_string());
        }

        if let Some(blink) = &self.blink {
            key = format!("{}blink", self.prefix);
            attributes.insert(key, blink.to_string());
        }

        if let Some(colour_group) = &self.colour_group {
            let colour = colour_group.to_string();
            attributes.insert("colorGroup".to_string(), colour);
        }

        if let Some(vector) = &self.colour_list {
            for i in 0..3 {
                if let Some(Some(colour)) = vector.get(i) {
                    key = format!("{}colour{}", self.prefix, i);
                    attributes.insert(key, colour.to_rgba());
                }
            }
        }

        if let Some(colour_display) = &self.colour_display {
            key = format!("{}Display", self.prefix);
            attributes.insert(key, colour_display.to_string());
        }
    }

    pub fn colour(&self, index: u8) -> &Colour {
        self.colour_list.as_ref().unwrap()[index as usize]
            .as_ref()
            .unwrap()
    }
    pub fn get_off_style(&self) -> &ColourOffStyle {
        &self.off_style
    }
    pub fn is_fader_gradient(&self) -> bool {
        if self.colour_display.is_none() {
            return false;
        }

        if self.colour_display.as_ref().unwrap() == &Gradient {
            return true;
        }

        if self.colour_display.as_ref().unwrap() == &GradientMeter {
            return true;
        }

        false
    }

    pub fn is_fader_meter(&self) -> bool {
        if self.colour_display.is_none() {
            return false;
        }
        if self.colour_display.as_ref().unwrap() == &Meter {
            return true;
        }
        if self.colour_display.as_ref().unwrap() == &GradientMeter {
            return true;
        }

        false
    }

    pub fn set_fader_display(&mut self, display: ColourDisplay) {
        self.colour_display = Some(display);
    }

    pub fn state(&self) -> &Option<ColourState> {
        &self.state
    }
    pub fn get_state(&self) -> bool {
        if let Some(state) = &self.state {
            return state == &ColourState::On;
        }
        false
    }

    pub fn blink(&self) -> &Option<ColourState> {
        &self.blink
    }

    pub fn set_state(&mut self, state: Option<ColourState>) {
        self.state = state;
    }
    pub fn set_state_on(&mut self, on: bool) {
        if on {
            self.state = Some(ColourState::On);
        } else {
            self.state = Some(ColourState::Off);
        }
    }

    pub fn set_blink(&mut self, blink: Option<ColourState>) {
        self.blink = blink;
    }

    pub fn set_blink_on(&mut self, on: bool) {
        if on {
            self.blink = Some(ColourState::On);
        } else {
            self.blink = Some(ColourState::Off);
        }
    }

    pub fn set_colour(&mut self, index: usize, input: Colour) {
        if let Some(colour) = &mut self.colour_list {
            colour[index] = Some(input);
        } else {
            // Apparently the colour list isn't defined, it should be, but whatever..
            let mut default = Vec::new();
            default.resize_with(3, || None);
            default[index] = Some(input);
            self.colour_list = Some(default);
        }
    }
    pub fn set_off_style(&mut self, off_style: ColourOffStyle) {
        self.off_style = off_style;
    }
}

#[derive(Debug, PartialEq, EnumString, Display)]
pub enum ColourOffStyle {
    #[strum(to_string = "DIMMED")]
    Dimmed,

    #[strum(to_string = "COLOUR2")]
    Colour2,

    #[strum(to_string = "DIMMEDCOLOUR2")]
    DimmedColour2,
}

#[derive(Debug, PartialEq, EnumString, Display)]
pub enum ColourDisplay {
    #[strum(to_string = "GRADIENT")]
    Gradient,

    #[strum(to_string = "METER")]
    Meter,

    #[strum(to_string = "GRADIENT_METER")]
    GradientMeter,

    #[strum(to_string = "TWO COLOR")]
    TwoColour,
}

#[derive(Debug, EnumString, PartialEq, Display)]
pub enum ColourState {
    #[strum(to_string = "0")]
    Off,

    #[strum(to_string = "1")]
    On,
}

#[derive(Debug)]
pub struct Colour {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Colour {
    pub fn new(rgba: &str) -> Result<Self, ParseError> {
        // I'm pretty sure now that this is actually wrong, I'll need to do more profile tests to
        // confirm, but I'm relatively sure the config format is ARGB and not RGBA, but because
        // the ordering is correct despite naming being wrong, it works.
        Ok(Self {
            red: u8::from_str_radix(&rgba[0..2], 16)?,
            green: u8::from_str_radix(&rgba[2..4], 16)?,
            blue: u8::from_str_radix(&rgba[4..6], 16)?,
            alpha: u8::from_str_radix(&rgba[6..8], 16)?,
        })
    }

    // Until I test and fix the above issue, this methods creates based on an RGBA.
    pub fn fromrgb(rgb: &str) -> Result<Self, ParseError> {
        Ok(Self {
            red: u8::from_str_radix("00", 16)?,
            green: u8::from_str_radix(&rgb[0..2], 16)?,
            blue: u8::from_str_radix(&rgb[2..4], 16)?,
            alpha: u8::from_str_radix(&rgb[4..6], 16)?,
        })
    }

    pub fn to_rgba(&self) -> String {
        return format!(
            "{:02X}{:02X}{:02X}{:02X}",
            self.red, self.green, self.blue, self.alpha
        );
    }

    pub fn to_reverse_bytes(&self) -> [u8; 4] {
        [self.alpha, self.blue, self.green, self.red]
    }
}
