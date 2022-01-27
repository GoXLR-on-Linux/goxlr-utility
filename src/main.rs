mod components;

use std::fs::File;
use std::io::BufReader;
use std::str::FromStr;
use xml::{EmitterConfig, EventReader};
use xml::reader::XmlEvent as XmlReaderEvent;
use crate::components::mixer::{Mixers};
use crate::components::mute::MuteButton;

fn main() {

    let mut mixer = Mixers::new();
    let mut muteButtons: Vec<MuteButton> = Vec::new();
    muteButtons.reserve(4);

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
        println!("Writing Button..");
        muteButton.write_button(&mut writer);
    }
}

struct GoXLR {
    mixer: Mixers,
}




