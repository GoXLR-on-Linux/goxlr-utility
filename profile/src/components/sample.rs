use std::collections::HashMap;
use std::io::Write;
use std::str::FromStr;

use anyhow::{bail, Result};

use enum_map::Enum;
use quick_xml::events::{BytesEnd, BytesStart, Event};
use quick_xml::Writer;
use rand::seq::SliceRandom;
use ritelinked::LinkedHashMap;
use strum::{Display, EnumIter, EnumProperty, EnumString};

use crate::components::colours::ColourMap;
use crate::components::sample::PlayOrder::{Random, Sequential};
use crate::profile::Attribute;

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

    pub fn parse_sample_root(&mut self, attributes: &Vec<Attribute>) -> Result<()> {
        for attr in attributes {
            if attr.name.ends_with("state") && self.element_name != "sampleClear" {
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

    pub fn parse_sample_stack(&mut self, id: char, attributes: &Vec<Attribute>) -> Result<()> {
        // The easiest way to handle this is to parse everything into key-value pairs, then try
        // to locate all the settings for each track inside it..
        let mut map: HashMap<String, String> = HashMap::default();

        for attr in attributes {
            map.insert(attr.name.clone(), attr.value.clone());
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
        let key = format!("sampleStack{id}stackSize");

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
                    map.get(&format!("track_{i}")),
                    map.get(&format!("track_{i}StartPosition")),
                    map.get(&format!("track_{i}EndPosition")),
                    map.get(&format!("track_{i}NormalizedGain")),
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

    pub fn write_sample<W: Write>(&self, writer: &mut Writer<W>) -> Result<()> {
        let mut elem = BytesStart::new(self.element_name.as_str());

        let mut attributes: HashMap<String, String> = HashMap::default();
        self.colour_map.write_colours(&mut attributes);

        // TODO: Solve the 'State' problem properly..
        /*
        This is somewhat dependant on the 'Active' stack, and whether this button has any
        tracks assigned to it. If there are tracks, it should be 'Stopped', if there are no
        tracks it should be 'Empty'. Given the contexts here, this should be handled at the
        profile management level.

        More annoyingly though, unlike every other profile component, this *HAS* to override
        the colour 'state' settings, so we write it last, unless it's sampleClear :)
         */
        if self.element_name != "sampleClear" {
            attributes.insert(
                format!("{}state", self.element_name),
                self.state.to_string(),
            );
        }

        // Write out the attributes etc for this element, but don't close it yet..
        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }
        writer.write_event(Event::Start(elem))?;

        // Now onto the damn stacks..
        for (key, value) in &self.sample_stack {
            let sub_element_name = format!("sampleStack{key}");

            let mut sub_elem = BytesStart::new(sub_element_name.as_str());

            // Welcome to the only place where order seems to matter, the track_X attributes must all appear together
            // in an ordered, unbroken list, otherwise the GoXLR App will crash :D
            let mut sub_attributes: LinkedHashMap<String, String> = Default::default();

            for i in 0..value.tracks.len() {
                sub_attributes.insert(
                    format!("track_{i}"),
                    value.tracks.get(i).unwrap().track.to_string(),
                );
            }

            if !value.tracks.is_empty() {
                sub_attributes.insert(
                    format!("sampleStack{key}stackSize"),
                    format!("{}", value.tracks.len()),
                );
            }

            for i in 0..value.tracks.len() {
                sub_attributes.insert(
                    format!("track_{i}NormalizedGain"),
                    format!("{}", value.tracks.get(i).unwrap().normalized_gain),
                );
                sub_attributes.insert(
                    format!("track_{i}StartPosition"),
                    format!("{}", value.tracks.get(i).unwrap().start_position),
                );
                sub_attributes.insert(
                    format!("track_{i}EndPosition"),
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
                sub_elem.push_attribute((key.as_str(), value.as_str()));
            }
            writer.write_event(Event::Empty(sub_elem))?;
        }

        writer.write_event(Event::End(BytesEnd::new(self.element_name.as_str())))?;
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
    pub fn get_stack_mut(&mut self, bank: SampleBank) -> &mut SampleStack {
        self.sample_stack.get_mut(&bank).unwrap()
    }
}

#[derive(Debug)]
pub struct SampleStack {
    tracks: Vec<Track>,
    playback_mode: Option<PlaybackMode>,
    play_order: Option<PlayOrder>,

    // Transient value, keep track of where we may be sequentially..
    transient_seq_position: usize,
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

            transient_seq_position: 0,
        }
    }

    pub fn get_playback_mode(&self) -> PlaybackMode {
        if let Some(mode) = self.playback_mode {
            return mode;
        }
        PlaybackMode::PlayNext
    }

    pub fn get_play_order(&self) -> PlayOrder {
        if let Some(order) = self.play_order {
            return order;
        }
        Sequential
    }

    pub fn get_tracks(&self) -> &Vec<Track> {
        &self.tracks
    }
    pub fn get_track_by_index(&self, index: usize) -> Result<&Track> {
        if self.tracks.len() <= index {
            bail!("Track not Found");
        }
        Ok(&self.tracks[index])
    }
    pub fn get_track_by_index_mut(&mut self, index: usize) -> Result<&mut Track> {
        if self.tracks.len() <= index {
            bail!("Track not Found");
        }
        Ok(&mut self.tracks[index])
    }

    pub fn get_track_count(&self) -> usize {
        self.tracks.len()
    }
    pub fn get_first_track(&self) -> &Track {
        &self.tracks[0]
    }

    pub fn get_next_track(&mut self) -> Option<&Track> {
        if self.get_track_count() == 1 {
            return Some(self.get_first_track());
        }

        let mut play_order = &self.play_order;
        if play_order.is_none() {
            play_order = &Some(Sequential)
        }

        if let Some(order) = play_order {
            // Per the Windows App, if there are only 2 tracks with 'Random' order, behave
            // sequentially.
            if order == &Sequential || (order == &Random && self.tracks.len() <= 2) {
                let track = &self.tracks[self.transient_seq_position];
                self.transient_seq_position += 1;

                if self.transient_seq_position >= self.tracks.len() {
                    self.transient_seq_position = 0;
                }

                return Some(track);
            } else if order == &Random {
                let track = self.tracks.choose(&mut rand::thread_rng());
                if let Some(track) = track {
                    return Some(track);
                }
            }
        }
        None
    }

    pub fn set_playback_mode(&mut self, playback_mode: Option<PlaybackMode>) {
        self.playback_mode = playback_mode;
    }
    pub fn set_play_order(&mut self, play_order: Option<PlayOrder>) {
        self.play_order = play_order;
    }

    pub fn add_track(&mut self, track: Track) -> &mut Track {
        self.tracks.push(track);
        let len = self.tracks.len();
        &mut self.tracks[len - 1]
    }

    pub fn remove_track_by_index(&mut self, track: usize) {
        self.tracks.remove(track);
    }
    pub fn clear_tracks(&mut self) {
        self.tracks.clear();
    }
}

#[derive(Debug, Clone)]
pub struct Track {
    pub track: String,
    pub start_position: f32,
    pub end_position: f32,
    pub normalized_gain: f64,
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

    pub fn track(&self) -> &str {
        &self.track
    }
    pub fn start_position(&self) -> f32 {
        self.start_position
    }
    pub fn end_position(&self) -> f32 {
        self.end_position
    }
    pub fn normalized_gain(&self) -> f64 {
        self.normalized_gain
    }
}

#[derive(Debug, Copy, Clone, Enum, EnumProperty)]
pub enum PlaybackMode {
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

#[derive(Debug, Copy, Clone, Enum, EnumProperty, Eq, PartialEq)]
pub enum PlayOrder {
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
