use std::env;
use std::fs::{create_dir, File, read_dir, remove_dir_all};
use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::process::exit;
use std::str::FromStr;

use enum_map::EnumMap;
use xml::{EmitterConfig, EventReader};
use xml::reader::XmlEvent as XmlReaderEvent;
use zip::write::FileOptions;
use strum::IntoEnumIterator;

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
use crate::components::simple::{SimpleElement, SimpleElements};
use crate::error::{ParseError, SaveError};
use crate::SampleButtons;
use crate::SampleButtons::{BottomLeft, BottomRight, Clear, TopLeft, TopRight};

#[derive(Debug)]
pub struct Profile {
    settings: ProfileSettings,
}

impl Profile {
    pub fn load<R: Read + std::io::Seek>(read: R) -> Result<Self, ParseError> {
        let mut archive = zip::ZipArchive::new(read)?;
        let settings = ProfileSettings::load(archive.by_name("profile.xml")?)?;
        Ok(Profile { settings })
    }

    /**
     * I have no idea what I'm doing, I couldn't find an easy way to just modify the archive,
     * so instead extract it, edit it and try to piece it all back together again!
     */
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SaveError> {
        dbg!("Saving file: {}", &path.as_ref());

        // Create a temporary directory, and extract the contents of the zip..
        let temporary_directory = env::temp_dir().join("goxlr-profile");
        if temporary_directory.exists() {
            remove_dir_all(temporary_directory.clone())?;
        }

        create_dir(temporary_directory.clone())?;

        // Open the original archive..
        let mut archive = zip::ZipArchive::new(BufReader::new(File::open(path.as_ref())?))?;
        archive.extract(temporary_directory.clone())?;

        // Replace the profile.xml file in the profile dir..
        let profile_file = temporary_directory.clone().join("profile.xml");
        self.settings.write(profile_file)?;

        // Ok, now we need to package it all back up..
        let file = File::create(path.as_ref())?;
        let mut archive = zip::ZipWriter::new(file);

        let files = read_dir(temporary_directory.clone())?;
        for file in files {
            let file_path = file.as_ref().unwrap().path();
            let file_name = file.as_ref().unwrap().file_name().into_string().unwrap().clone();

            archive.start_file(file_name, FileOptions::default())?;
            let mut f = File::open(file_path)?;

            let mut buffer: Vec<u8> = Vec::new();
            f.read_to_end(&mut buffer)?;
            archive.write_all(&*buffer)?;
            buffer.clear();
        }
        archive.finish()?;

        remove_dir_all(temporary_directory.clone())?;
        Ok(())
    }

    pub fn settings_mut(&mut self) -> &mut ProfileSettings {
        &mut self.settings
    }

    pub fn settings(&self) -> &ProfileSettings {
        &self.settings
    }
}

#[derive(Debug)]
pub struct ProfileSettings {
    root: RootElement,
    browser: BrowserPreviewTree,
    mixer: Mixers,
    context: Context,
    mute_chat: MuteChat,
    mute_buttons: Vec<MuteButton>,
    faders: Vec<Fader>,
    effects: Vec<Effects>,
    scribbles: Vec<Scribble>,
    sampler_map: EnumMap<SampleButtons, Option<SampleBase>>,
    simple_elements: EnumMap<SimpleElements, Option<SimpleElement>>,
    megaphone_effect: MegaphoneEffectBase,
    robot_effect: RobotEffectBase,
    hardtune_effect: HardtuneEffectBase,
    reverb_encoder: ReverbEncoderBase,
    echo_encoder: EchoEncoderBase,
    pitch_encoder: PitchEncoderBase,
    gender_encoder: GenderEncoderBase,
}

