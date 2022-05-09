use anyhow::{anyhow, Context, Result};
use enumset::EnumSet;
use goxlr_profile_loader::components::colours::{ColourMap, ColourOffStyle, ColourState};
use goxlr_profile_loader::components::colours::ColourOffStyle::Dimmed;
use goxlr_profile_loader::components::mixer::{FullChannelList, InputChannels, OutputChannels};
use goxlr_profile_loader::mic_profile::MicProfileSettings;
use goxlr_profile_loader::profile::{Profile, ProfileSettings};
use goxlr_profile_loader::SampleButtons::{BottomLeft, BottomRight, Clear, TopLeft, TopRight};
use goxlr_types::{ChannelName, FaderName, InputDevice, MicrophoneType, OutputDevice, VersionNumber, MuteFunction as BasicMuteFunction };
use goxlr_usb::colouring::ColourTargets;
use log::error;
use std::fs::File;
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use strum::EnumCount;
use strum::IntoEnumIterator;
use byteorder::{ByteOrder, LittleEndian};
use enum_map::EnumMap;
use goxlr_profile_loader::components::mute::{MuteButton, MuteFunction};
use goxlr_profile_loader::components::simple::SimpleElements;
use goxlr_usb::buttonstate::{Buttons, ButtonStates};

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

    pub fn to_named(&self, name: String, directory: &Path) -> Result<()> {
        let path = directory.join(format!("{}.goxlr", name));
        if path.is_file() {
            self.profile.save(path)?;
        }
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn create_router(&mut self) -> [EnumSet<OutputDevice>; InputDevice::COUNT] {
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

    pub fn get_router(&mut self, output: InputDevice) -> EnumMap<OutputDevice, bool> {
        let mut map: EnumMap<OutputDevice, bool> = EnumMap::default();

        // Get the mixer table
        let mixer = &self.profile.settings().mixer().mixer_table()[standard_input_to_profile(output)];
        for (channel, volume) in mixer.iter() {
            map[profile_to_standard_output(channel)] = *volume > 0;
        }

        return map;
    }

    pub fn get_fader_assignment(&mut self, fader: FaderName) -> ChannelName {
        let fader = self.profile.settings().fader(fader as usize);
        profile_to_standard_channel(fader.channel())
    }

    pub fn set_fader_assignment(&mut self, fader: FaderName, channel: ChannelName) {
        self.profile
            .settings()
            .fader(fader as usize)
            .set_channel(standard_to_profile_channel(channel));
    }

    pub fn switch_fader_assignment(&mut self, fader_one: FaderName, fader_two: FaderName) {
        // TODO: Scribble?
        self.profile.settings().faders().swap(fader_one as usize, fader_two as usize);
        self.profile.settings().mute_buttons().swap(fader_one as usize, fader_two as usize);
    }

    pub fn get_channel_volume(&mut self, channel: ChannelName) -> u8 {
        self.profile
            .settings()
            .mixer()
            .channel_volume(standard_to_profile_channel(channel))
    }

    pub fn set_channel_volume(&mut self, channel: ChannelName, volume: u8) {
        self.profile
            .settings()
            .mixer()
            .set_channel_volume(standard_to_profile_channel(channel), volume);
    }

    pub fn get_colour_map(&mut self, use_format_1_3_40: bool) -> [u8; 520] {
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

    fn get_button_colour_map(&mut self, button: Buttons) -> &ColourMap {
        get_colour_map_from_button(self.profile.settings(), button)
    }

    /** Regular Mute button handlers */
    fn get_mute_button(&mut self, fader: FaderName) -> &mut MuteButton {
        self.profile.settings().mute_button(fader as usize)
    }

    pub fn get_mute_button_behaviour(&mut self, fader: FaderName) -> BasicMuteFunction {
        let mute_config = self.get_mute_button(fader);

        return match mute_config.mute_function() {
            MuteFunction::All => BasicMuteFunction::All,
            MuteFunction::ToStream => BasicMuteFunction::ToStream,
            MuteFunction::ToVoiceChat => BasicMuteFunction::ToVoiceChat,
            MuteFunction::ToPhones => BasicMuteFunction::ToPhones,
            MuteFunction::ToLineOut => BasicMuteFunction::ToLineOut
        };
    }

    pub fn get_mute_button_state(&mut self, fader: FaderName) -> (bool, bool, MuteFunction) {
        let mute_config = self.get_mute_button(fader);
        let colour_map = mute_config.colour_map();

        // We should be safe to straight unwrap these, state and blink are always present.
        let muted_to_x = colour_map.state().as_ref().unwrap() == &ColourState::On;
        let muted_to_all = colour_map.blink().as_ref().unwrap() == &ColourState::On;
        let mute_function = mute_config.mute_function().clone();

        return (muted_to_x, muted_to_all, mute_function);
    }

    pub fn get_mute_button_previous_volume(&mut self, fader: FaderName) -> u8 {
        self.get_mute_button(fader).previous_volume()
    }

    pub fn set_mute_button_previous_volume(&mut self, fader: FaderName, volume: u8) {
        self.get_mute_button(fader).set_previous_volume(volume);
    }

    pub fn set_mute_button_on(&mut self, fader: FaderName, on: bool) {
        self.get_mute_button(fader).colour_map().set_state_on(on);
    }

    pub fn set_mute_button_blink(&mut self, fader: FaderName, on: bool) {
        self.get_mute_button(fader).colour_map().set_blink_on(on);
    }


    /** 'Cough' / Mute Chat Button handlers.. */
    pub fn get_mute_chat_button_state(&mut self) -> (bool, bool, bool, MuteFunction) {
        let mute_config = self.profile.settings().mute_chat();

        // Identical behaviour, different variable locations..
        let mute_toggle = mute_config.is_cough_toggle();
        let muted_to_x = mute_config.cough_button_on();
        let muted_to_all = mute_config.blink() == &ColourState::On;
        let mute_function = mute_config.cough_mute_source().clone();

        return (mute_toggle, muted_to_x, muted_to_all, mute_function);
    }

    pub fn set_mute_chat_button_on(&mut self, on: bool) {
        self.profile.settings().mute_chat().set_cough_button_on(on);
    }

    pub fn set_mute_chat_button_blink(&mut self, on: bool) {
        self.profile.settings().mute_chat().set_blink_on(on);
    }

    pub fn get_mute_chat_button_blink(&mut self) -> bool {
        self.profile.settings().mute_chat().get_blink_on()
    }

    pub fn get_mute_chat_button_on(&mut self) -> bool {
        self.profile.settings().mute_chat().get_cough_button_on()
    }

    pub fn get_mute_chat_button_colour_state(&mut self) -> ButtonStates {
        if self.get_mute_chat_button_blink() {
            return ButtonStates::Flashing;
        }

        if self.get_mute_chat_button_on() {
            return ButtonStates::Colour1;
        }

        return match self.profile.settings().mute_chat().colour_map().get_off_style() {
            ColourOffStyle::Dimmed => ButtonStates::DimmedColour1,
            ColourOffStyle::Colour2 => ButtonStates::Colour2,
            ColourOffStyle::DimmedColour2 => ButtonStates::DimmedColour2
        }
    }

    /** Fader Stuff */
    pub fn get_mic_fader_id(&mut self) -> u8 {
        self.profile.settings().mute_chat().mic_fader_id()
    }

    pub fn set_mic_fader_id(&mut self, id: u8) {
        self.profile.settings().mute_chat().set_mic_fader_id(id);
    }

    pub fn fader_from_id(&self, fader: u8) -> FaderName {
        return match fader {
            0 => FaderName::A,
            1 => FaderName::B,
            2 => FaderName::C,
            _ => FaderName::D
        }
    }

    pub fn is_fader_gradient(&mut self, fader: FaderName) -> bool {
        self.profile.settings().fader(fader as usize).colour_map().is_fader_gradient()
    }

    pub fn is_fader_meter(&mut self, fader: FaderName) -> bool {
        self.profile.settings().fader(fader as usize).colour_map().is_fader_meter()
    }

    /** Bleep Button **/
    pub fn set_swear_button_on(&mut self, on: bool) {
        // Get the colour map for the bleep button..
        self.profile.
            settings()
            .simple_element(SimpleElements::Swear)
            .colour_map()
            .set_state_on(on);
    }

    /** Generic Stuff **/
    pub fn get_button_colour_state(&mut self, button: Buttons) -> ButtonStates {
        let colour_map = self.get_button_colour_map(button);

        if let Some(blink) = colour_map.blink() {
            if blink == &ColourState::On {
                return ButtonStates::Flashing;
            }
        }

        if let Some(state) = colour_map.state() {
            if state == &ColourState::On {
                return ButtonStates::Colour1;
            }
        }

        // Button is turned off, so go return the 'Off Style'
        return match colour_map.get_off_style() {
            ColourOffStyle::Dimmed => ButtonStates::DimmedColour1,
            ColourOffStyle::Colour2 => ButtonStates::Colour2,
            ColourOffStyle::DimmedColour2 => ButtonStates::DimmedColour2
        }
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

    pub fn mic_gains(&mut self) -> [u16; 3] {
        [
            self.profile.setup().dynamic_mic_gain() as u16,
            self.profile.setup().condenser_mic_gain() as u16,
            self.profile.setup().trs_mic_gain() as u16,
        ]
    }

    pub fn mic_type(&mut self) -> MicrophoneType {
        match self.profile.setup().mic_type() {
            0 => MicrophoneType::Dynamic,
            1 => MicrophoneType::Condenser,
            2 => MicrophoneType::Jack,
            _ => MicrophoneType::Jack, // default
        }
    }

    pub fn set_mic_type(&mut self, mic_type: MicrophoneType) {
        self.profile.setup().set_mic_type(mic_type as u8);
    }

    pub fn set_mic_gain(&mut self, mic_type: MicrophoneType, gain: u16) {
        match mic_type {
            MicrophoneType::Dynamic => self.profile.setup().set_dynamic_mic_gain(gain),
            MicrophoneType::Condenser => self.profile.setup().set_condenser_mic_gain(gain),
            MicrophoneType::Jack => self.profile.setup().set_trs_mic_gain(gain)
        }
    }

    pub fn mic_params(&self) -> [[u8; 4]; 9] {
        let mut gate_threshold = [0; 4];
        let mut gate_attack = [0; 4];
        let mut gate_release = [0; 4];
        let mut gate_attenuation = [0; 4];

        LittleEndian::write_f32(&mut gate_threshold, self.profile.gate().threshold().into());
        LittleEndian::write_f32(&mut gate_attack, self.profile.gate().attack().into());
        LittleEndian::write_f32(&mut gate_release, self.profile.gate().release().into());
        LittleEndian::write_f32(&mut gate_attenuation, self.profile.gate().attenuation().into());

        let mut comp_threshold = [0; 4];
        let mut comp_ratio = [0; 4];
        let mut comp_attack = [0; 4];
        let mut comp_release = [0; 4];
        let mut comp_makeup = [0; 4];

        LittleEndian::write_f32(&mut comp_threshold, self.profile.compressor().threshold().into());
        LittleEndian::write_f32(&mut comp_ratio, self.profile.compressor().ratio().into());
        LittleEndian::write_f32(&mut comp_attack, self.profile.compressor().attack().into());
        LittleEndian::write_f32(&mut comp_release, self.profile.compressor().release().into());
        LittleEndian::write_f32(&mut comp_makeup, self.profile.compressor().makeup().into());

        [
            gate_threshold,
            gate_attack,
            gate_release,
            gate_attenuation,
            comp_threshold,
            comp_ratio,
            comp_attack,
            comp_release,
            comp_makeup,
        ]
    }

    pub fn mic_effects(&self) -> [i32; 9] {
        [
            self.profile.gate().threshold().into(),
            self.profile.gate().attack().into(),
            self.profile.gate().release().into(),
            self.profile.gate().attenuation().into(),
            self.profile.compressor().threshold().into(),
            self.profile.compressor().ratio().into(),
            self.profile.compressor().attack().into(),
            self.profile.compressor().release().into(),
            self.profile.compressor().makeup().into(),
        ]
    }

    pub fn get_eq_gain(&self) -> [i32; 10] {
        [
            self.profile.equalizer().eq_31h_gain().into(),
            self.profile.equalizer().eq_63h_gain().into(),
            self.profile.equalizer().eq_125h_gain().into(),
            self.profile.equalizer().eq_250h_gain().into(),
            self.profile.equalizer().eq_500h_gain().into(),
            self.profile.equalizer().eq_1k_gain().into(),
            self.profile.equalizer().eq_2k_gain().into(),
            self.profile.equalizer().eq_4k_gain().into(),
            self.profile.equalizer().eq_8k_gain().into(),
            self.profile.equalizer().eq_16k_gain().into(),
        ]
    }

    pub fn get_eq_freq(&self) -> [i32; 10] {
        // Some kind of mapping needs to occur here, so returning a default..
        [
            15,
            40,
            63,
            87,
            111,
            135,
            159,
            183,
            207,
            231
        ]
    }

    pub fn get_eq_gain_mini(&self) -> [[u8; 4]; 6] {

        let mut eq_90_gain = [0; 4];
        let mut eq_250_gain = [0; 4];
        let mut eq_500_gain = [0; 4];
        let mut eq_1k_gain = [0; 4];
        let mut eq_3k_gain = [0; 4];
        let mut eq_8k_gain = [0; 4];



        LittleEndian::write_f32(&mut eq_90_gain, self.profile.equalizer_mini().eq_90h_gain().into());
        LittleEndian::write_f32(&mut eq_250_gain, self.profile.equalizer_mini().eq_250h_gain().into());
        LittleEndian::write_f32(&mut eq_500_gain, self.profile.equalizer_mini().eq_500h_gain().into());
        LittleEndian::write_f32(&mut eq_1k_gain, self.profile.equalizer_mini().eq_1k_gain().into());
        LittleEndian::write_f32(&mut eq_3k_gain, self.profile.equalizer_mini().eq_3k_gain().into());
        LittleEndian::write_f32(&mut eq_8k_gain, self.profile.equalizer_mini().eq_8k_gain().into());

        [
            eq_90_gain,
            eq_250_gain,
            eq_500_gain,
            eq_1k_gain,
            eq_3k_gain,
            eq_8k_gain
        ]
    }

    pub fn get_eq_freq_mini(&self) -> [[u8; 4]; 6] {
        let mut eq_90_freq = [0; 4];
        let mut eq_250_freq = [0; 4];
        let mut eq_500_freq = [0; 4];
        let mut eq_1k_freq = [0; 4];
        let mut eq_3k_freq = [0; 4];
        let mut eq_8k_freq = [0; 4];

        LittleEndian::write_f32(&mut eq_90_freq, self.profile.equalizer_mini().eq_90h_freq().into());
        LittleEndian::write_f32(&mut eq_250_freq, self.profile.equalizer_mini().eq_250h_freq().into());
        LittleEndian::write_f32(&mut eq_500_freq, self.profile.equalizer_mini().eq_500h_freq().into());
        LittleEndian::write_f32(&mut eq_1k_freq, self.profile.equalizer_mini().eq_1k_freq().into());
        LittleEndian::write_f32(&mut eq_3k_freq, self.profile.equalizer_mini().eq_3k_freq().into());
        LittleEndian::write_f32(&mut eq_8k_freq, self.profile.equalizer_mini().eq_8k_freq().into());

        [
            eq_90_freq,
            eq_250_freq,
            eq_500_freq,
            eq_1k_freq,
            eq_3k_freq,
            eq_8k_freq
        ]
    }

    pub fn get_deesser(&self) -> i32 {
        self.profile.deess() as i32
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

fn standard_input_to_profile(value: InputDevice) -> InputChannels {
    match value {
        InputDevice::Microphone => InputChannels::Mic,
        InputDevice::Chat => InputChannels::Chat,
        InputDevice::Music => InputChannels::Music,
        InputDevice::Game => InputChannels::Game,
        InputDevice::Console => InputChannels::Console,
        InputDevice::LineIn => InputChannels::LineIn,
        InputDevice::System => InputChannels::System,
        InputDevice::Samples => InputChannels::Sample,
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

// Commented to prevent warning, will probably be needed later!
// fn standard_output_to_profile(value: OutputDevice) -> OutputChannels {
//     match value {
//         OutputDevice::Headphones => OutputChannels::Headphones,
//         OutputDevice::BroadcastMix => OutputChannels::Broadcast,
//         OutputDevice::LineOut => OutputChannels::LineOut,
//         OutputDevice::ChatMic => OutputChannels::ChatMic,
//         OutputDevice::Sampler => OutputChannels::Sampler,
//     }
// }




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

fn get_colour_map_from_button(profile: &mut ProfileSettings, button: Buttons) -> &ColourMap {
    get_profile_colour_map(profile, map_button_to_colour_target(button))
}

fn map_button_to_colour_target(button: Buttons) -> ColourTargets {
    match button {
        Buttons::Fader1Mute => ColourTargets::Fader1Mute,
        Buttons::Fader2Mute => ColourTargets::Fader2Mute,
        Buttons::Fader3Mute => ColourTargets::Fader3Mute,
        Buttons::Fader4Mute => ColourTargets::Fader4Mute,
        Buttons::Bleep => ColourTargets::Bleep,
        Buttons::MicrophoneMute => ColourTargets::MicrophoneMute,
        Buttons::EffectSelect1 => ColourTargets::EffectSelect1,
        Buttons::EffectSelect2 => ColourTargets::EffectSelect2,
        Buttons::EffectSelect3 => ColourTargets::EffectSelect3,
        Buttons::EffectSelect4 => ColourTargets::EffectSelect4,
        Buttons::EffectSelect5 => ColourTargets::EffectSelect5,
        Buttons::EffectSelect6 => ColourTargets::EffectSelect6,
        Buttons::EffectFx => ColourTargets::EffectFx,
        Buttons::EffectMegaphone => ColourTargets::EffectMegaphone,
        Buttons::EffectRobot => ColourTargets::EffectRobot,
        Buttons::EffectHardTune => ColourTargets::EffectHardTune,
        Buttons::SamplerSelectA => ColourTargets::SamplerSelectA,
        Buttons::SamplerSelectB => ColourTargets::SamplerSelectB,
        Buttons::SamplerSelectC => ColourTargets::SamplerSelectC,
        Buttons::SamplerTopLeft => ColourTargets::SamplerTopLeft,
        Buttons::SamplerTopRight => ColourTargets::SamplerTopRight,
        Buttons::SamplerBottomLeft => ColourTargets::SamplerBottomLeft,
        Buttons::SamplerBottomRight => ColourTargets::SamplerBottomRight,
        Buttons::SamplerClear => ColourTargets::SamplerClear,
    }
}

fn get_profile_colour_map(profile: &mut ProfileSettings, colour_target: ColourTargets) -> &ColourMap {
    match colour_target {
        ColourTargets::Fader1Mute => profile.mute_button(0).colour_map(),
        ColourTargets::Fader2Mute => profile.mute_button(1).colour_map(),
        ColourTargets::Fader3Mute => profile.mute_button(2).colour_map(),
        ColourTargets::Fader4Mute => profile.mute_button(3).colour_map(),
        ColourTargets::Bleep => profile.simple_element(SimpleElements::Swear).colour_map(),
        ColourTargets::MicrophoneMute => profile.mute_chat().colour_map(),
        ColourTargets::EffectSelect1 => profile.effects(0).colour_map(),
        ColourTargets::EffectSelect2 => profile.effects(1).colour_map(),
        ColourTargets::EffectSelect3 => profile.effects(2).colour_map(),
        ColourTargets::EffectSelect4 => profile.effects(3).colour_map(),
        ColourTargets::EffectSelect5 => profile.effects(4).colour_map(),
        ColourTargets::EffectSelect6 => profile.effects(5).colour_map(),
        ColourTargets::EffectFx => profile.simple_element(SimpleElements::FxClear).colour_map(),
        ColourTargets::EffectMegaphone => profile.megaphone_effect().colour_map(),
        ColourTargets::EffectRobot => profile.robot_effect().colour_map(),
        ColourTargets::EffectHardTune => profile.hardtune_effect().colour_map(),
        ColourTargets::SamplerSelectA => {
            profile.simple_element(SimpleElements::SampleBankA).colour_map()
        }
        ColourTargets::SamplerSelectB => {
            profile.simple_element(SimpleElements::SampleBankB).colour_map()
        }
        ColourTargets::SamplerSelectC => {
            profile.simple_element(SimpleElements::SampleBankC).colour_map()
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
        ColourTargets::Scribble1 => profile.scribble(0).colour_map(),
        ColourTargets::Scribble2 => profile.scribble(1).colour_map(),
        ColourTargets::Scribble3 => profile.scribble(2).colour_map(),
        ColourTargets::Scribble4 => profile.scribble(3).colour_map(),
        ColourTargets::PitchEncoder => profile.pitch_encoder().colour_map(),
        ColourTargets::GenderEncoder => profile.gender_encoder().colour_map(),
        ColourTargets::ReverbEncoder => profile.reverb_encoder().colour_map(),
        ColourTargets::EchoEncoder => profile.echo_encoder().colour_map(),
        ColourTargets::LogoX => profile.simple_element(SimpleElements::LogoX).colour_map(),
        ColourTargets::Global => profile.simple_element(SimpleElements::GlobalColour).colour_map(),
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
