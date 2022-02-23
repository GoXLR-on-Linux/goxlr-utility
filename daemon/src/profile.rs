use enumset::EnumSet;
use goxlr_profile_loader::components::mixer::{FullChannelList, InputChannels, OutputChannels};
use goxlr_profile_loader::profile::Profile;
use goxlr_types::{ChannelName, FaderName, FirmwareVersions, InputDevice, OutputDevice, VersionNumber};
use strum::EnumCount;
use strum::IntoEnumIterator;
use goxlr_profile_loader::components::colours::ColourMap;
use goxlr_profile_loader::components::colours::ColourOffStyle::Dimmed;
use goxlr_profile_loader::SampleButtons::{BottomLeft, BottomRight, Clear, TopLeft, TopRight};
use goxlr_usb::colouring::ColourTargets;
use goxlr_usb::rusb::Version;

#[derive(Debug)]
pub struct ProfileAdapter {
    profile: Profile,
}

impl ProfileAdapter {
    pub fn new(profile: Profile) -> Self {
        Self { profile }
    }

    pub fn create_router(&self) -> [EnumSet<OutputDevice>; InputDevice::COUNT] {
        let mut router = [EnumSet::empty(); InputDevice::COUNT];

        for (input, potential_outputs) in self.profile.mixer().mixer_table().iter() {
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
        let fader = self.profile.fader(fader as usize);
        profile_to_standard_channel(fader.channel())
    }

    pub fn get_channel_volume(&self, channel: ChannelName) -> u8 {
        self.profile
            .mixer()
            .channel_volume(standard_to_profile_channel(channel))
    }

    pub fn get_colour_map(&self, use_format_1_3_40: bool) -> [u8;520] {
        let mut colour_array = [0; 520];

        for colour in ColourTargets::iter() {
            let colour_map = get_profile_colour_map(&self.profile, colour);

            for i in 0 .. colour.get_colour_count() {
                let position = colour.position(i, use_format_1_3_40);

                if i == 1 && colour_map.get_off_style() == &Dimmed && colour.is_blank_when_dimmed() {
                    colour_array[position .. position + 4].copy_from_slice(&[00, 00, 00, 00]);
                } else {
                    // Update the correct 4 bytes in the map..
                    colour_array[position..position + 4]
                        .copy_from_slice(&colour_map.colour(i).to_reverse_bytes());
                }
            }
        }
        //dbg!(colour_array);
        return colour_array;
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

fn get_profile_colour_map(profile: &Profile, colour_target: ColourTargets) -> &ColourMap {
    match colour_target {
        ColourTargets::Fader1Mute => &profile.mute_buttons(0).colour_map(),
        ColourTargets::Fader2Mute => &profile.mute_buttons(1).colour_map(),
        ColourTargets::Fader3Mute => &profile.mute_buttons(2).colour_map(),
        ColourTargets::Fader4Mute => &profile.mute_buttons(3).colour_map(),
        ColourTargets::Bleep => &profile.simple_element("swear").unwrap().colour_map(),
        ColourTargets::MicrophoneMute => &profile.mute_chat().colour_map(),
        ColourTargets::EffectSelect1 => &profile.effects(0).colour_map(),
        ColourTargets::EffectSelect2 => &profile.effects(1).colour_map(),
        ColourTargets::EffectSelect3 => &profile.effects(2).colour_map(),
        ColourTargets::EffectSelect4 => &profile.effects(3).colour_map(),
        ColourTargets::EffectSelect5 => &profile.effects(4).colour_map(),
        ColourTargets::EffectSelect6 => &profile.effects(5).colour_map(),
        ColourTargets::EffectFx => &profile.simple_element("fxClear").unwrap().colour_map(),
        ColourTargets::EffectMegaphone => &profile.megaphone_effect().colour_map(),
        ColourTargets::EffectRobot => &profile.robot_effect().colour_map(),
        ColourTargets::EffectHardTune => &profile.hardtune_effect().colour_map(),
        ColourTargets::SamplerSelectA => &profile.simple_element("sampleBankA").unwrap().colour_map(),
        ColourTargets::SamplerSelectB => &profile.simple_element("sampleBankB").unwrap().colour_map(),
        ColourTargets::SamplerSelectC => &profile.simple_element("sampleBankC").unwrap().colour_map(),
        ColourTargets::SamplerTopLeft => &profile.sample_button(TopLeft).colour_map(),
        ColourTargets::SamplerTopRight => &profile.sample_button(TopRight).colour_map(),
        ColourTargets::SamplerBottomLeft => &profile.sample_button(BottomLeft).colour_map(),
        ColourTargets::SamplerBottomRight => &profile.sample_button(BottomRight).colour_map(),
        ColourTargets::SamplerClear => &profile.sample_button(Clear).colour_map(),
        ColourTargets::FadeMeter1 => &profile.fader(0).colour_map(),
        ColourTargets::FadeMeter2 => &profile.fader(1).colour_map(),
        ColourTargets::FadeMeter3 => &profile.fader(2).colour_map(),
        ColourTargets::FadeMeter4 => &profile.fader(3).colour_map(),
        ColourTargets::Scribble1 => &profile.scribbles(0).colour_map(),
        ColourTargets::Scribble2 => &profile.scribbles(1).colour_map(),
        ColourTargets::Scribble3 => &profile.scribbles(2).colour_map(),
        ColourTargets::Scribble4 => &profile.scribbles(3).colour_map(),
        ColourTargets::PitchEncoder => &profile.pitch_encoder().colour_map(),
        ColourTargets::GenderEncoder => &profile.gender_encoder().colour_map(),
        ColourTargets::ReverbEncoder => &profile.reverb_encoder().colour_map(),
        ColourTargets::EchoEncoder => &profile.echo_encoder().colour_map(),
        ColourTargets::LogoX => &profile.simple_element("logoX").unwrap().colour_map(),
        ColourTargets::Global => &profile.simple_element("globalColour").unwrap().colour_map()
    }
}

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
    return false;
}