use std::collections::HashMap;
use std::io::Write;
use std::str::FromStr;

use enum_map::Enum;
use ritelinked::LinkedHashMap;
use strum::{Display, EnumIter, EnumProperty, EnumString};
use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use crate::components::colours::ColourMap;

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
pub struct SampleBase {
    element_name: String,
    colour_map: ColourMap,
    state: String, // Seems to be "Empty" most of the time..
    sample_stack: HashMap<SampleBank, SampleStack>,
}

impl SampleBase {
    pub fn new(element_name: String) -> Self {
        let colour_map = element_name.clone();
        Self {
            element_name,
            colour_map: ColourMap::new(colour_map),
            state: "Empty".to_string(),
            sample_stack: Default::default(),
        }
    }

    pub fn parse_sample_root(&mut self, attributes: &[OwnedAttribute]) -> Result<(), ParseError> {
        for attr in attributes {
            if attr.name.local_name.ends_with("state") && self.element_name != "sampleClear" {
                if attr.value != "Empty" && attr.value != "Stopped" {
                    println!("[Sampler] Unknown State: {}", &attr.value);
                }
                self.state = attr.value.clone();
                continue;
            }

            if !self.colour_map.read_colours(attr)? {
                println!("[Sampler] Unparsed Attribute: {}", attr.name);
            }
        }

        Ok(())
    }

    pub fn parse_sample_stack(
        &mut self,
        id: char,
        attributes: &[OwnedAttribute],
    ) -> Result<(), ParseError> {
        // The easiest way to handle this is to parse everything into key-value pairs, then try
        // to locate all the settings for each track inside it..
        let mut map: HashMap<String, String> = HashMap::default();

        for attr in attributes {
            map.insert(attr.name.local_name.clone(), attr.value.clone());
        }

        let mut sample_stack = SampleStack::new();

        // Pull out any 'extra' attributes which may be useful..
        if let Some(value) = map.get("playbackMode") {
            sample_stack.playback_mode = Some(PlaybackMode::from_usize(value.parse::<usize>()?));
        }

        if let Some(value) = map.get("playOrder") {
            sample_stack.play_order = Some(PlayOrder::from_usize(value.parse::<usize>()?));
        }

        // Ok, somewhere in here we should have a key that tells us how many tracks are configured..
        let key = format!("sampleStack{}stackSize", id);

        if !map.contains_key(key.as_str()) {
            // Stack doesn't contain any tracks, we're done here.
            self.sample_stack
                .insert(SampleBank::from_str(id.to_string().as_str())?, sample_stack);
            return Ok(());
        }

        if let Some(track_count) = map.get(key.as_str()) {
            let track_count: u8 = track_count.parse()?;
            for i in 0..track_count {
                if let (Some(track), Some(start), Some(end), Some(gain)) = (
                    map.get(&format!("track_{}", i)),
                    map.get(&format!("track_{}StartPosition", i)),
                    map.get(&format!("track_{}EndPosition", i)),
                    map.get(&format!("track_{}NormalizedGain", i)),
                ) {
                    let track = Track::new(
                        track.to_string(),
                        start.parse()?,
                        end.parse()?,
                        gain.parse()?,
                    );
                    sample_stack.tracks.push(track);
                }
            }
        }

        self.sample_stack
            .insert(SampleBank::from_str(id.to_string().as_str())?, sample_stack);

        Ok(())
    }

