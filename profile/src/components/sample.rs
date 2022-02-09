use std::collections::HashMap;
use std::fs::File;

use enum_map::Enum;
use ritelinked::LinkedHashMap;
use strum::EnumProperty;
use xml::attribute::OwnedAttribute;
use xml::writer::events::StartElementBuilder;
use xml::writer::XmlEvent as XmlWriterEvent;
use xml::EventWriter;

use crate::components::colours::ColourMap;

/**
 * This is relatively static, main tag contains standard colour mapping, subtags contain various
 * presets, we'll use an EnumMap to define the 'presets' as they'll be useful for the other various
 * 'types' of presets (encoders and effects).
 */

pub struct SampleBase {
    element_name: String,
    colour_map: ColourMap,
    state: String, // Seems to be "Empty" most of the time..
    sample_stack: HashMap<char, SampleStack>,
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

    pub fn parse_sample_root(&mut self, attributes: &[OwnedAttribute]) {
        for attr in attributes {
            if attr.name.local_name.ends_with("state") && self.element_name != "sampleClear" {
                if attr.value != "Empty" && attr.value != "Stopped" {
                    println!("[Sampler] Unknown State: {}", &attr.value);
                }
                self.state = attr.value.clone();
                continue;
            }

            if !self.colour_map.read_colours(attr) {
                println!("[Sampler] Unparsed Attribute: {}", attr.name);
            }
        }
    }

    pub fn parse_sample_stack(&mut self, id: char, attributes: &[OwnedAttribute]) {
        // The easiest way to handle this is to parse everything into key-value pairs, then try
        // to locate all the settings for each track inside it..
        let mut map: HashMap<String, String> = HashMap::default();

        for attr in attributes {
            map.insert(attr.name.local_name.clone(), attr.value.clone());
        }

        let mut sample_stack = SampleStack::new();

        // Pull out any 'extra' attributes which may be useful..
        if map.contains_key("playbackMode") {
            sample_stack.playback_mode = Option::Some(PlaybackMode::from_usize(
                map.get("playbackMode").unwrap().parse::<usize>().unwrap(),
            ));
        }

        if map.contains_key("playOrder") {
            sample_stack.play_order = Option::Some(PlayOrder::from_usize(
                map.get("playOrder").unwrap().parse::<usize>().unwrap(),
            ));
        }

        // Ok, somewhere in here we should have a key that tells us how many tracks are configured..
        let key = format!("sampleStack{}stackSize", id);

        if !map.contains_key(key.as_str()) {
            // Stack doesn't contain any tracks, we're done here.
            self.sample_stack.insert(id, sample_stack);
            return;
        }

        let track_count: u8 = map.get(key.as_str()).unwrap().parse().unwrap();

        for i in 0..track_count {
            let track = Track::new(
                map.get(format!("track_{}", i).as_str()).unwrap().clone(),
                map.get(format!("track_{}StartPosition", i).as_str())
                    .unwrap()
                    .parse()
                    .unwrap(),
                map.get(format!("track_{}EndPosition", i).as_str())
                    .unwrap()
                    .parse()
                    .unwrap(),
                map.get(format!("track_{}NormalizedGain", i).as_str())
                    .unwrap()
                    .parse()
                    .unwrap(),
            );
            sample_stack.tracks.push(track);
        }

        self.sample_stack.insert(id, sample_stack);
    }

    pub fn write_sample(
        &self,
        writer: &mut EventWriter<&mut File>,
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

            if value.playback_mode.is_some() {
                let output = value.playback_mode.as_ref().unwrap();
                sub_attributes.insert(
                    "playbackMode".to_string(),
                    output.get_str("index").unwrap().to_string(),
                );
            }

            if value.play_order.is_some() {
                let order = value.play_order.as_ref().unwrap();
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
}

#[derive(Debug)]
struct SampleStack {
    tracks: Vec<Track>,
    playback_mode: Option<PlaybackMode>,
    play_order: Option<PlayOrder>,
}

impl SampleStack {
    pub fn new() -> Self {
        Self {
            tracks: vec![],
            playback_mode: None,
            play_order: None,
        }
    }
}

#[derive(Debug)]
struct Track {
    track: String,
    start_position: u8,
    end_position: u8,
    normalized_gain: f64,
}

impl Track {
    pub fn new(track: String, start_position: u8, end_position: u8, normalized_gain: f64) -> Self {
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
    PLAY_NEXT,
    #[strum(props(index = "1"))]
    PLAY_STOP,
    #[strum(props(index = "2"))]
    PLAY_FADE,
    #[strum(props(index = "3"))]
    STOP_ON_RELEASE,
    #[strum(props(index = "4"))]
    FADE_ON_RELEASE,
    #[strum(props(index = "5"))]
    LOOP,
}

#[derive(Debug, Enum, EnumProperty)]
enum PlayOrder {
    #[strum(props(index = "0"))]
    SEQUENTIAL,
    #[strum(props(index = "1"))]
    RANDOM,
}
