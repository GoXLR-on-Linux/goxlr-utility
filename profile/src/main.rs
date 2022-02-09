use std::fs::File;
use std::io::BufReader;
use std::process::exit;
use std::str::FromStr;

use enum_map::{Enum, EnumMap};
use xml::reader::XmlEvent as XmlReaderEvent;
use xml::{EmitterConfig, EventReader};

use crate::components::browser::BrowserPreviewTree;
use crate::components::context::Context;
use crate::components::echo::EchoEncoderBase;
use crate::components::effects::Effects;
use crate::components::fader::Fader;
use crate::components::gender::GenderEncoderBase;
use crate::components::hardtune::HardtuneEffectBase;
use crate::components::megaphone::MegaphoneEffectBase;
use crate::components::mixer::Mixers;
use crate::components::mute::MuteButton;
use crate::components::mute_chat::MuteChat;
use crate::components::pitch::PitchEncoderBase;
use crate::components::reverb::ReverbEncoderBase;
use crate::components::robot::RobotEffectBase;
use crate::components::root::RootElement;
use crate::components::sample::SampleBase;
use crate::components::scribble::Scribble;
use crate::components::simple::SimpleElement;
use crate::SampleButtons::{BOTTOM_LEFT, BOTTOM_RIGHT, CLEAR, TOP_LEFT, TOP_RIGHT};

