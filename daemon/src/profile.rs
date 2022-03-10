use anyhow::{anyhow, Context, Result};
use enumset::EnumSet;
use goxlr_profile_loader::components::colours::ColourMap;
use goxlr_profile_loader::components::colours::ColourOffStyle::Dimmed;
use goxlr_profile_loader::components::mixer::{FullChannelList, InputChannels, OutputChannels};
use goxlr_profile_loader::mic_profile::MicProfileSettings;
use goxlr_profile_loader::profile::{Profile, ProfileSettings};
use goxlr_profile_loader::SampleButtons::{BottomLeft, BottomRight, Clear, TopLeft, TopRight};
use goxlr_types::{
    ChannelName, FaderName, InputDevice, MicrophoneType, OutputDevice, VersionNumber,
};
use goxlr_usb::colouring::ColourTargets;
use log::error;
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use strum::EnumCount;
use strum::IntoEnumIterator;

pub const DEFAULT_PROFILE_NAME: &str = "Default - Vaporwave";
const DEFAULT_PROFILE: &[u8] = include_bytes!("../profiles/Default - Vaporwave.goxlr");

pub const DEFAULT_MIC_PROFILE_NAME: &str = "DEFAULT";
const DEFAULT_MIC_PROFILE: &[u8] = include_bytes!("../profiles/DEFAULT.goxlrMicProfile");

#[derive(Debug)]
pub struct ProfileAdapter {
    name: String,
    profile: Profile,
}

impl ProfileAdapter {
    pub fn from_named_or_default(name: Option<String>, directory: &Path) -> Self {
        if let Some(name) = name {
            match ProfileAdapter::from_named(name.clone(), directory) {
                Ok(result) => return result,
                Err(error) => error!("Couldn't load profile {}: {}", name, error),
            }
        }

        ProfileAdapter::default()
    }

    pub fn from_named(name: String, directory: &Path) -> Result<Self> {
        let path = directory.join(format!("{}.goxlr", name));
        if path.is_file() {
            let file = File::open(path).context("Couldn't open profile for reading")?;
            return ProfileAdapter::from_reader(name, file).context("Couldn't read profile");
        }

        if name == DEFAULT_PROFILE_NAME {
            return Ok(ProfileAdapter::default());
        }

        Err(anyhow!(
            "Profile {} does not exist inside {}",
            name,
            directory.to_string_lossy()
        ))
    }

    pub fn default() -> Self {
        ProfileAdapter::from_reader(
            DEFAULT_PROFILE_NAME.to_string(),
            Cursor::new(DEFAULT_PROFILE),
        )
        .expect("Default profile isn't available")
    }

    pub fn from_reader<R: Read + Seek>(name: String, reader: R) -> Result<Self> {
        let profile = Profile::load(reader)?;
        Ok(Self { name, profile })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn create_router(&self) -> [EnumSet<OutputDevice>; InputDevice::COUNT] {
        let mut router = [EnumSet::empty(); InputDevice::COUNT];

        for (input, potential_outputs) in self.profile.settings().mixer().mixer_table().iter() {
            let mut outputs = EnumSet::empty();

            for (channel, volume) in potential_outputs.iter() {
                if *volume > 0 {
                    outputs.insert(profile_to_standard_output(channel));
                }
            }

            router[profile_to_standard_input(input) as usize] = outputs;
        }
        router
    }

    pub fn get_fader_assignment(&self, fader: FaderName) -> ChannelName {
        let fader = self.profile.settings().fader(fader as usize);
        profile_to_standard_channel(fader.channel())
    }

    pub fn get_channel_volume(&self, channel: ChannelName) -> u8 {
        self.profile
            .settings()
            .mixer()
            .channel_volume(standard_to_profile_channel(channel))
    }

    pub fn get_colour_map(&self, use_format_1_3_40: bool) -> [u8; 520] {
        let mut colour_array = [0; 520];

        for colour in ColourTargets::iter() {
            let colour_map = get_profile_colour_map(self.profile.settings(), colour);

            for i in 0..colour.get_colour_count() {
                let position = colour.position(i, use_format_1_3_40);

                if i == 1 && colour_map.get_off_style() == &Dimmed && colour.is_blank_when_dimmed()
                {
                    colour_array[position..position + 4].copy_from_slice(&[00, 00, 00, 00]);
                } else {
                    // Update the correct 4 bytes in the map..
                    colour_array[position..position + 4]
                        .copy_from_slice(&colour_map.colour(i).to_reverse_bytes());
                }
            }
        }

        colour_array
    }
}

#[derive(Debug)]
pub struct MicProfileAdapter {
    name: String,
    profile: MicProfileSettings,
}

impl MicProfileAdapter {
    pub fn from_named_or_default(name: Option<String>, directory: &Path) -> Self {
        if let Some(name) = name {
            match MicProfileAdapter::from_named(name.clone(), directory) {
                Ok(result) => return result,
                Err(error) => error!("Couldn't load mic profile {}: {}", name, error),
            }
        }

        MicProfileAdapter::default()
    }