    pub fn write_sample<W: Write>(
        &self,
        writer: &mut EventWriter<&mut W>,
    ) -> Result<(), xml::writer::Error> {
        let mut element: StartElementBuilder =
            XmlWriterEvent::start_element(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        attributes.insert(
            format!("{}state", self.element_name),
            self.state.to_string(),
        );
        self.colour_map.write_colours(&mut attributes);

        // Write out the attributes etc for this element, but don't close it yet..
        for (key, value) in &attributes {
            element = element.attr(key.as_str(), value.as_str());
        }

        writer.write(element)?;

        // Now onto the damn stacks..
        for (key, value) in &self.sample_stack {
            let sub_element_name = format!("sampleStack{}", key);

            let mut sub_element = XmlWriterEvent::start_element(sub_element_name.as_str());

            // Welcome to the only place where order seems to matter, the track_X attributes must all appear together
            // in an ordered, unbroken list, otherwise the GoXLR App will crash :D
            let mut sub_attributes: LinkedHashMap<String, String> = Default::default();

            for i in 0..value.tracks.len() {
                sub_attributes.insert(
                    format!("track_{}", i),
                    value.tracks.get(i).unwrap().track.to_string(),
                );
            }

            if !value.tracks.is_empty() {
                sub_attributes.insert(
                    format!("sampleStack{}stackSize", key),
                    format!("{}", value.tracks.len()),
                );
            }

            for i in 0..value.tracks.len() {
                sub_attributes.insert(
                    format!("track_{}NormalizedGain", i),
                    format!("{}", value.tracks.get(i).unwrap().normalized_gain),
                );
                sub_attributes.insert(
                    format!("track_{}StartPosition", i),
                    format!("{}", value.tracks.get(i).unwrap().start_position),
                );
                sub_attributes.insert(
                    format!("track_{}EndPosition", i),
                    format!("{}", value.tracks.get(i).unwrap().end_position),
                );
            }

            if let Some(output) = &value.playback_mode {
                sub_attributes.insert(
                    "playbackMode".to_string(),
                    output.get_str("index").unwrap().to_string(),
                );
            }

            if let Some(order) = &value.play_order {
                sub_attributes.insert(
                    "playOrder".to_string(),
                    order.get_str("index").unwrap().to_string(),
                );
            }

            // Write the attributes into the tag, and close it.
            for (key, value) in &sub_attributes {
                sub_element = sub_element.attr(key.as_str(), value.as_str());
            }
            writer.write(sub_element)?;
            writer.write(XmlWriterEvent::end_element())?;
        }

        writer.write(XmlWriterEvent::end_element())?;
        Ok(())
    }

    pub fn colour_map(&self) -> &ColourMap {
        &self.colour_map
    }

    pub fn colour_map_mut(&mut self) -> &mut ColourMap {
        &mut self.colour_map
    }

    pub fn get_stack(&self, bank: SampleBank) -> &SampleStack {
        self.sample_stack.get(&bank).unwrap()
    }
}

#[derive(Debug)]
pub struct SampleStack {
    tracks: Vec<Track>,
    playback_mode: Option<PlaybackMode>,
    play_order: Option<PlayOrder>,
}

impl Default for SampleStack {
    fn default() -> Self {
        Self::new()
    }
}

impl SampleStack {
    pub fn new() -> Self {
        Self {
            tracks: vec![],
            playback_mode: None,
            play_order: None,
        }
    }

    pub fn get_sample_count(&self) -> usize {
        self.tracks.len()
    }

    pub fn get_first_sample_file(&self) -> String {
        self.tracks[0].track.to_string()
    }
}

#[derive(Debug)]
struct Track {
    track: String,
    start_position: f32,
    end_position: f32,
    normalized_gain: f64,
}

impl Track {
    pub fn new(
        track: String,
        start_position: f32,
        end_position: f32,
        normalized_gain: f64,
    ) -> Self {
        Self {
            track,
            start_position,
            end_position,
            normalized_gain,
        }
    }
}

#[derive(Debug, Enum, EnumProperty)]
enum PlaybackMode {
    #[strum(props(index = "0"))]
    PlayNext,
    #[strum(props(index = "1"))]
    PlayStop,
    #[strum(props(index = "2"))]
    PlayFade,
    #[strum(props(index = "3"))]
    StopOnRelease,
    #[strum(props(index = "4"))]
    FadeOnRelease,
    #[strum(props(index = "5"))]
    Loop,
}

#[derive(Debug, Enum, EnumProperty)]
enum PlayOrder {
    #[strum(props(index = "0"))]
    Sequential,
    #[strum(props(index = "1"))]
    Random,
}

#[derive(
    Debug, Copy, Clone, Display, Enum, EnumString, EnumProperty, EnumIter, PartialEq, Eq, Hash,
)]
pub enum SampleBank {
    #[strum(props(contextTitle = "sampleStackA"))]
    A,
    #[strum(props(contextTitle = "sampleStackB"))]
    B,
    #[strum(props(contextTitle = "sampleStackC"))]
    C,
}