mod components;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut root = RootElement::new();
    let mut browser = BrowserPreviewTree::new("browserPreviewTree".to_string());

    let mut mixer = Mixers::new();
    let mut context = Context::new("selectedContext".to_string());
    let mut mute_chat = MuteChat::new("muteChat".to_string());

    // A lot of these Vec's will need tidying up, some will work as EnumMap, or other such stuff..
    // For now, all I'm doing is testing reading and writing, I'll do final structuing later.
    let mut mute_buttons: Vec<MuteButton> = Vec::new();
    mute_buttons.reserve_exact(4);

    let mut faders: Vec<Fader> = Vec::new();
    faders.reserve_exact(4);

    let mut effects: Vec<Effects> = Vec::new();
    effects.reserve_exact(6);

    let mut scribbles: Vec<Scribble> = Vec::new();
    scribbles.reserve_exact(4);

    let mut simple_elements: Vec<SimpleElement> = Vec::new();

    let mut megaphone_effect = MegaphoneEffectBase::new("megaphoneEffect".to_string());
    let mut robot_effect = RobotEffectBase::new("robotEffect".to_string());
    let mut hardtune_effect = HardtuneEffectBase::new("hardtuneEffect".to_string());
    let mut reverb_encoder = ReverbEncoderBase::new("reverbEncoder".to_string());
    let mut echo_encoder = EchoEncoderBase::new("echoEncoder".to_string());
    let mut pitch_encoder = PitchEncoderBase::new("pitchEncoder".to_string());
    let mut gender_encoder = GenderEncoderBase::new("genderEncoder".to_string());

    let mut sampler_map: EnumMap<SampleButtons, Option<SampleBase>> = EnumMap::default();

    let file = File::open("test-data/profile.xml").unwrap();
    let file = BufReader::new(file);

    let parser = EventReader::new(file);

    let mut active_sample_button = Option::None;

    for e in parser {
        match e {
            Ok(XmlReaderEvent::StartElement {
                name, attributes, ..
            }) => {
                if name.local_name == "ValueTreeRoot" {
                    // This also handles <AppTree, due to a single shared value.
                    root.parse_root(&attributes);

                    // This code was made for XML version 2, v1 not currently supported.
                    if root.get_version() > 2 {
                        println!("XML Version Not Supported: {}", root.get_version());
                        exit(-1);
                    }

                    if root.get_version() < 2 {
                        println!(
                            "XML Version {} detected, will be upgraded to v2",
                            root.get_version()
                        );
                    }
                    continue;
                }

                if name.local_name == "browserPreviewTree" {
                    browser.parse_browser(&attributes);
                    continue;
                }

                if name.local_name == "mixerTree" {
                    mixer.parse_mixers(&attributes);
                    continue;
                }

                if name.local_name == "selectedContext" {
                    context.parse_context(&attributes);
                    continue;
                }

                if name.local_name == "muteChat" {
                    mute_chat.parse_mute_chat(&attributes);
                    continue;
                }

                // Might need to pattern match this..
                if name.local_name.starts_with("mute") && name.local_name != "muteChat" {
                    // In the XML, the count starts as 1, here, we're gonna store as 0.
                    let id =
                        u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str())
                            .unwrap()
                            - 1;
                    let mut mute_button = MuteButton::new(id + 1);
                    mute_button.parse_button(&attributes);
                    mute_buttons.insert(id as usize, mute_button);
                    continue;
                }

                if name.local_name.starts_with("FaderMeter") {
                    // In the XML, the count starts at 0, and we have different capitalisation :D
                    let id =
                        u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str())
                            .unwrap();
                    let mut fader = Fader::new(id);
                    fader.parse_fader(&attributes);
                    faders.insert(id as usize, fader);
                    continue;
                }

                if name.local_name.starts_with("effects") {
                    let id =
                        u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str())
                            .unwrap()
                            - 1;
                    let mut effect = Effects::new(id + 1);
                    effect.parse_effect(&attributes);
                    effects.insert(id as usize, effect);
                    continue;
                }

                if name.local_name.starts_with("scribble") {
                    let id =
                        u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str())
                            .unwrap()
                            - 1;
                    let mut scribble = Scribble::new(id + 1);
                    scribble.parse_scribble(&attributes);
                    scribbles.insert(id as usize, scribble);
                    continue;
                }

                if name.local_name == "megaphoneEffect" {
                    megaphone_effect.parse_megaphone_root(&attributes);
                    continue;
                }

                // Because the depth is crazy small, and tag names don't ever repeat themselves, there's really no point
                // tracking the opening and closing of tags except when writing, so we'll continue treating the reading
                // as if it were a very flat structure.
                if name.local_name.starts_with("megaphoneEffectpreset") {
                    let id =
                        u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str())
                            .unwrap();
                    megaphone_effect.parse_megaphone_preset(id, &attributes);
                    continue;
                }

                if name.local_name == "robotEffect" {
                    robot_effect.parse_robot_root(&attributes);
                    continue;
                }

                if name.local_name.starts_with("robotEffectpreset") {
                    let id =
                        u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str())
                            .unwrap();
                    robot_effect.parse_robot_preset(id, &attributes);
                    continue;
                }

                if name.local_name == "hardtuneEffect" {
                    hardtune_effect.parse_hardtune_root(&attributes);
                    continue;
                }

                if name.local_name.starts_with("hardtuneEffectpreset") {
                    let id =
                        u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str())
                            .unwrap();
                    hardtune_effect.parse_hardtune_preset(id, &attributes);
                    continue;
                }

                if name.local_name == "reverbEncoder" {
                    reverb_encoder.parse_reverb_root(&attributes);
                    continue;
                }

                if name.local_name.starts_with("reverbEncoderpreset") {
                    let id =
                        u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str())
                            .unwrap();
                    reverb_encoder.parse_reverb_preset(id, &attributes);
                    continue;
                }

                if name.local_name == "echoEncoder" {
                    echo_encoder.parse_echo_root(&attributes);
                    continue;
                }

                if name.local_name.starts_with("echoEncoderpreset") {
                    let id =
                        u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str())
                            .unwrap();
                    echo_encoder.parse_echo_preset(id, &attributes);
                    continue;
                }

                if name.local_name == "pitchEncoder" {
                    pitch_encoder.parse_pitch_root(&attributes);
                    continue;
                }

                if name.local_name.starts_with("pitchEncoderpreset") {
                    let id =
                        u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str())
                            .unwrap();
                    pitch_encoder.parse_pitch_preset(id, &attributes);
                    continue;
                }

                if name.local_name == "genderEncoder" {
                    gender_encoder.parse_gender_root(&attributes);
                    continue;
                }

                if name.local_name.starts_with("genderEncoderpreset") {
                    let id =
                        u8::from_str(name.local_name.chars().last().unwrap().to_string().as_str())
                            .unwrap();
                    gender_encoder.parse_gender_preset(id, &attributes);
                    continue;
                }

                // These can probably be a little cleaner..
                if name.local_name == "sampleTopLeft" {
                    let mut sampler = SampleBase::new("sampleTopLeft".to_string());
                    sampler.parse_sample_root(&attributes);
                    sampler_map[TOP_LEFT] = Option::Some(sampler);
                    active_sample_button = sampler_map[TOP_LEFT].as_mut();
                    continue;
                }

                if name.local_name == "sampleTopRight" {
                    let mut sampler = SampleBase::new("sampleTopRight".to_string());
                    sampler.parse_sample_root(&attributes);
                    sampler_map[TOP_RIGHT] = Option::Some(sampler);
                    active_sample_button = sampler_map[TOP_RIGHT].as_mut();
                    continue;
                }

                if name.local_name == "sampleBottomLeft" {
                    let mut sampler = SampleBase::new("sampleBottomLeft".to_string());
                    sampler.parse_sample_root(&attributes);
                    sampler_map[BOTTOM_LEFT] = Option::Some(sampler);
                    active_sample_button = sampler_map[BOTTOM_LEFT].as_mut();
                    continue;
                }

                if name.local_name == "sampleBottomRight" {
                    let mut sampler = SampleBase::new("sampleBottomRight".to_string());
                    sampler.parse_sample_root(&attributes);
                    sampler_map[BOTTOM_RIGHT] = Option::Some(sampler);
                    active_sample_button = sampler_map[BOTTOM_RIGHT].as_mut();
                    continue;
                }

                if name.local_name == "sampleClear" {
                    let mut sampler = SampleBase::new("sampleClear".to_string());
                    sampler.parse_sample_root(&attributes);
                    sampler_map[CLEAR] = Option::Some(sampler);
                    active_sample_button = sampler_map[CLEAR].as_mut();
                    continue;
                }

                if name.local_name.starts_with("sampleStack") {
                    let id = name.local_name.chars().last().unwrap();
                    active_sample_button
                        .as_mut()
                        .unwrap()
                        .parse_sample_stack(id, &attributes);
                    continue;
                }

                if name.local_name.starts_with("sampleBank")
                    || name.local_name == "fxClear"
                    || name.local_name == "swear"
                    || name.local_name == "globalColour"
                    || name.local_name == "logoX"
                {
                    // In this case, the tag name, and attribute prefixes are the same..
                    let mut simple_element = SimpleElement::new(name.local_name.clone());
                    simple_element.parse_simple(&attributes);
                    simple_elements.push(simple_element);
                    continue;
                }

                if name.local_name == "AppTree" {
                    // This is handled by ValueTreeRoot
                    continue;
                }

                println!("Unhandled Tag: {}", name.local_name);
            }

            Ok(XmlReaderEvent::EndElement { name }) => {
                // This probably isn't needed, but cleans up the variable once the stacks have been
                // read.
                if name.local_name == "sampleTopLeft"
                    || name.local_name == "sampleTopRight"
                    || name.local_name == "sampleBottomLeft"
                    || name.local_name == "sampleBottomRight"
                    || name.local_name == "sampleClear"
                {
                    active_sample_button = Option::None;
                }
            }
            Err(e) => {
                println!("Error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Create the file, and the writer..
    let mut out_file = File::create("test-data/output.xml").unwrap();
    let mut writer = EmitterConfig::new()
        .perform_indent(true)
        .create_writer(&mut out_file);

    // Write the initial root tag..
    root.write_initial(&mut writer)?;
    browser.write_browser(&mut writer)?;

    mixer.write_mixers(&mut writer)?;
    context.write_context(&mut writer)?;
    mute_chat.write_mute_chat(&mut writer)?;

    for mute_button in mute_buttons.iter() {
        mute_button.write_button(&mut writer)?;
    }

    for fader in faders.iter() {
        fader.write_fader(&mut writer)?;
    }

    for effect in effects.iter() {
        effect.write_effects(&mut writer)?;
    }

    for scribble in scribbles.iter() {
        scribble.write_scribble(&mut writer)?;
    }

    megaphone_effect.write_megaphone(&mut writer)?;
    robot_effect.write_robot(&mut writer)?;
    hardtune_effect.write_hardtune(&mut writer)?;

    reverb_encoder.write_reverb(&mut writer)?;
    echo_encoder.write_echo(&mut writer)?;
    pitch_encoder.write_pitch(&mut writer)?;
    gender_encoder.write_gender(&mut writer)?;

    for (_key, value) in sampler_map {
        if value.is_some() {
            value.unwrap().write_sample(&mut writer)?;
        }
    }

    for simple_element in simple_elements {
        simple_element.write_simple(&mut writer)?;
    }

    // Finalise the XML..
    root.write_final(&mut writer)?;
    Ok(())
}

struct GoXLR {
    mixer: Mixers,
}

#[derive(Debug, Enum)]
enum SampleButtons {
    TOP_LEFT,
    TOP_RIGHT,
    BOTTOM_LEFT,
    BOTTOM_RIGHT,
    CLEAR,
}