    pub fn from_named(name: String, directory: &Path) -> Result<Self> {
        let path = directory.join(format!("{}.goxlrMicProfile", name));
        if path.is_file() {
            let file = File::open(path).context("Couldn't open mic profile for reading")?;
            return MicProfileAdapter::from_reader(name, file).context("Couldn't read mic profile");
        }

        if name == DEFAULT_MIC_PROFILE_NAME {
            return Ok(MicProfileAdapter::default());
        }

        Err(anyhow!(
            "Mic profile {} does not exist inside {}",
            name,
            directory.to_string_lossy()
        ))
    }

    pub fn default() -> Self {
        MicProfileAdapter::from_reader(
            DEFAULT_MIC_PROFILE_NAME.to_string(),
            Cursor::new(DEFAULT_MIC_PROFILE),
        )
        .expect("Default mic profile isn't available")
    }

    pub fn from_reader<R: Read + Seek>(name: String, reader: R) -> Result<Self> {
        let profile = MicProfileSettings::load(reader)?;
        Ok(Self { name, profile })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn mic_gains(&self) -> [u16; 3] {
        [
            self.profile.setup().dynamic_mic_gain() as u16,
            self.profile.setup().condenser_mic_gain() as u16,
            self.profile.setup().trs_mic_gain() as u16,
        ]
    }

    pub fn mic_type(&self) -> MicrophoneType {
        match self.profile.setup().mic_type() {
            0 => MicrophoneType::Dynamic,
            1 => MicrophoneType::Condenser,
            2 => MicrophoneType::Jack,
            _ => MicrophoneType::Jack, // default
        }
    }
}

fn profile_to_standard_input(value: InputChannels) -> InputDevice {
    match value {
        InputChannels::Mic => InputDevice::Microphone,
        InputChannels::Chat => InputDevice::Chat,
        InputChannels::Music => InputDevice::Music,
        InputChannels::Game => InputDevice::Game,
        InputChannels::Console => InputDevice::Console,
        InputChannels::LineIn => InputDevice::LineIn,
        InputChannels::System => InputDevice::System,
        InputChannels::Sample => InputDevice::Samples,
    }
}

fn profile_to_standard_output(value: OutputChannels) -> OutputDevice {
    match value {
        OutputChannels::Headphones => OutputDevice::Headphones,
        OutputChannels::Broadcast => OutputDevice::BroadcastMix,
        OutputChannels::LineOut => OutputDevice::LineOut,
        OutputChannels::ChatMic => OutputDevice::ChatMic,
        OutputChannels::Sampler => OutputDevice::Sampler,
    }
}

fn profile_to_standard_channel(value: FullChannelList) -> ChannelName {
    match value {
        FullChannelList::Mic => ChannelName::Mic,
        FullChannelList::Chat => ChannelName::Chat,
        FullChannelList::Music => ChannelName::Music,
        FullChannelList::Game => ChannelName::Game,
        FullChannelList::Console => ChannelName::Console,
        FullChannelList::LineIn => ChannelName::LineIn,
        FullChannelList::System => ChannelName::System,
        FullChannelList::Sample => ChannelName::Sample,
        FullChannelList::Headphones => ChannelName::Headphones,
        FullChannelList::MicMonitor => ChannelName::MicMonitor,
        FullChannelList::LineOut => ChannelName::LineOut,
    }
}

fn standard_to_profile_channel(value: ChannelName) -> FullChannelList {
    match value {
        ChannelName::Mic => FullChannelList::Mic,
        ChannelName::Chat => FullChannelList::Chat,
        ChannelName::Music => FullChannelList::Music,
        ChannelName::Game => FullChannelList::Game,
        ChannelName::Console => FullChannelList::Console,
        ChannelName::LineIn => FullChannelList::LineIn,
        ChannelName::System => FullChannelList::System,
        ChannelName::Sample => FullChannelList::Sample,
        ChannelName::Headphones => FullChannelList::Headphones,
        ChannelName::MicMonitor => FullChannelList::MicMonitor,
        ChannelName::LineOut => FullChannelList::LineOut,
    }
}

fn get_profile_colour_map(profile: &ProfileSettings, colour_target: ColourTargets) -> &ColourMap {
    match colour_target {
        ColourTargets::Fader1Mute => profile.mute_buttons(0).colour_map(),
        ColourTargets::Fader2Mute => profile.mute_buttons(1).colour_map(),
        ColourTargets::Fader3Mute => profile.mute_buttons(2).colour_map(),
        ColourTargets::Fader4Mute => profile.mute_buttons(3).colour_map(),
        ColourTargets::Bleep => profile.simple_element("swear").unwrap().colour_map(),
        ColourTargets::MicrophoneMute => profile.mute_chat().colour_map(),
        ColourTargets::EffectSelect1 => profile.effects(0).colour_map(),
        ColourTargets::EffectSelect2 => profile.effects(1).colour_map(),
        ColourTargets::EffectSelect3 => profile.effects(2).colour_map(),
        ColourTargets::EffectSelect4 => profile.effects(3).colour_map(),
        ColourTargets::EffectSelect5 => profile.effects(4).colour_map(),
        ColourTargets::EffectSelect6 => profile.effects(5).colour_map(),
        ColourTargets::EffectFx => profile.simple_element("fxClear").unwrap().colour_map(),
        ColourTargets::EffectMegaphone => profile.megaphone_effect().colour_map(),
        ColourTargets::EffectRobot => profile.robot_effect().colour_map(),
        ColourTargets::EffectHardTune => profile.hardtune_effect().colour_map(),
        ColourTargets::SamplerSelectA => {
            profile.simple_element("sampleBankA").unwrap().colour_map()
        }
        ColourTargets::SamplerSelectB => {
            profile.simple_element("sampleBankB").unwrap().colour_map()
        }
        ColourTargets::SamplerSelectC => {
            profile.simple_element("sampleBankC").unwrap().colour_map()
        }
        ColourTargets::SamplerTopLeft => profile.sample_button(TopLeft).colour_map(),
        ColourTargets::SamplerTopRight => profile.sample_button(TopRight).colour_map(),
        ColourTargets::SamplerBottomLeft => profile.sample_button(BottomLeft).colour_map(),
        ColourTargets::SamplerBottomRight => profile.sample_button(BottomRight).colour_map(),
        ColourTargets::SamplerClear => profile.sample_button(Clear).colour_map(),
        ColourTargets::FadeMeter1 => profile.fader(0).colour_map(),
        ColourTargets::FadeMeter2 => profile.fader(1).colour_map(),
        ColourTargets::FadeMeter3 => profile.fader(2).colour_map(),
        ColourTargets::FadeMeter4 => profile.fader(3).colour_map(),
        ColourTargets::Scribble1 => profile.scribbles(0).colour_map(),
        ColourTargets::Scribble2 => profile.scribbles(1).colour_map(),
        ColourTargets::Scribble3 => profile.scribbles(2).colour_map(),
        ColourTargets::Scribble4 => profile.scribbles(3).colour_map(),
        ColourTargets::PitchEncoder => profile.pitch_encoder().colour_map(),
        ColourTargets::GenderEncoder => profile.gender_encoder().colour_map(),
        ColourTargets::ReverbEncoder => profile.reverb_encoder().colour_map(),
        ColourTargets::EchoEncoder => profile.echo_encoder().colour_map(),
        ColourTargets::LogoX => profile.simple_element("logoX").unwrap().colour_map(),
        ColourTargets::Global => profile.simple_element("globalColour").unwrap().colour_map(),
    }
}

#[allow(clippy::comparison_chain)]
pub fn version_newer_or_equal_to(version: &VersionNumber, comparison: VersionNumber) -> bool {
    if version.0 > comparison.0 {
        return true;
    } else if version.0 < comparison.0 {
        return false;
    }

    if version.1 > comparison.1 {
        return true;
    } else if version.1 < comparison.1 {
        return false;
    }

    if version.2 > comparison.2 {
        return true;
    } else if version.2 < comparison.2 {
        return false;
    }

    if version.3 >= comparison.3 {
        return true;
    }

    false
}