impl ProfileSettings {
    pub fn load<R: Read>(read: R) -> Result<Self, ParseError> {
        let parser = EventReader::new(read);

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

        let mut simple_elements: EnumMap<SimpleElements, Option<SimpleElement>> = Default::default();


        let mut megaphone_effect = MegaphoneEffectBase::new("megaphoneEffect".to_string());
        let mut robot_effect = RobotEffectBase::new("robotEffect".to_string());
        let mut hardtune_effect = HardtuneEffectBase::new("hardtuneEffect".to_string());
        let mut reverb_encoder = ReverbEncoderBase::new("reverbEncoder".to_string());
        let mut echo_encoder = EchoEncoderBase::new("echoEncoder".to_string());
        let mut pitch_encoder = PitchEncoderBase::new("pitchEncoder".to_string());
        let mut gender_encoder = GenderEncoderBase::new("genderEncoder".to_string());

        let mut sampler_map: EnumMap<SampleButtons, Option<SampleBase>> = EnumMap::default();

        let mut active_sample_button = Option::None;

        for e in parser {
            match e {
                Ok(XmlReaderEvent::StartElement {
                    name, attributes, ..
                }) => {
                    if name.local_name == "ValueTreeRoot" {
                        // This also handles <AppTree, due to a single shared value.
                        root.parse_root(&attributes)?;

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
                        browser.parse_browser(&attributes)?;
                        continue;
                    }

                    if name.local_name == "mixerTree" {
                        mixer.parse_mixers(&attributes)?;
                        continue;
                    }

                    if name.local_name == "selectedContext" {
                        context.parse_context(&attributes)?;
                        continue;
                    }

                    if name.local_name == "muteChat" {
                        mute_chat.parse_mute_chat(&attributes)?;
                        continue;
                    }

                    // Might need to pattern match this..
                    if name.local_name.starts_with("mute") && name.local_name != "muteChat" {
                        // In the XML, the count starts as 1, here, we're gonna store as 0.
                        if let Some(id) = name
                            .local_name
                            .chars()
                            .last()
                            .map(|s| u8::from_str(&s.to_string()))
                            .transpose()?
                        {
                            let mut mute_button = MuteButton::new(id);
                            mute_button.parse_button(&attributes)?;
                            mute_buttons.insert(id as usize - 1, mute_button);
                            continue;
                        }
                    }

                    if name.local_name.starts_with("FaderMeter") {
                        // In the XML, the count starts at 0, and we have different capitalisation :D
                        if let Some(id) = name
                            .local_name
                            .chars()
                            .last()
                            .map(|s| u8::from_str(&s.to_string()))
                            .transpose()?
                        {
                            let mut fader = Fader::new(id);
                            fader.parse_fader(&attributes)?;
                            faders.insert(id as usize, fader);
                            continue;
                        }
                    }

                    if name.local_name.starts_with("effects") {
                        if let Some(id) = name
                            .local_name
                            .chars()
                            .last()
                            .map(|s| u8::from_str(&s.to_string()))
                            .transpose()?
                        {
                            let mut effect = Effects::new(id);
                            effect.parse_effect(&attributes)?;
                            effects.insert(id as usize - 1, effect);
                            continue;
                        }
                    }

                    if name.local_name.starts_with("scribble") {
                        if let Some(id) = name
                            .local_name
                            .chars()
                            .last()
                            .map(|s| u8::from_str(&s.to_string()))
                            .transpose()?
                        {
                            let mut scribble = Scribble::new(id);
                            scribble.parse_scribble(&attributes)?;
                            scribbles.insert(id as usize - 1, scribble);
                            continue;
                        }
                    }

                    if name.local_name == "megaphoneEffect" {
                        megaphone_effect.parse_megaphone_root(&attributes)?;
                        continue;
                    }

                    // Because the depth is crazy small, and tag names don't ever repeat themselves, there's really no point
                    // tracking the opening and closing of tags except when writing, so we'll continue treating the reading
                    // as if it were a very flat structure.
                    if name.local_name.starts_with("megaphoneEffectpreset") {
                        if let Some(id) = name
                            .local_name
                            .chars()
                            .last()
                            .map(|s| u8::from_str(&s.to_string()))
                            .transpose()?
                        {
                            megaphone_effect.parse_megaphone_preset(id, &attributes)?;
                            continue;
                        }
                    }

                    if name.local_name == "robotEffect" {
                        robot_effect.parse_robot_root(&attributes)?;
                        continue;
                    }

                    if name.local_name.starts_with("robotEffectpreset") {
                        if let Some(id) = name
                            .local_name
                            .chars()
                            .last()
                            .map(|s| u8::from_str(&s.to_string()))
                            .transpose()?
                        {
                            robot_effect.parse_robot_preset(id, &attributes)?;
                            continue;
                        }
                    }

                    if name.local_name == "hardtuneEffect" {
                        hardtune_effect.parse_hardtune_root(&attributes)?;
                        continue;
                    }

                    if name.local_name.starts_with("hardtuneEffectpreset") {
                        if let Some(id) = name
                            .local_name
                            .chars()
                            .last()
                            .map(|s| u8::from_str(&s.to_string()))
                            .transpose()?
                        {
                            hardtune_effect.parse_hardtune_preset(id, &attributes)?;
                            continue;
                        }
                    }

                    if name.local_name == "reverbEncoder" {
                        reverb_encoder.parse_reverb_root(&attributes)?;
                        continue;
                    }

                    if name.local_name.starts_with("reverbEncoderpreset") {
                        if let Some(id) = name
                            .local_name
                            .chars()
                            .last()
                            .map(|s| u8::from_str(&s.to_string()))
                            .transpose()?
                        {
                            reverb_encoder.parse_reverb_preset(id, &attributes)?;
                            continue;
                        }
                    }

                    if name.local_name == "echoEncoder" {
                        echo_encoder.parse_echo_root(&attributes)?;
                        continue;
                    }

                    if name.local_name.starts_with("echoEncoderpreset") {
                        if let Some(id) = name
                            .local_name
                            .chars()
                            .last()
                            .map(|s| u8::from_str(&s.to_string()))
                            .transpose()?
                        {
                            echo_encoder.parse_echo_preset(id, &attributes)?;
                            continue;
                        }
                    }

                    if name.local_name == "pitchEncoder" {
                        pitch_encoder.parse_pitch_root(&attributes)?;
                        continue;
                    }

                    if name.local_name.starts_with("pitchEncoderpreset") {
                        if let Some(id) = name
                            .local_name
                            .chars()
                            .last()
                            .map(|s| u8::from_str(&s.to_string()))
                            .transpose()?
                        {
                            pitch_encoder.parse_pitch_preset(id, &attributes)?;
                            continue;
                        }
                    }

                    if name.local_name == "genderEncoder" {
                        gender_encoder.parse_gender_root(&attributes)?;
                        continue;
                    }

                    if name.local_name.starts_with("genderEncoderpreset") {
                        if let Some(id) = name
                            .local_name
                            .chars()
                            .last()
                            .map(|s| u8::from_str(&s.to_string()))
                            .transpose()?
                        {
                            gender_encoder.parse_gender_preset(id, &attributes)?;
                            continue;
                        }
                    }

                    // These can probably be a little cleaner..
                    if name.local_name == "sampleTopLeft" {
                        let mut sampler = SampleBase::new("sampleTopLeft".to_string());
                        sampler.parse_sample_root(&attributes)?;
                        sampler_map[TopLeft] = Option::Some(sampler);
                        active_sample_button = sampler_map[TopLeft].as_mut();
                        continue;
                    }

                    if name.local_name == "sampleTopRight" {
                        let mut sampler = SampleBase::new("sampleTopRight".to_string());
                        sampler.parse_sample_root(&attributes)?;
                        sampler_map[TopRight] = Option::Some(sampler);
                        active_sample_button = sampler_map[TopRight].as_mut();
                        continue;
                    }

                    if name.local_name == "sampleBottomLeft" {
                        let mut sampler = SampleBase::new("sampleBottomLeft".to_string());
                        sampler.parse_sample_root(&attributes)?;
                        sampler_map[BottomLeft] = Option::Some(sampler);
                        active_sample_button = sampler_map[BottomLeft].as_mut();
                        continue;
                    }

                    if name.local_name == "sampleBottomRight" {
                        let mut sampler = SampleBase::new("sampleBottomRight".to_string());
                        sampler.parse_sample_root(&attributes)?;
                        sampler_map[BottomRight] = Option::Some(sampler);
                        active_sample_button = sampler_map[BottomRight].as_mut();
                        continue;
                    }

                    if name.local_name == "sampleClear" {
                        let mut sampler = SampleBase::new("sampleClear".to_string());
                        sampler.parse_sample_root(&attributes)?;
                        sampler_map[Clear] = Option::Some(sampler);
                        active_sample_button = sampler_map[Clear].as_mut();
                        continue;
                    }

                    if name.local_name.starts_with("sampleStack") {
                        if let Some(id) = name.local_name.chars().last() {
                            if let Some(button) = &mut active_sample_button {
                                button.parse_sample_stack(id, &attributes)?;
                                continue;
                            }
                        }
                    }

                    if name.local_name.starts_with("sampleBank")
                        || name.local_name == "fxClear"
                        || name.local_name == "swear"
                        || name.local_name == "globalColour"
                        || name.local_name == "logoX"
                    {
                        // In this case, the tag name, and attribute prefixes are the same..
                        let mut simple_element = SimpleElement::new(name.local_name.clone());
                        simple_element.parse_simple(&attributes)?;
                        simple_elements[SimpleElements::from_str(&name.local_name)?] = Some(simple_element);

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

        Ok(Self {
            root,
            browser,
            mixer,
            context,
            mute_chat,
            mute_buttons,
            faders,
            effects,
            scribbles,
            sampler_map,
            simple_elements,
            megaphone_effect,
            robot_effect,
            hardtune_effect,
            reverb_encoder,
            echo_encoder,
            pitch_encoder,
            gender_encoder,
        })
    }

    pub fn write<P: AsRef<Path>>(&self, path: P) -> Result<(), xml::writer::Error> {
        let out_file = File::create(path)?;
        return self.write_to(out_file);
    }

    pub fn write_to<W: Write>(&self, mut sink: W) -> Result<(), xml::writer::Error> {
        // Create the file, and the writer..

        let mut writer = EmitterConfig::new()
            .perform_indent(true)
            .write_document_declaration(true)
            .create_writer(&mut sink);

        // Write the initial root tag..
        self.root.write_initial(&mut writer)?;
        self.browser.write_browser(&mut writer)?;

        self.mixer.write_mixers(&mut writer)?;
        self.context.write_context(&mut writer)?;
        self.mute_chat.write_mute_chat(&mut writer)?;

        for mute_button in self.mute_buttons.iter() {
            mute_button.write_button(&mut writer)?;
        }

        for fader in self.faders.iter() {
            fader.write_fader(&mut writer)?;
        }

        for effect in self.effects.iter() {
            effect.write_effects(&mut writer)?;
        }

        for scribble in self.scribbles.iter() {
            scribble.write_scribble(&mut writer)?;
        }

        self.megaphone_effect.write_megaphone(&mut writer)?;
        self.robot_effect.write_robot(&mut writer)?;
        self.hardtune_effect.write_hardtune(&mut writer)?;

        self.reverb_encoder.write_reverb(&mut writer)?;
        self.echo_encoder.write_echo(&mut writer)?;
        self.pitch_encoder.write_pitch(&mut writer)?;
        self.gender_encoder.write_gender(&mut writer)?;

        for (_key, value) in &self.sampler_map {
            if let Some(value) = value {
                value.write_sample(&mut writer)?;
            }
        }

        for simple_element in SimpleElements::iter() {
            self.simple_elements[simple_element].as_ref().unwrap().write_simple(&mut writer)?;
        }

        // Finalise the XML..
        self.root.write_final(&mut writer)?;

        Ok(())
    }

    pub fn mixer_mut(&mut self) -> &mut Mixers {
        &mut self.mixer
    }

    pub fn mixer(&self) -> &Mixers {
        &self.mixer
    }

    pub fn faders(&mut self) -> &mut Vec<Fader> {
        &mut self.faders
    }

    pub fn fader_mut(&mut self, fader: usize) -> &mut Fader {
        &mut self.faders[fader]
    }

    pub fn fader(&self, fader: usize) -> &Fader {
        &self.faders[fader]
    }

    pub fn mute_buttons(&mut self) -> &mut Vec<MuteButton> {
        &mut self.mute_buttons
    }

    pub fn mute_button_mut(&mut self, index: usize) -> &mut MuteButton {
        &mut self.mute_buttons[index]
    }

    pub fn mute_button(&self, index: usize) -> &MuteButton {
        &self.mute_buttons[index]
    }

    pub fn scribbles(&mut self) -> &mut Vec<Scribble> {
        &mut self.scribbles
    }

    pub fn scribble(&self, index: usize) -> &Scribble {
        &self.scribbles[index]
    }


    pub fn effects(&self, effect: usize) -> &Effects {
        &self.effects[effect]
    }

    pub fn mute_chat_mut(&mut self) -> &mut MuteChat {
        &mut self.mute_chat
    }

    pub fn mute_chat(&self) -> &MuteChat {
        &self.mute_chat
    }

    pub fn megaphone_effect(&self) -> &MegaphoneEffectBase {
        &self.megaphone_effect
    }

    pub fn robot_effect(&self) -> &RobotEffectBase {
        &self.robot_effect
    }

    pub fn hardtune_effect(&self) -> &HardtuneEffectBase {
        &self.hardtune_effect
    }

    pub fn sample_button(&self, button: SampleButtons) -> &SampleBase {
        self.sampler_map[button].as_ref().unwrap()
    }

    pub fn pitch_encoder(&self) -> &PitchEncoderBase {
        &self.pitch_encoder
    }

    pub fn echo_encoder(&self) -> &EchoEncoderBase {
        &self.echo_encoder
    }

    pub fn gender_encoder(&self) -> &GenderEncoderBase {
        &self.gender_encoder
    }

    pub fn reverb_encoder(&self) -> &ReverbEncoderBase {
        &self.reverb_encoder
    }

    pub fn simple_element_mut(&mut self, name: SimpleElements) -> &mut SimpleElement {
        if self.simple_elements[name].is_some() {
            return self.simple_elements[name].as_mut().unwrap();
        }

        // If for whatever reason, this is missing, we'll use the global colour.
        return self.simple_elements[SimpleElements::GlobalColour].as_mut().unwrap();
    }

    pub fn simple_element(&self, name: SimpleElements) -> &SimpleElement {
        if self.simple_elements[name].is_some() {
            return self.simple_elements[name].as_ref().unwrap();
        }

        // If for whatever reason, this is missing, we'll use the global colour.
        return self.simple_elements[SimpleElements::GlobalColour].as_ref().unwrap();
    }
}
