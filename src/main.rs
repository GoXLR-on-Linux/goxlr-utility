mod components;

use std::fs::File;
use std::io::BufReader;
use std::str::FromStr;
use xml::{EmitterConfig, EventReader};
use xml::reader::XmlEvent as XmlReaderEvent;
use crate::components::effects::Effects;
use crate::components::fader::Fader;
use crate::components::mixer::{Mixers};
use crate::components::mute::MuteButton;
use crate::components::scribble::Scribble;
use crate::components::simple::SimpleElement;

fn main() {

    let mut mixer = Mixers::new();

    // A lot of these Vec's will need tidying up, some will work as EnumMap, or other such stuff..
    // For now, all I'm doing is testing reading and writing, I'll do final structuing later.
    let mut muteButtons: Vec<MuteButton> = Vec::new();
    muteButtons.reserve_exact(4);

    let mut faders: Vec<Fader> = Vec::new();
    faders.reserve_exact(4);

    let mut effects: Vec<Effects> = Vec::new();
    effects.reserve_exact(6);

    let mut scribbles: Vec<Scribble> = Vec::new();
    scribbles.reserve_exact(4);

    let mut simpleElements: Vec<SimpleElement> = Vec::new();

    let file = File::open("test-data/profile.xml").unwrap();
    let file = BufReader::new(file);

    let parser = EventReader::new(file);
    let mut depth = 0;
    for e in parser {
        match e {
            Ok(XmlReaderEvent::StartElement { name, attributes, .. }) => {
                if name.local_name == "mixerTree" {
                    mixer.parse_mixers(&attributes);
                }

                // Might need to pattern match this..
                if name.local_name.starts_with("mute") && name.local_name != "muteChat" {
                    // In the XML, the count starts as 1, here, we're gonna store as 0.
                    let id = u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str()).unwrap() - 1;
                    let mut muteButton = MuteButton::new(id + 1);
                    muteButton.parse_button(&attributes);
                    muteButtons.insert(id as usize, muteButton);
                }

                if name.local_name.starts_with("FaderMeter") {
                    // In the XML, the count starts at 0, and we have different capitalisation :D
                    let id = u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str()).unwrap();
                    let mut fader = Fader::new(id);
                    fader.parse_fader(&attributes);
                    faders.insert(id as usize, fader);
                }

                if name.local_name.starts_with("effects") {
                    let id = u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str()).unwrap() - 1;
                    let mut effect = Effects::new(id + 1);
                    effect.parse_effect(&attributes);
                    effects.insert(id as usize, effect);
                }

                if name.local_name.starts_with("scribble") {
                    let id = u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str()).unwrap() - 1;
                    let mut scribble = Scribble::new(id + 1);
                    scribble.parse_scribble(&attributes);
                    scribbles.insert(id as usize, scribble);
                }

                if name.local_name.starts_with("sampleBank")
                    || name.local_name == "fxClear"
                    || name.local_name == "swear"
                    || name.local_name == "globalColour"
                    || name.local_name == "logoX"
                {
                    // In this case, the tag name, and attribute prefixes are the same..
                    let mut simpleElement = SimpleElement::new(name.local_name.clone());
                    simpleElement.parse_simple(&attributes);
                    simpleElements.push(simpleElement);
                }

                // MISSING:
                // ValueTreeRoot
                // browserPreviewTree
                // selectedContext
                // muteChat
                //
                // megaphoneEffect
                // robotEffect
                // hardtuneEffect
                //
                // reverbEncoder
                // echoEncoder
                // pitchEncoder
                // genderEncoder
                //
                // sampleTopLeft
                // sampleTopRight
                // sampleBottomLeft
                // sampleBottomRight
                // sampleClear (this is a regular button, but has a sample stack attached to it O_o)
                //
                // AppTree



            }
            Ok(XmlReaderEvent::EndElement { name }) => {
                // This will be more relevant when I start hitting Encoders, Samples and Effects!
            }
            Err(e) => {
                println!("Error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Create the file, and the writer..
    let mut outFile = File::create("test-data/output.xml").unwrap();
    let mut writer = EmitterConfig::new().perform_indent(true).create_writer(&mut outFile);
    mixer.write_mixers(&mut writer);
    for muteButton in muteButtons.iter() {
        muteButton.write_button(&mut writer);
    }

    for fader in faders.iter() {
        fader.write_fader(&mut writer);
    }

    for effect in effects.iter() {
        effect.write_effects(&mut writer);
    }

    for scribble in scribbles.iter() {
        scribble.write_scribble(&mut writer);
    }

    for simpleElement in simpleElements {
        simpleElement.write_simple(&mut writer);
    }

}

struct GoXLR {
    mixer: Mixers,
}




