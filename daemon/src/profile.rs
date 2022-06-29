use std::collections::HashSet;
use anyhow::{anyhow, Context, Result};
use enumset::EnumSet;
use goxlr_profile_loader::components::colours::{Colour, ColourDisplay, ColourMap, ColourOffStyle, ColourState};
use goxlr_profile_loader::components::mixer::{FullChannelList, InputChannels, OutputChannels};
use goxlr_profile_loader::mic_profile::MicProfileSettings;
use goxlr_profile_loader::profile::{Profile, ProfileSettings};
use goxlr_profile_loader::SampleButtons::{BottomLeft, BottomRight, Clear, TopLeft, TopRight};
use goxlr_types::{ChannelName, FaderName, InputDevice, MicrophoneType, OutputDevice, VersionNumber, MuteFunction as BasicMuteFunction, FaderDisplayStyle as BasicColourDisplay, ButtonColourOffStyle as BasicColourOffStyle, EffectBankPresets, MicrophoneParamKey, EffectKey, EqFrequencies, MiniEqFrequencies, CompressorRatio, CompressorAttackTime, GateTimes, CompressorReleaseTime, ButtonColourTargets, ButtonColourGroups};
use goxlr_usb::colouring::ColourTargets;
use log::error;
use std::fs::{create_dir_all, File};
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use strum::EnumCount;
use strum::IntoEnumIterator;
use byteorder::{ByteOrder, LittleEndian};
use enum_map::EnumMap;
use futures::executor::block_on;
use goxlr_ipc::{Compressor, Equaliser, EqualiserFrequency, EqualiserGain, EqualiserMini, EqualiserMiniFrequency, EqualiserMiniGain, NoiseGate};
use goxlr_profile_loader::components::echo::EchoEncoder;
use goxlr_profile_loader::components::gender::GenderEncoder;
use goxlr_profile_loader::components::hardtune::{HardtuneEffect, HardtuneSource};
use goxlr_profile_loader::components::megaphone::{MegaphoneEffect, Preset};
use goxlr_profile_loader::components::mute::{MuteButton, MuteFunction};
use goxlr_profile_loader::components::mute_chat::MuteChat;
use goxlr_profile_loader::components::pitch::{PitchEncoder, PitchStyle};
use goxlr_profile_loader::components::reverb::ReverbEncoder;
use goxlr_profile_loader::components::robot::RobotEffect;
use goxlr_profile_loader::components::sample::SampleBank;
use goxlr_profile_loader::components::simple::SimpleElements;
use goxlr_profile_loader::SampleButtons;
use goxlr_usb::buttonstate::{Buttons, ButtonStates};
use crate::SettingsHandle;

pub const DEFAULT_PROFILE_NAME: &str = "Default - Vaporwave";
const DEFAULT_PROFILE: &[u8] = include_bytes!("../profiles/Default - Vaporwave.goxlr");

pub const DEFAULT_MIC_PROFILE_NAME: &str = "DEFAULT";
const DEFAULT_MIC_PROFILE: &[u8] = include_bytes!("../profiles/DEFAULT.goxlrMicProfile");

static GATE_ATTENUATION: [i8; 26] = [
    -6,  -7,  -8,  -9,  -10, -11, -12, -13, -14, -15, -16, -17, -18,
    -19, -20, -21, -22, -23, -24, -25, -26, -27, -28, -30, -32, -61
];

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
        let mut path = directory.join(format!("{}.goxlr", name));

        if !path.is_file() {
            path = directory.join(format!("{}.goxlrProfile", name));
        }

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

    pub fn write_profile(&mut self, name: String, directory: &Path, overwrite: bool) -> Result<()> {
        let path = directory.join(format!("{}.goxlr", name));
        if !directory.exists() {
            // Attempt to create the profile directory..
            if let Err(e) = create_dir_all(directory) {
                return Err(e).context(format!(
                    "Could not create profile directory at {}",
                    directory.to_string_lossy()
                ))?;
            }
        }

        if !overwrite && path.is_file() {
            return Err(anyhow!("Profile exists, will not overwrite"));
        }

        self.profile.save(path)?;

        // Keep our names in sync (in case it was changed)
        if name != self.name() {
            dbg!("Changing Profile Name: {} -> {}", self.name(), name.clone());
            self.name = name;
        }

        Ok(())
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

    // This is similar to above, but provides a slightly 'nicer' true / false for lookups, which
    // maps slightly better when converting to something like JSON, this may fully replace the above
    // but for now will sit along side
    pub fn create_router_table(&self) -> [[bool; OutputDevice::COUNT]; InputDevice::COUNT] {
        let mut router = [[false; OutputDevice::COUNT]; InputDevice::COUNT];

        for (input, potential_outputs) in self.profile.settings().mixer().mixer_table().iter() {
            for (channel, volume) in potential_outputs.iter() {
                if *volume > 0 {
                    router[profile_to_standard_input(input) as usize]
                        [profile_to_standard_output(channel) as usize] = true;
                }
            }
        }
        router
    }

    pub fn get_router(&self, input: InputDevice) -> EnumMap<OutputDevice, bool> {
        let mut map: EnumMap<OutputDevice, bool> = EnumMap::default();

        // Get the mixer table
        let mixer = &self.profile.settings().mixer().mixer_table()[standard_input_to_profile(input)];
        for (channel, volume) in mixer.iter() {
            map[profile_to_standard_output(channel)] = *volume > 0;
        }

        return map;
    }

    pub fn set_routing(&mut self, input: InputDevice, output: OutputDevice, enabled: bool) {
        let input = standard_input_to_profile(input);
        let output = standard_output_to_profile(output);

        let mut value = 8192;
        if !enabled {
            value = 0;
        }

        let table = self.profile.settings_mut().mixer_mut().mixer_table_mut();
        table[input][output] = value;

    }

    pub fn get_fader_assignment(&self, fader: FaderName) -> ChannelName {
        let fader = self.profile.settings().fader(fader as usize);
        profile_to_standard_channel(fader.channel())
    }

    pub fn set_fader_assignment(&mut self, fader: FaderName, channel: ChannelName) {
        self.profile
            .settings_mut()
            .fader_mut(fader as usize)
            .set_channel(standard_to_profile_channel(channel));
    }

    pub fn switch_fader_assignment(&mut self, fader_one: FaderName, fader_two: FaderName) {
        // TODO: Scribble?
        self.profile.settings_mut().faders().swap(fader_one as usize, fader_two as usize);
        self.profile.settings_mut().mute_buttons().swap(fader_one as usize, fader_two as usize);
    }

    pub fn set_fader_display(&mut self, fader: FaderName, display: BasicColourDisplay) {
        let colours = self.profile.settings_mut().fader_mut(fader as usize).colour_map_mut();
        colours.set_fader_display(standard_to_profile_fader_display(display));
    }

    // We have a return type here, as there's string parsing involved..
    pub fn set_fader_colours(&mut self, fader: FaderName, top: String, bottom: String) -> Result<()> {
        let colours = self.profile.settings_mut().fader_mut(fader as usize).colour_map_mut();
        if top.len() != 6 || bottom.len() != 6 {
            return Err(anyhow!("Expected Length: 6 (RRGGBB), Top: {}, Bottom: {}", top.len(), bottom.len()));
        }

        colours.set_colour(0, Colour::fromrgb(top.as_str())?);
        colours.set_colour(1, Colour::fromrgb(bottom.as_str())?);
        Ok(())
    }

    pub fn get_channel_volume(&self, channel: ChannelName) -> u8 {
        self.profile
            .settings()
            .mixer()
            .channel_volume(standard_to_profile_channel(channel))
    }

    pub fn get_volumes(&self) -> [u8; ChannelName::COUNT] {
        let mut volumes = [255; ChannelName::COUNT];
        for channel in ChannelName::iter() {
            volumes[channel as usize] = self.get_channel_volume(channel);
        }

        return volumes;
    }

    pub fn set_channel_volume(&mut self, channel: ChannelName, volume: u8) {
        self.profile
            .settings_mut()
            .mixer_mut()
            .set_channel_volume(standard_to_profile_channel(channel), volume);
    }

    pub fn get_colour_map(&self, use_format_1_3_40: bool) -> [u8; 520] {
        let mut colour_array = [0; 520];

        for colour in ColourTargets::iter() {
            let colour_map = get_profile_colour_map(self.profile.settings(), colour);

            for i in 0..colour.get_colour_count() {
                let position = colour.position(i, use_format_1_3_40);

                // Ok, previously this was based on 'is_blank_when_dimmed', but turns out I misinterpreted
                // what was going on there, if a sample button has no samples assigned to it, it'll go
                // dark, so we need to check for that here.
                match colour {
                    ColourTargets::SamplerBottomLeft |
                    ColourTargets::SamplerBottomRight |
                    ColourTargets::SamplerTopLeft |
                    ColourTargets::SamplerTopRight => {
                        if i == 0 {
                            colour_array[position..position + 4].copy_from_slice(&self.get_sampler_lighting(colour));
                        } else {
                            colour_array[position..position + 4]
                                .copy_from_slice(&colour_map.colour(i).to_reverse_bytes());
                        }
                    }
                    _ => {
                        // Update the correct 4 bytes in the map..
                        colour_array[position..position + 4]
                            .copy_from_slice(&colour_map.colour(i).to_reverse_bytes());
                    }
                }
            }
        }

        colour_array
    }

    fn get_sampler_lighting(&self, target: ColourTargets) -> [u8; 4] {
        return match target {
            ColourTargets::SamplerBottomLeft => self.get_colour_array(target, SampleButtons::BottomLeft),
            ColourTargets::SamplerBottomRight => self.get_colour_array(target, SampleButtons::BottomRight),
            ColourTargets::SamplerTopLeft => self.get_colour_array(target, SampleButtons::TopLeft),
            ColourTargets::SamplerTopRight => self.get_colour_array(target, SampleButtons::TopRight),

            // Honestly, we should never reach this, return nothing.
            _ => [00, 00, 00, 00]
        };
    }

    fn get_colour_array(&self, target: ColourTargets, button: SampleButtons) -> [u8;4] {
        if self.current_sample_bank_has_samples(button) {
            return get_profile_colour_map(self.profile.settings(), target)
                .colour(0)
                .to_reverse_bytes();
        } else {
            [00, 00, 00, 00]
        }
    }

    fn get_button_colour_map(&self, button: Buttons) -> &ColourMap {
        get_colour_map_from_button(self.profile.settings(), button)
    }

    /** Regular Mute button handlers */
    fn get_mute_button(&self, fader: FaderName) -> &MuteButton {
        self.profile.settings().mute_button(fader as usize)
    }

    fn get_mute_button_mut(&mut self, fader: FaderName) -> &mut MuteButton {
        self.profile.settings_mut().mute_button_mut(fader as usize)
    }

    pub fn get_mute_button_behaviour(&self, fader: FaderName) -> BasicMuteFunction {
        let mute_config = self.get_mute_button(fader);
        return profile_to_standard_mute_function(*mute_config.mute_function());
    }

    pub fn set_mute_button_behaviour(&mut self, fader: FaderName, behaviour: BasicMuteFunction) {
        let mute_config = self.get_mute_button_mut(fader);
        mute_config.set_mute_function(standard_to_profile_mute_function(behaviour));
    }

    pub fn get_mute_button_state(&self, fader: FaderName) -> (bool, bool, MuteFunction) {
        let mute_config = self.get_mute_button(fader);
        let colour_map = mute_config.colour_map();

        // We should be safe to straight unwrap these, state and blink are always present.
        let muted_to_x = colour_map.state().as_ref().unwrap() == &ColourState::On;
        let muted_to_all = colour_map.blink().as_ref().unwrap() == &ColourState::On;
        let mute_function = mute_config.mute_function().clone();

        return (muted_to_x, muted_to_all, mute_function);
    }

    pub fn get_mute_button_previous_volume(&self, fader: FaderName) -> u8 {
        self.get_mute_button(fader).previous_volume()
    }

    pub fn set_mute_button_previous_volume(&mut self, fader: FaderName, volume: u8) {
        self.get_mute_button_mut(fader).set_previous_volume(volume);
    }

    pub fn set_mute_button_on(&mut self, fader: FaderName, on: bool) {
        self.get_mute_button_mut(fader).colour_map_mut().set_state_on(on);
    }

    pub fn set_mute_button_blink(&mut self, fader: FaderName, on: bool) {
        self.get_mute_button_mut(fader).colour_map_mut().set_blink_on(on);
    }

    /** 'Cough' / Mute Chat Button handlers.. */
    pub fn get_chat_mute_button(&self) -> &MuteChat {
        self.profile.settings().mute_chat()
    }

    pub fn get_chat_mute_button_mut(&mut self) -> &mut MuteChat {
        self.profile.settings_mut().mute_chat_mut()
    }

    pub fn get_chat_mute_button_behaviour(&self) -> BasicMuteFunction {
        let mute_config = self.get_chat_mute_button();
        return profile_to_standard_mute_function(*mute_config.cough_mute_source());
    }

    pub fn set_chat_mute_button_behaviour(&mut self, behaviour: BasicMuteFunction) {
        let mute_config = self.get_chat_mute_button_mut();
        mute_config.set_cough_mute_source(standard_to_profile_mute_function(behaviour));
    }

    pub fn get_mute_chat_button_state(&self) -> (bool, bool, bool, MuteFunction) {
        let mute_config = self.profile.settings().mute_chat();

        // Identical behaviour, different variable locations..
        let mute_toggle = mute_config.is_cough_toggle();
        let muted_to_x = mute_config.cough_button_on();
        let muted_to_all = mute_config.blink() == &ColourState::On;
        let mute_function = mute_config.cough_mute_source().clone();

        return (mute_toggle, muted_to_x, muted_to_all, mute_function);
    }

    pub fn set_mute_chat_button_on(&mut self, on: bool) {
        self.profile.settings_mut().mute_chat_mut().set_cough_button_on(on);
    }

    pub fn set_mute_chat_button_blink(&mut self, on: bool) {
        self.profile.settings_mut().mute_chat_mut().set_blink_on(on);
    }

    pub fn get_mute_chat_button_blink(&self) -> bool {
        self.profile.settings().mute_chat().get_blink_on()
    }

    pub fn get_mute_chat_button_on(&self) -> bool {
        self.profile.settings().mute_chat().get_cough_button_on()
    }

    pub fn get_mute_chat_button_colour_state(&self) -> ButtonStates {
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
    pub fn get_mic_fader_id(&self) -> u8 {
        self.profile.settings().mute_chat().mic_fader_id()
    }

    pub fn set_mic_fader_id(&mut self, id: u8) {
        self.profile.settings_mut().mute_chat_mut().set_mic_fader_id(id);
    }

    pub fn fader_from_id(&self, fader: u8) -> FaderName {
        return match fader {
            0 => FaderName::A,
            1 => FaderName::B,
            2 => FaderName::C,
            _ => FaderName::D
        }
    }

    pub fn is_fader_gradient(&self, fader: FaderName) -> bool {
        self.profile.settings().fader(fader as usize).colour_map().is_fader_gradient()
    }

    pub fn is_fader_meter(&self, fader: FaderName) -> bool {
        self.profile.settings().fader(fader as usize).colour_map().is_fader_meter()
    }

    /** Bleep Button **/
    pub fn set_swear_button_on(&mut self, on: bool) {
        // Get the colour map for the bleep button..
        self.profile.
            settings_mut()
            .simple_element_mut(SimpleElements::Swear)
            .colour_map_mut()
            .set_state_on(on);
    }

    /** Effects Bank Behaviours **/
    pub fn load_effect_bank(&mut self, preset: EffectBankPresets) {
        let preset = standard_to_profile_preset(preset);
        let current = self.profile.settings().context().selected_effects();

        // Ok, first thing we need to do is set the prefix in the profile..
        self.profile.settings_mut().context_mut().set_selected_effects(preset);

        // Disable the 'On' state of the existing button..
        self.profile.settings_mut().effects_mut(current).colour_map_mut().set_state_on(false);

        // Now we need to go through all the buttons, and set their new colour state..
        let state = self.profile.settings_mut().robot_effect().get_preset(preset).state();
        self.profile.settings_mut().robot_effect_mut().colour_map_mut().set_state_on(state);

        let state = self.profile.settings_mut().megaphone_effect().get_preset(preset).state();
        self.profile.settings_mut().megaphone_effect_mut().colour_map_mut().set_state_on(state);

        let state = self.profile.settings_mut().hardtune_effect().get_preset(preset).state();
        self.profile.settings_mut().hardtune_effect_mut().colour_map_mut().set_state_on(state);

        // Set the new button 'On'
        self.profile.settings_mut().effects_mut(preset).colour_map_mut().set_state_on(true);
    }

    pub fn toggle_megaphone(&mut self) {
        let current = self.profile.settings().context().selected_effects();

        let new_state = !self.profile.settings().megaphone_effect().get_preset(current).state();

        self.profile.settings_mut().megaphone_effect_mut().get_preset_mut(current).set_state(new_state);
        self.profile.settings_mut().megaphone_effect_mut().colour_map_mut().set_state_on(new_state);
    }

    pub fn toggle_robot(&mut self) {
        let current = self.profile.settings().context().selected_effects();

        let new_state = !self.profile.settings().robot_effect().get_preset(current).state();

        self.profile.settings_mut().robot_effect_mut().get_preset_mut(current).set_state(new_state);
        self.profile.settings_mut().robot_effect_mut().colour_map_mut().set_state_on(new_state);
    }

    pub fn toggle_hardtune(&mut self) {
        let current = self.profile.settings().context().selected_effects();

        let new_state = !self.profile.settings().hardtune_effect().get_preset(current).state();

        self.profile.settings_mut().hardtune_effect_mut().get_preset_mut(current).set_state(new_state);
        self.profile.settings_mut().hardtune_effect_mut().colour_map_mut().set_state_on(new_state);
    }

    pub fn toggle_effects(&mut self) {
        let state = !self.profile.settings().simple_element(SimpleElements::FxClear).colour_map().get_state();
        self.profile.settings_mut().simple_element_mut(SimpleElements::FxClear).colour_map_mut().set_state_on(state);
    }

    pub fn get_pitch_value(&self) -> i8 {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().pitch_encoder().get_preset(current).knob_position()
    }

    pub fn set_pitch_value(&mut self, value: i8) {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings_mut().pitch_encoder_mut().get_preset_mut(current).set_knob_position(value)
    }

    pub fn get_active_pitch_profile(&self) -> &PitchEncoder {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().pitch_encoder().get_preset(current)
    }

    pub fn get_gender_value(&self) -> i8 {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().gender_encoder().get_preset(current).knob_position()
    }

    pub fn set_gender_value(&mut self, value: i8) {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings_mut().gender_encoder_mut().get_preset_mut(current).set_knob_position(value)
    }

    pub fn get_active_gender_profile(&self) -> &GenderEncoder {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().gender_encoder().get_preset(current)
    }

    pub fn get_reverb_value(&self) -> i8 {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().reverb_encoder().get_preset(current).knob_position()
    }

    pub fn set_reverb_value(&mut self, value: i8) {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings_mut().reverb_encoder_mut().get_preset_mut(current).set_knob_position(value)
    }

    pub fn get_active_reverb_profile(&self) -> &ReverbEncoder {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().reverb_encoder().get_preset(current)
    }

    pub fn get_echo_value(&self) -> i8 {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().echo_encoder().get_preset(current).knob_position()
    }

    pub fn set_echo_value(&mut self, value: i8) {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings_mut().echo_encoder_mut().get_preset_mut(current).set_knob_position(value)
    }

    pub fn get_active_echo_profile(&self) -> &EchoEncoder {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().echo_encoder().get_preset(current)
    }

    pub fn get_active_megaphone_profile(&self) -> &MegaphoneEffect {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().megaphone_effect().get_preset(current)
    }

    pub fn get_active_robot_profile(&self) -> &RobotEffect {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().robot_effect().get_preset(current)
    }

    pub fn get_active_hardtune_profile(&self) -> &HardtuneEffect {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().hardtune_effect().get_preset(current)
    }

    pub fn is_active_hardtune_source_all(&self) -> bool {
        if let Some(source) = self.get_active_hardtune_profile().source() {
            return source == &HardtuneSource::All;
        }

        // If it's not set, assume default behaviour of 'All'
        return true;
    }

    pub fn get_active_hardtune_source(&self) -> InputDevice {
        let source = self.get_active_hardtune_profile().source();
        return match source.unwrap() {
            HardtuneSource::Music => InputDevice::Music,
            HardtuneSource::Game => InputDevice::Game,
            HardtuneSource::LineIn => InputDevice::LineIn,

            // This should never really be called when Source is All, return a default.
            HardtuneSource::All => InputDevice::Music,
        }

    }

    pub fn is_hardtune_pitch_enabled(&self) -> bool {
        self.profile.settings().hardtune_effect().colour_map().get_state()
    }

    pub fn is_pitch_narrow(&self) -> bool {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().pitch_encoder().get_preset(current).style() == &PitchStyle::Narrow
    }

    pub fn is_fx_enabled(&self) -> bool {
        self.profile.settings().simple_element(SimpleElements::FxClear).colour_map().get_state()
    }

    pub fn is_megaphone_enabled(&self) -> bool {
        if !self.is_fx_enabled() {
            return false;
        }
        self.profile.settings().megaphone_effect().colour_map().get_state()
    }

    pub fn is_robot_enabled(&self) -> bool {
        if !self.is_fx_enabled() {
            return false;
        }
        self.profile.settings().robot_effect().colour_map().get_state()
    }

    pub fn is_hardtune_enabled(&self) -> bool {
        if !self.is_fx_enabled() {
            return false;
        }
        self.profile.settings().hardtune_effect().colour_map().get_state()
    }

    /** Sampler Related **/
    pub fn load_sample_bank(&mut self, bank: goxlr_types::SampleBank) {
        let bank = standard_to_profile_sample_bank(bank);
        let current = self.profile.settings().context().selected_sample();

        // Set the new context..
        self.profile.settings_mut().context_mut().set_selected_sample(bank);

        // Disable the 'on' state of the existing bank..
        self.profile.settings_mut()
            .simple_element_mut(sample_bank_to_simple_element(current))
            .colour_map_mut()
            .set_state_on(false);

        // TODO: When loading a bank, we should check for the existance of samples
        // If they're missing, remove them from the stack.

        // Set the 'on' state for the new bank..
        self.profile.settings_mut()
            .simple_element_mut(sample_bank_to_simple_element(bank))
            .colour_map_mut()
            .set_state_on(true);
    }

    pub fn current_sample_bank_has_samples(&self, button: SampleButtons) -> bool {
        let bank = self.profile.settings().context().selected_sample();
        let stack = self.profile.settings().sample_button(button).get_stack(bank);


        if stack.get_sample_count() == 0 {
            return false;
        }
        return true;
    }

    pub fn get_sample_file(&self, button: SampleButtons) -> String {
        let bank = self.profile.settings().context().selected_sample();
        let stack = self.profile.settings().sample_button(button).get_stack(bank);

        stack.get_first_sample_file()
    }

    pub fn is_sample_active(&self, button: SampleButtons) -> bool {
        self.profile.settings().sample_button(button).colour_map().get_state()
    }

    pub fn set_sample_button_state(&mut self, button: SampleButtons, state: bool) {
        self.profile.settings_mut().sample_button_mut(button).colour_map_mut().set_state_on(state);
    }

    /** Colour Changing Code **/
    pub fn set_button_colours(&mut self, target: ButtonColourTargets, colour_one: String, colour_two: Option<&String>) -> Result<()> {
        let colour_target = standard_to_colour_target(target);
        let colours = get_profile_colour_map_mut(self.profile.settings_mut(), colour_target);

        if colour_one.len() != 6 {
            return Err(anyhow!("Expected Length: 6 (RRGGBB), Colour One: {}", colour_one.len()));
        }

        if let Some(two) = colour_two {
            if two.len() != 6 {
                return Err(anyhow!("Expected Length: 6 (RRGGBB), Colour Two: {}", two.len()));
            }
            colours.set_colour(1, Colour::fromrgb(two.as_str())?);
        }
        colours.set_colour(0, Colour::fromrgb(colour_one.as_str())?);
        Ok(())
    }

    pub fn set_button_off_style(&mut self, target: ButtonColourTargets, off_style: BasicColourOffStyle) {
        let colour_target = standard_to_colour_target(target);
        get_profile_colour_map_mut(self.profile.settings_mut(), colour_target).set_off_style(
            standard_to_profile_colour_off_style(off_style)
        );
    }

    // TODO: We can probably do better with grouping these so they can be reused.
    pub fn set_group_button_colours(&mut self, group: ButtonColourGroups, colour_one: String, colour_two: Option<String>) -> Result<()> {
        match group {
            ButtonColourGroups::FaderMute => {
                self.set_button_colours(ButtonColourTargets::Fader1Mute, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::Fader2Mute, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::Fader3Mute, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::Fader4Mute, colour_one.clone(), colour_two.as_ref())?;
            }
            ButtonColourGroups::EffectSelector => {
                self.set_button_colours(ButtonColourTargets::EffectSelect1, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::EffectSelect2, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::EffectSelect3, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::EffectSelect4, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::EffectSelect5, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::EffectSelect6, colour_one.clone(), colour_two.as_ref())?;
            }
            ButtonColourGroups::SampleBankSelector => {
                self.set_button_colours(ButtonColourTargets::SamplerSelectA, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::SamplerSelectB, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::SamplerSelectC, colour_one.clone(), colour_two.as_ref())?;
            }
            ButtonColourGroups::SamplerButtons => {
                self.set_button_colours(ButtonColourTargets::SamplerTopLeft, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::SamplerTopRight, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::SamplerBottomLeft, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::SamplerBottomRight, colour_one.clone(), colour_two.as_ref())?;
                self.set_button_colours(ButtonColourTargets::SamplerClear, colour_one.clone(), colour_two.as_ref())?;
            }
        }

        Ok(())
    }

    pub fn set_group_button_off_style(&mut self, target: ButtonColourGroups, off_style: BasicColourOffStyle) {
        match target {
            ButtonColourGroups::FaderMute => {
                self.set_button_off_style(ButtonColourTargets::Fader1Mute, off_style);
                self.set_button_off_style(ButtonColourTargets::Fader2Mute, off_style);
                self.set_button_off_style(ButtonColourTargets::Fader3Mute, off_style);
                self.set_button_off_style(ButtonColourTargets::Fader4Mute, off_style);
            }
            ButtonColourGroups::EffectSelector => {
                self.set_button_off_style(ButtonColourTargets::EffectSelect1, off_style);
                self.set_button_off_style(ButtonColourTargets::EffectSelect2, off_style);
                self.set_button_off_style(ButtonColourTargets::EffectSelect3, off_style);
                self.set_button_off_style(ButtonColourTargets::EffectSelect4, off_style);
                self.set_button_off_style(ButtonColourTargets::EffectSelect5, off_style);
                self.set_button_off_style(ButtonColourTargets::EffectSelect6, off_style);
            }
            ButtonColourGroups::SampleBankSelector => {
                self.set_button_off_style(ButtonColourTargets::SamplerSelectA, off_style);
                self.set_button_off_style(ButtonColourTargets::SamplerSelectB, off_style);
                self.set_button_off_style(ButtonColourTargets::SamplerSelectC, off_style);
            }
            ButtonColourGroups::SamplerButtons => {
                self.set_button_off_style(ButtonColourTargets::SamplerTopLeft, off_style);
                self.set_button_off_style(ButtonColourTargets::SamplerTopRight, off_style);
                self.set_button_off_style(ButtonColourTargets::SamplerBottomLeft, off_style);
                self.set_button_off_style(ButtonColourTargets::SamplerBottomRight, off_style);
                self.set_button_off_style(ButtonColourTargets::SamplerClear, off_style);
            }
        }
    }


    /** Generic Stuff **/
    pub fn get_button_colour_state(&self, button: Buttons) -> ButtonStates {
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

    pub fn write_profile(&mut self, name: String, directory: &Path, overwrite: bool) -> Result<()> {
        let path = directory.join(format!("{}.goxlrMicProfile", name));
        if !directory.exists() {
            // Attempt to create the profile directory..
            if let Err(e) = create_dir_all(directory) {
                return Err(e).context(format!(
                    "Could not create mic profile directory at {}",
                    directory.to_string_lossy()
                ))?;
            }
        }

        if !overwrite && path.is_file() {
            return Err(anyhow!("Profile exists, will not overwrite"));
        }

        self.profile.save(path)?;

        // Keep our names in sync (in case it was changed)
        if name != self.name() {
            dbg!("Changing Profile Name: {} -> {}", self.name(), name.clone());
            self.name = name;
        }

        Ok(())
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

    pub fn noise_gate_ipc(&self) -> NoiseGate {
        NoiseGate {
            threshold: self.profile.gate().threshold(),
            attack: GateTimes::iter().nth(self.profile.gate().attack() as usize).unwrap(),
            release: GateTimes::iter().nth(self.profile.gate().release() as usize).unwrap(),
            enabled: self.profile.gate().enabled(),
            attenuation: self.profile.gate().attenuation()
        }
    }

    pub fn compressor_ipc(&self) -> Compressor {
        Compressor {
            threshold: self.profile.compressor().threshold(),
            ratio: CompressorRatio::iter().nth(self.profile.compressor().ratio() as usize).unwrap(),
            attack: CompressorAttackTime::iter().nth(self.profile.compressor().attack() as usize).unwrap(),
            release: CompressorReleaseTime::iter().nth(self.profile.compressor().release() as usize).unwrap(),
            makeup_gain: self.profile.compressor().makeup()
        }
    }

    pub fn equalizer_ipc(&self) -> Equaliser {
        Equaliser {
            gain: EqualiserGain {
                eq_31h_gain: self.profile.equalizer().eq_31h_gain(),
                eq_63h_gain: self.profile.equalizer().eq_63h_gain(),
                eq_125h_gain: self.profile.equalizer().eq_125h_gain(),
                eq_250h_gain: self.profile.equalizer().eq_250h_gain(),
                eq_500h_gain: self.profile.equalizer().eq_500h_gain(),
                eq_1k_gain: self.profile.equalizer().eq_1k_gain(),
                eq_2k_gain: self.profile.equalizer().eq_2k_gain(),
                eq_4k_gain: self.profile.equalizer().eq_4k_gain(),
                eq_8k_gain: self.profile.equalizer().eq_8k_gain(),
                eq_16k_gain: self.profile.equalizer().eq_16k_gain(),
            },
            frequency: EqualiserFrequency {
                eq_31h_freq: self.profile.equalizer().eq_31h_freq(),
                eq_63h_freq: self.profile.equalizer().eq_63h_freq(),
                eq_125h_freq: self.profile.equalizer().eq_125h_freq(),
                eq_250h_freq: self.profile.equalizer().eq_250h_freq(),
                eq_500h_freq: self.profile.equalizer().eq_500h_freq(),
                eq_1k_freq: self.profile.equalizer().eq_1k_freq(),
                eq_2k_freq: self.profile.equalizer().eq_2k_freq(),
                eq_4k_freq: self.profile.equalizer().eq_4k_freq(),
                eq_8k_freq: self.profile.equalizer().eq_8k_freq(),
                eq_16k_freq: self.profile.equalizer().eq_16k_freq()
            }
        }
    }

    pub fn equalizer_mini_ipc(&self) -> EqualiserMini {
        EqualiserMini {
            gain: EqualiserMiniGain {
                eq_90h_gain: self.profile.equalizer_mini().eq_90h_gain(),
                eq_250h_gain: self.profile.equalizer_mini().eq_250h_gain(),
                eq_500h_gain: self.profile.equalizer_mini().eq_500h_gain(),
                eq_1k_gain: self.profile.equalizer_mini().eq_1k_gain(),
                eq_3k_gain: self.profile.equalizer_mini().eq_3k_gain(),
                eq_8k_gain: self.profile.equalizer_mini().eq_8k_gain()
            },
            frequency: EqualiserMiniFrequency {
                eq_90h_freq: self.profile.equalizer_mini().eq_90h_freq(),
                eq_250h_freq: self.profile.equalizer_mini().eq_250h_freq(),
                eq_500h_freq: self.profile.equalizer_mini().eq_500h_freq(),
                eq_1k_freq: self.profile.equalizer_mini().eq_1k_freq(),
                eq_3k_freq: self.profile.equalizer_mini().eq_3k_freq(),
                eq_8k_freq: self.profile.equalizer_mini().eq_8k_freq()
            }
        }
    }

    pub fn set_mic_type(&mut self, mic_type: MicrophoneType) {
        self.profile.setup_mut().set_mic_type(mic_type as u8);
    }

    pub fn set_mic_gain(&mut self, mic_type: MicrophoneType, gain: u16) {
        match mic_type {
            MicrophoneType::Dynamic => self.profile.setup_mut().set_dynamic_mic_gain(gain),
            MicrophoneType::Condenser => self.profile.setup_mut().set_condenser_mic_gain(gain),
            MicrophoneType::Jack => self.profile.setup_mut().set_trs_mic_gain(gain)
        }
    }

    pub fn set_eq_gain(&mut self, gain: EqFrequencies, value: i8) -> EffectKey {
        return match gain {
            EqFrequencies::Equalizer31Hz => {
                self.profile.equalizer_mut().set_eq_31h_gain(value);
                EffectKey::Equalizer31HzGain
            }
            EqFrequencies::Equalizer63Hz => {
                self.profile.equalizer_mut().set_eq_63h_gain(value);
                EffectKey::Equalizer63HzGain
            }
            EqFrequencies::Equalizer125Hz => {
                self.profile.equalizer_mut().set_eq_125h_gain(value);
                EffectKey::Equalizer125HzGain
            }
            EqFrequencies::Equalizer250Hz => {
                self.profile.equalizer_mut().set_eq_250h_gain(value);
                EffectKey::Equalizer250HzGain
            }
            EqFrequencies::Equalizer500Hz => {
                self.profile.equalizer_mut().set_eq_500h_gain(value);
                EffectKey::Equalizer500HzGain
            }
            EqFrequencies::Equalizer1KHz => {
                self.profile.equalizer_mut().set_eq_1k_gain(value);
                EffectKey::Equalizer1KHzGain
            }
            EqFrequencies::Equalizer2KHz => {
                self.profile.equalizer_mut().set_eq_2k_gain(value);
                EffectKey::Equalizer2KHzGain
            }
            EqFrequencies::Equalizer4KHz => {
                self.profile.equalizer_mut().set_eq_4k_gain(value);
                EffectKey::Equalizer4KHzGain
            }
            EqFrequencies::Equalizer8KHz => {
                self.profile.equalizer_mut().set_eq_8k_gain(value);
                EffectKey::Equalizer8KHzGain
            }
            EqFrequencies::Equalizer16KHz => {
                self.profile.equalizer_mut().set_eq_16k_gain(value);
                EffectKey::Equalizer16KHzGain
            }
        }
    }

    pub fn set_eq_freq(&mut self, freq: EqFrequencies, value: f32) -> Result<EffectKey> {
        return match freq {
            EqFrequencies::Equalizer31Hz => {
                if value < 30.0 || value > 300.0 {
                    return Err(anyhow!("31Hz Frequency must be between 30.0 and 300.0"));
                }

                self.profile.equalizer_mut().set_eq_31h_freq(value);
                Ok(EffectKey::Equalizer31HzFrequency)
            }
            EqFrequencies::Equalizer63Hz => {
                if value < 30.0 || value > 300.0 {
                    return Err(anyhow!("63Hz Frequency must be between 30.0 and 300.0"));
                }

                self.profile.equalizer_mut().set_eq_63h_freq(value);
                Ok(EffectKey::Equalizer63HzFrequency)
            }
            EqFrequencies::Equalizer125Hz => {
                if value < 30.0 || value > 300.0 {
                    return Err(anyhow!("125Hz Frequency must be between 30.0 and 300.0"));
                }

                self.profile.equalizer_mut().set_eq_125h_freq(value);
                Ok(EffectKey::Equalizer125HzFrequency)
            }
            EqFrequencies::Equalizer250Hz => {
                if value < 30.0 || value > 300.0 {
                    return Err(anyhow!("250Hz Frequency must be between 30.0 and 300.0"));
                }

                self.profile.equalizer_mut().set_eq_250h_freq(value);
                Ok(EffectKey::Equalizer250HzFrequency)
            }
            EqFrequencies::Equalizer500Hz => {
                if value < 300.0 || value > 2000.0 {
                    return Err(anyhow!("500Hz Frequency must be between 300.0 and 2000.0"));
                }

                self.profile.equalizer_mut().set_eq_500h_freq(value);
                Ok(EffectKey::Equalizer500HzFrequency)
            }
            EqFrequencies::Equalizer1KHz => {
                if value < 300.0 || value > 2000.0 {
                    return Err(anyhow!("1KHz Frequency must be between 300.0 and 2000.0"));
                }

                self.profile.equalizer_mut().set_eq_1k_freq(value);
                Ok(EffectKey::Equalizer1KHzFrequency)
            }
            EqFrequencies::Equalizer2KHz => {
                if value < 300.0 || value > 2000.0 {
                    return Err(anyhow!("2KHz Frequency must be between 300.0 and 2000.0"));
                }

                self.profile.equalizer_mut().set_eq_2k_freq(value);
                Ok(EffectKey::Equalizer2KHzFrequency)
            }
            EqFrequencies::Equalizer4KHz => {
                if value < 2000.0 || value > 18000.0 {
                    return Err(anyhow!("4KHz Frequency must be between 2000.0 and 18000.0"));
                }

                self.profile.equalizer_mut().set_eq_4k_freq(value);
                Ok(EffectKey::Equalizer4KHzFrequency)
            }
            EqFrequencies::Equalizer8KHz => {
                if value < 2000.0 || value > 18000.0 {
                    return Err(anyhow!("8KHz Frequency must be between 2000.0 and 18000.0"));
                }

                self.profile.equalizer_mut().set_eq_8k_freq(value);
                Ok(EffectKey::Equalizer8KHzFrequency)
            }
            EqFrequencies::Equalizer16KHz => {
                if value < 2000.0 || value > 18000.0 {
                    return Err(anyhow!("16KHz Frequency must be between 2000.0 and 18000.0"));
                }

                self.profile.equalizer_mut().set_eq_16k_freq(value);
                Ok(EffectKey::Equalizer16KHzFrequency)
            }
        }
    }

    pub fn set_mini_eq_gain(&mut self, gain: MiniEqFrequencies, value: i8) -> MicrophoneParamKey {
        return match gain {
            MiniEqFrequencies::Equalizer90Hz => {
                self.profile.equalizer_mini_mut().set_eq_90h_gain(value);
                MicrophoneParamKey::Equalizer90HzGain
            }
            MiniEqFrequencies::Equalizer250Hz => {
                self.profile.equalizer_mini_mut().set_eq_250h_gain(value);
                MicrophoneParamKey::Equalizer250HzGain
            }
            MiniEqFrequencies::Equalizer500Hz => {
                self.profile.equalizer_mini_mut().set_eq_500h_gain(value);
                MicrophoneParamKey::Equalizer500HzGain
            }
            MiniEqFrequencies::Equalizer1KHz => {
                self.profile.equalizer_mini_mut().set_eq_1k_gain(value);
                MicrophoneParamKey::Equalizer1KHzGain
            }
            MiniEqFrequencies::Equalizer3KHz => {
                self.profile.equalizer_mini_mut().set_eq_3k_gain(value);
                MicrophoneParamKey::Equalizer3KHzGain
            }
            MiniEqFrequencies::Equalizer8KHz => {
                self.profile.equalizer_mini_mut().set_eq_8k_gain(value);
                MicrophoneParamKey::Equalizer8KHzGain
            }
        }
    }

    pub fn set_mini_eq_freq(&mut self, freq: MiniEqFrequencies, value: f32) -> MicrophoneParamKey {
        return match freq {
            MiniEqFrequencies::Equalizer90Hz => {
                self.profile.equalizer_mini_mut().set_eq_90h_freq(value);
                MicrophoneParamKey::Equalizer90HzFrequency
            }
            MiniEqFrequencies::Equalizer250Hz => {
                self.profile.equalizer_mini_mut().set_eq_250h_freq(value);
                MicrophoneParamKey::Equalizer250HzFrequency
            }
            MiniEqFrequencies::Equalizer500Hz => {
                self.profile.equalizer_mini_mut().set_eq_500h_freq(value);
                MicrophoneParamKey::Equalizer500HzFrequency
            }
            MiniEqFrequencies::Equalizer1KHz => {
                self.profile.equalizer_mini_mut().set_eq_1k_freq(value);
                MicrophoneParamKey::Equalizer1KHzFrequency
            }
            MiniEqFrequencies::Equalizer3KHz => {
                self.profile.equalizer_mini_mut().set_eq_3k_freq(value);
                MicrophoneParamKey::Equalizer3KHzFrequency
            }
            MiniEqFrequencies::Equalizer8KHz => {
                self.profile.equalizer_mini_mut().set_eq_8k_freq(value);
                MicrophoneParamKey::Equalizer8KHzFrequency
            }
        }
    }

    pub fn set_gate_threshold(&mut self, value: i8) {
        self.profile.gate_mut().set_threshold(value);
    }

    pub fn set_gate_attenuation(&mut self, value: u8) {
        self.profile.gate_mut().set_attenuation(value);
    }

    pub fn set_gate_attack(&mut self, value: GateTimes) {
        self.profile.gate_mut().set_attack(value as u8);
    }

    pub fn set_gate_release(&mut self,value: GateTimes) {
        self.profile.gate_mut().set_release(value as u8);
    }

    pub fn set_gate_active(&mut self, value: bool) {
        self.profile.gate_mut().set_enabled(value);
    }

    pub fn set_compressor_threshold(&mut self, value: i8) {
        self.profile.compressor_mut().set_threshold(value);
    }

    pub fn set_compressor_ratio(&mut self, value: CompressorRatio) {
        self.profile.compressor_mut().set_ratio(value as u8);
    }

    pub fn set_compressor_attack(&mut self, value: CompressorAttackTime) {
        self.profile.compressor_mut().set_attack(value as u8);
    }

    pub fn set_compressor_release(&mut self, value: CompressorReleaseTime) {
        self.profile.compressor_mut().set_release(value as u8);
    }

    pub fn set_compressor_makeup(&mut self, value: u8) {
        self.profile.compressor_mut().set_makeup_gain(value);
    }

    /// The uber method, fetches the relevant setting from the profile and returns it..
    pub fn get_param_value(&self, param: MicrophoneParamKey, serial: &str, settings: &SettingsHandle) -> [u8; 4] {
        match param {
            MicrophoneParamKey::MicType => {
                let microphone_type: MicrophoneType = self.mic_type();
                match microphone_type.has_phantom_power() {
                    true => [0x01 as u8, 0, 0, 0],
                    false => [0, 0, 0, 0],
                }
            },
            MicrophoneParamKey::DynamicGain => self.gain_value(self.mic_gains()[MicrophoneType::Dynamic as usize]),
            MicrophoneParamKey::CondenserGain => self.gain_value(self.mic_gains()[MicrophoneType::Condenser as usize]),
            MicrophoneParamKey::JackGain => self.gain_value(self.mic_gains()[MicrophoneType::Jack as usize]),
            MicrophoneParamKey::GateThreshold => self.i8_to_f32(self.profile.gate().threshold()),
            MicrophoneParamKey::GateAttack => self.u8_to_f32(self.profile.gate().attack()),
            MicrophoneParamKey::GateRelease => self.u8_to_f32(self.profile.gate().release()),
            MicrophoneParamKey::GateAttenuation => self.i8_to_f32(self.gate_attenuation_from_percent(self.profile.gate().attenuation())),
            MicrophoneParamKey::CompressorThreshold => self.i8_to_f32(self.profile.compressor().threshold()),
            MicrophoneParamKey::CompressorRatio => self.u8_to_f32(self.profile.compressor().ratio()),
            MicrophoneParamKey::CompressorAttack => self.u8_to_f32(self.profile.compressor().ratio()),
            MicrophoneParamKey::CompressorRelease => self.u8_to_f32(self.profile.compressor().release()),
            MicrophoneParamKey::CompressorMakeUpGain => self.u8_to_f32(self.profile.compressor().makeup()),
            MicrophoneParamKey::BleepLevel => {
                // Hopefully we can eventually move this to the profile, it's a little obnoxious right now!
                let bleep_value = block_on(settings.get_device_bleep_volume(serial)).unwrap_or(-20);
                self.calculate_bleep(bleep_value)
            },
            MicrophoneParamKey::Equalizer90HzFrequency => self.f32_to_f32(self.profile.equalizer_mini().eq_90h_freq()),
            MicrophoneParamKey::Equalizer90HzGain => self.i8_to_f32(self.profile.equalizer_mini().eq_90h_gain()),
            MicrophoneParamKey::Equalizer250HzFrequency => self.f32_to_f32(self.profile.equalizer_mini().eq_250h_freq()),
            MicrophoneParamKey::Equalizer250HzGain => self.i8_to_f32(self.profile.equalizer_mini().eq_250h_gain()),
            MicrophoneParamKey::Equalizer500HzFrequency => self.f32_to_f32(self.profile.equalizer_mini().eq_500h_freq()),
            MicrophoneParamKey::Equalizer500HzGain => self.i8_to_f32(self.profile.equalizer_mini().eq_500h_gain()),
            MicrophoneParamKey::Equalizer1KHzFrequency => self.f32_to_f32(self.profile.equalizer_mini().eq_1k_freq()),
            MicrophoneParamKey::Equalizer1KHzGain => self.i8_to_f32(self.profile.equalizer_mini().eq_1k_gain()),
            MicrophoneParamKey::Equalizer3KHzFrequency => self.f32_to_f32(self.profile.equalizer_mini().eq_3k_freq()),
            MicrophoneParamKey::Equalizer3KHzGain => self.i8_to_f32(self.profile.equalizer_mini().eq_3k_gain()),
            MicrophoneParamKey::Equalizer8KHzFrequency => self.f32_to_f32(self.profile.equalizer_mini().eq_8k_freq()),
            MicrophoneParamKey::Equalizer8KHzGain => self.i8_to_f32(self.profile.equalizer_mini().eq_8k_gain()),
        }
    }

    fn calculate_bleep(&self, value: i8) -> [u8;4] {
        // TODO: Confirm the output here..
        let mut return_value = [0;4];
        LittleEndian::write_f32(&mut return_value, value as f32 * 65536.0);
        return return_value;
    }

    /// This is going to require a CRAPLOAD of work to sort..
    pub fn get_effect_value(&self,
                            effect: EffectKey,
                            serial: &str,
                            settings: &SettingsHandle,
                            main_profile: &ProfileAdapter
    ) -> i32 {
        match effect {
            EffectKey::DisableMic => {
                // TODO: Actually use this..
                // Originally I favoured just muting the mic channel, but discovered during testing
                // of the effects that the mic is still read even when the channel is muted, so we
                // need to correctly send this when the mic gets muted / unmuted.
                0
            },
            EffectKey::BleepLevel => block_on(settings.get_device_bleep_volume(serial)).unwrap_or(-20).into(),
            EffectKey::GateMode => 2,   // Not a profile setting, hard coded in Windows
            EffectKey::GateEnabled => 1,    // Used for 'Mic Testing' in the UI
            EffectKey::GateThreshold => self.profile.gate().threshold().into(),
            EffectKey::GateAttenuation => self.gate_attenuation_from_percent(self.profile.gate().attenuation()).into(),
            EffectKey::GateAttack => self.profile.gate().attack().into(),
            EffectKey::GateRelease => self.profile.gate().release().into(),
            EffectKey::Unknown14b => 0,

            EffectKey::Equalizer31HzFrequency => self.profile.equalizer().eq_31h_freq_as_goxlr(),
            EffectKey::Equalizer63HzFrequency => self.profile.equalizer().eq_63h_freq_as_goxlr(),
            EffectKey::Equalizer125HzFrequency => self.profile.equalizer().eq_125h_freq_as_goxlr(),
            EffectKey::Equalizer250HzFrequency => self.profile.equalizer().eq_250h_freq_as_goxlr(),
            EffectKey::Equalizer500HzFrequency => self.profile.equalizer().eq_500h_freq_as_goxlr(),
            EffectKey::Equalizer1KHzFrequency => self.profile.equalizer().eq_1k_freq_as_goxlr(),
            EffectKey::Equalizer2KHzFrequency => self.profile.equalizer().eq_2k_freq_as_goxlr(),
            EffectKey::Equalizer4KHzFrequency => self.profile.equalizer().eq_4k_freq_as_goxlr(),
            EffectKey::Equalizer8KHzFrequency => self.profile.equalizer().eq_8k_freq_as_goxlr(),
            EffectKey::Equalizer16KHzFrequency => self.profile.equalizer().eq_16k_freq_as_goxlr(),

            EffectKey::Equalizer31HzGain => self.profile.equalizer().eq_31h_gain().into(),
            EffectKey::Equalizer63HzGain => self.profile.equalizer().eq_63h_gain().into(),
            EffectKey::Equalizer125HzGain => self.profile.equalizer().eq_125h_gain().into(),
            EffectKey::Equalizer250HzGain => self.profile.equalizer().eq_250h_gain().into(),
            EffectKey::Equalizer500HzGain => self.profile.equalizer().eq_500h_gain().into(),
            EffectKey::Equalizer1KHzGain => self.profile.equalizer().eq_1k_gain().into(),
            EffectKey::Equalizer2KHzGain => self.profile.equalizer().eq_2k_gain().into(),
            EffectKey::Equalizer4KHzGain => self.profile.equalizer().eq_4k_gain().into(),
            EffectKey::Equalizer8KHzGain => self.profile.equalizer().eq_8k_gain().into(),
            EffectKey::Equalizer16KHzGain => self.profile.equalizer().eq_16k_gain().into(),

            EffectKey::CompressorThreshold => self.profile.compressor().threshold().into(),
            EffectKey::CompressorRatio => self.profile.compressor().ratio().into(),
            EffectKey::CompressorAttack => self.profile.compressor().attack().into(),
            EffectKey::CompressorRelease => self.profile.compressor().release().into(),
            EffectKey::CompressorMakeUpGain => self.profile.compressor().makeup().into(),

            EffectKey::DeEsser => self.get_deesser(),

            EffectKey::ReverbAmount => main_profile.get_active_reverb_profile().amount().into(),
            EffectKey::ReverbDecay => main_profile.get_active_reverb_profile().decay().into(),
            EffectKey::ReverbEarlyLevel => main_profile.get_active_reverb_profile().early_level().into(),
            EffectKey::ReverbTailLevel => 0, // Always 0 from the Windows UI
            EffectKey::ReverbPredelay => main_profile.get_active_reverb_profile().predelay().into(),
            EffectKey::ReverbLoColor => main_profile.get_active_reverb_profile().locolor().into(),
            EffectKey::ReverbHiColor => main_profile.get_active_reverb_profile().hicolor().into(),
            EffectKey::ReverbHiFactor => main_profile.get_active_reverb_profile().hifactor().into(),
            EffectKey::ReverbDiffuse => main_profile.get_active_reverb_profile().diffuse().into(),
            EffectKey::ReverbModSpeed => main_profile.get_active_reverb_profile().mod_speed().into(),
            EffectKey::ReverbModDepth => main_profile.get_active_reverb_profile().mod_depth().into(),
            EffectKey::ReverbStyle => *main_profile.get_active_reverb_profile().style() as i32,

            EffectKey::EchoAmount => main_profile.get_active_echo_profile().amount().into(),
            EffectKey::EchoFeedback => main_profile.get_active_echo_profile().feedback_control().into(),
            EffectKey::EchoTempo => main_profile.get_active_echo_profile().tempo().into(),
            EffectKey::EchoDelayL => main_profile.get_active_echo_profile().time_left().into(),
            EffectKey::EchoDelayR => main_profile.get_active_echo_profile().time_right().into(),
            EffectKey::EchoFeedbackL => main_profile.get_active_echo_profile().feedback_left().into(),
            EffectKey::EchoXFBLtoR => main_profile.get_active_echo_profile().xfb_l_to_r().into(),
            EffectKey::EchoFeedbackR => main_profile.get_active_echo_profile().feedback_right().into(),
            EffectKey::EchoXFBRtoL => main_profile.get_active_echo_profile().xfb_r_to_l().into(),
            EffectKey::EchoSource => main_profile.get_active_echo_profile().source() as i32,
            EffectKey::EchoDivL => main_profile.get_active_echo_profile().div_l().into(),
            EffectKey::EchoDivR => main_profile.get_active_echo_profile().div_r().into(),
            EffectKey::EchoFilterStyle => main_profile.get_active_echo_profile().filter_style().into(),

            EffectKey::PitchAmount => main_profile.get_active_pitch_profile().knob_position().into(),
            EffectKey::PitchThreshold => main_profile.get_active_pitch_profile().threshold().into(),
            EffectKey::PitchCharacter => main_profile.get_active_pitch_profile().inst_ratio_value().into(),

            EffectKey::GenderAmount => main_profile.get_active_gender_profile().amount().into(),

            EffectKey::MegaphoneAmount => main_profile.get_active_megaphone_profile().trans_dist_amt().into(),
            EffectKey::MegaphonePostGain => main_profile.get_active_megaphone_profile().trans_postgain().into(),
            EffectKey::MegaphoneStyle => *main_profile.get_active_megaphone_profile().style() as i32,
            EffectKey::MegaphoneHP => main_profile.get_active_megaphone_profile().trans_hp().into(),
            EffectKey::MegaphoneLP => main_profile.get_active_megaphone_profile().trans_lp().into(),
            EffectKey::MegaphonePreGain => main_profile.get_active_megaphone_profile().trans_pregain().into(),
            EffectKey::MegaphoneDistType => main_profile.get_active_megaphone_profile().trans_dist_type().into(),
            EffectKey::MegaphonePresenceGain => main_profile.get_active_megaphone_profile().trans_presence_gain().into(),
            EffectKey::MegaphonePresenceFC => main_profile.get_active_megaphone_profile().trans_presence_fc().into(),
            EffectKey::MegaphonePresenceBW => main_profile.get_active_megaphone_profile().trans_presence_bw().into(),
            EffectKey::MegaphoneBeatboxEnable => main_profile.get_active_megaphone_profile().trans_beatbox_enabled().into(),
            EffectKey::MegaphoneFilterControl => main_profile.get_active_megaphone_profile().trans_filter_control().into(),
            EffectKey::MegaphoneFilter => main_profile.get_active_megaphone_profile().trans_filter().into(),
            EffectKey::MegaphoneDrivePotGainCompMid => main_profile.get_active_megaphone_profile().trans_drive_pot_gain_comp_mid().into(),
            EffectKey::MegaphoneDrivePotGainCompMax => main_profile.get_active_megaphone_profile().trans_drive_pot_gain_comp_max().into(),

            EffectKey::HardTuneAmount => main_profile.get_active_hardtune_profile().amount().into(),
            EffectKey::HardTuneKeySource => 0,  // Always 0, HardTune is handled through routing
            EffectKey::HardTuneScale => main_profile.get_active_hardtune_profile().scale().into(),
            EffectKey::HardTunePitchAmount => main_profile.get_active_hardtune_profile().pitch_amt().into(),
            EffectKey::HardTuneRate => main_profile.get_active_hardtune_profile().rate().into(),
            EffectKey::HardTuneWindow => main_profile.get_active_hardtune_profile().window().into(),

            EffectKey::RobotLowGain => main_profile.get_active_robot_profile().vocoder_low_gain().into(),
            EffectKey::RobotLowFreq => main_profile.get_active_robot_profile().vocoder_low_freq().into(),
            EffectKey::RobotLowWidth => main_profile.get_active_robot_profile().vocoder_low_bw().into(),
            EffectKey::RobotMidGain => main_profile.get_active_robot_profile().vocoder_mid_gain().into(),
            EffectKey::RobotMidFreq => main_profile.get_active_robot_profile().vocoder_mid_freq().into(),
            EffectKey::RobotMidWidth => main_profile.get_active_robot_profile().vocoder_mid_bw().into(),
            EffectKey::RobotHiGain => main_profile.get_active_robot_profile().vocoder_high_gain().into(),
            EffectKey::RobotHiFreq => main_profile.get_active_robot_profile().vocoder_high_freq().into(),
            EffectKey::RobotHiWidth => main_profile.get_active_robot_profile().vocoder_high_bw().into(),
            EffectKey::RobotWaveform => main_profile.get_active_robot_profile().synthosc_waveform().into(),
            EffectKey::RobotPulseWidth => main_profile.get_active_robot_profile().synthosc_pulse_width().into(),
            EffectKey::RobotThreshold => main_profile.get_active_robot_profile().vocoder_gate_threshold().into(),
            EffectKey::RobotDryMix => main_profile.get_active_robot_profile().dry_mix().into(),
            EffectKey::RobotStyle => *main_profile.get_active_robot_profile().style() as i32,

            EffectKey::RobotEnabled => main_profile.is_robot_enabled().into(),
            EffectKey::MegaphoneEnabled => main_profile.is_megaphone_enabled().into(),
            EffectKey::HardTuneEnabled => main_profile.is_hardtune_enabled().into(),

            // Encoders are always enabled when FX is enabled..
            EffectKey::Encoder1Enabled => main_profile.is_fx_enabled().into(),
            EffectKey::Encoder2Enabled => main_profile.is_fx_enabled().into(),
            EffectKey::Encoder3Enabled => main_profile.is_fx_enabled().into(),
            EffectKey::Encoder4Enabled => main_profile.is_fx_enabled().into(),
        }
    }

    fn u8_to_f32(&self, value: u8) -> [u8; 4] {
        let mut return_value = [0;4];
        LittleEndian::write_f32(&mut return_value, value.into());
        return return_value;
    }

    fn i8_to_f32(&self, value: i8) -> [u8; 4] {
        let mut return_value = [0;4];
        LittleEndian::write_f32(&mut return_value, value.into());
        return return_value;
    }

    fn f32_to_f32(&self, value: f32) -> [u8; 4] {
        let mut return_value = [0;4];
        LittleEndian::write_f32(&mut return_value, value.into());
        return return_value;
    }

    fn gain_value(&self, value: u16) -> [u8; 4] {
        let mut return_value = [0;4];
        LittleEndian::write_u16(&mut return_value[2..], value);
        return return_value;
    }

    /*
    Gate attenuation is an interesting one, it's stored and represented as a percent,
    but implemented as a non-linear array, so we're going to implement this the same way
    the Windows client does.
     */
    fn gate_attenuation_from_percent(&self, value: u8) -> i8 {
        let index = value as f32 * 0.24;

        if value > 99 {
            return GATE_ATTENUATION[25];
        }

        return GATE_ATTENUATION[index as usize];
    }

    pub fn get_common_keys(&self) -> HashSet<EffectKey> {
        let mut keys = HashSet::new();
        keys.insert(EffectKey::DeEsser);
        keys.insert(EffectKey::GateThreshold);
        keys.insert(EffectKey::GateAttack);
        keys.insert(EffectKey::GateRelease);
        keys.insert(EffectKey::GateAttenuation);
        keys.insert(EffectKey::CompressorThreshold);
        keys.insert(EffectKey::CompressorRatio);
        keys.insert(EffectKey::CompressorAttack);
        keys.insert(EffectKey::CompressorRelease);
        keys.insert(EffectKey::CompressorMakeUpGain);
        keys.insert(EffectKey::GateEnabled);
        keys.insert(EffectKey::BleepLevel);
        keys.insert(EffectKey::GateMode);
        keys.insert(EffectKey::DisableMic);

        // TODO: Are these common?
        keys.insert(EffectKey::Encoder1Enabled);
        keys.insert(EffectKey::Encoder2Enabled);
        keys.insert(EffectKey::Encoder3Enabled);
        keys.insert(EffectKey::Encoder4Enabled);

        keys.insert(EffectKey::RobotEnabled);
        keys.insert(EffectKey::HardTuneEnabled);
        keys.insert(EffectKey::MegaphoneEnabled);

        return keys;
    }

    pub fn get_full_keys(&self) -> HashSet<EffectKey> {
        let mut keys = HashSet::new();

        // Lets go mental, return everything that's not common..
        let common_effects = self.get_common_keys();

        for effect in EffectKey::iter() {
            if !common_effects.contains(&effect) {
                keys.insert(effect);
            }
        }

        return keys;
    }

    // These are specific Group Key sets, useful for applying a specific effect at once.
    pub fn get_reverb_keyset(&self) -> HashSet<EffectKey> {
        let mut set = HashSet::new();
        set.insert(EffectKey::ReverbAmount);
        set.insert(EffectKey::ReverbDecay);
        set.insert(EffectKey::ReverbEarlyLevel);
        set.insert(EffectKey::ReverbTailLevel);
        set.insert(EffectKey::ReverbPredelay);
        set.insert(EffectKey::ReverbLoColor);
        set.insert(EffectKey::ReverbHiColor);
        set.insert(EffectKey::ReverbHiFactor);
        set.insert(EffectKey::ReverbDiffuse);
        set.insert(EffectKey::ReverbModSpeed);
        set.insert(EffectKey::ReverbModDepth);
        set.insert(EffectKey::ReverbStyle);

        set
    }

    pub fn get_echo_keyset(&self) -> HashSet<EffectKey> {
        let mut set = HashSet::new();
        set.insert(EffectKey::EchoAmount);
        set.insert(EffectKey::EchoFeedback);
        set.insert(EffectKey::EchoTempo);
        set.insert(EffectKey::EchoDelayL);
        set.insert(EffectKey::EchoDelayR);
        set.insert(EffectKey::EchoFeedbackL);
        set.insert(EffectKey::EchoFeedbackR);
        set.insert(EffectKey::EchoXFBLtoR);
        set.insert(EffectKey::EchoXFBRtoL);
        set.insert(EffectKey::EchoSource);
        set.insert(EffectKey::EchoDivL);
        set.insert(EffectKey::EchoDivR);
        set.insert(EffectKey::EchoFilterStyle);

        set
    }

    pub fn get_pitch_keyset(&self) -> HashSet<EffectKey> {
        let mut set = HashSet::new();
        set.insert(EffectKey::PitchAmount);
        set.insert(EffectKey::PitchThreshold);
        set.insert(EffectKey::PitchCharacter);

        set
    }

    pub fn get_gender_keyset(&self) -> HashSet<EffectKey> {
        let mut set = HashSet::new();
        set.insert(EffectKey::GenderAmount);

        set
    }

    pub fn get_megaphone_keyset(&self) -> HashSet<EffectKey> {
        let mut set = HashSet::new();
        set.insert(EffectKey::MegaphoneAmount);
        set.insert(EffectKey::MegaphonePostGain);
        set.insert(EffectKey::MegaphoneStyle);
        set.insert(EffectKey::MegaphoneHP);
        set.insert(EffectKey::MegaphoneLP);
        set.insert(EffectKey::MegaphonePreGain);
        set.insert(EffectKey::MegaphoneDistType);
        set.insert(EffectKey::MegaphonePresenceGain);
        set.insert(EffectKey::MegaphonePresenceFC);
        set.insert(EffectKey::MegaphonePresenceBW);
        set.insert(EffectKey::MegaphoneBeatboxEnable);
        set.insert(EffectKey::MegaphoneFilterControl);
        set.insert(EffectKey::MegaphoneFilter);
        set.insert(EffectKey::MegaphoneDrivePotGainCompMid);
        set.insert(EffectKey::MegaphoneDrivePotGainCompMax);

        set
    }

    pub fn get_robot_keyset(&self) -> HashSet<EffectKey> {
        let mut set = HashSet::new();
        set.insert(EffectKey::RobotLowGain);
        set.insert(EffectKey::RobotLowFreq);
        set.insert(EffectKey::RobotLowWidth);
        set.insert(EffectKey::RobotMidGain);
        set.insert(EffectKey::RobotMidFreq);
        set.insert(EffectKey::RobotMidWidth);
        set.insert(EffectKey::RobotHiGain);
        set.insert(EffectKey::RobotHiFreq);
        set.insert(EffectKey::RobotHiWidth);
        set.insert(EffectKey::RobotWaveform);
        set.insert(EffectKey::RobotPulseWidth);
        set.insert(EffectKey::RobotThreshold);
        set.insert(EffectKey::RobotDryMix);
        set.insert(EffectKey::RobotStyle);

        set
    }

    pub fn get_hardtune_keyset(&self) -> HashSet<EffectKey> {
        let mut set = HashSet::new();
        set.insert(EffectKey::HardTuneAmount);
        set.insert(EffectKey::HardTuneKeySource);
        set.insert(EffectKey::HardTuneScale);
        set.insert(EffectKey::HardTunePitchAmount);
        set.insert(EffectKey::HardTuneRate);
        set.insert(EffectKey::HardTuneWindow);

        set
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

fn standard_output_to_profile(value: OutputDevice) -> OutputChannels {
    match value {
        OutputDevice::Headphones => OutputChannels::Headphones,
        OutputDevice::BroadcastMix => OutputChannels::Broadcast,
        OutputDevice::LineOut => OutputChannels::LineOut,
        OutputDevice::ChatMic => OutputChannels::ChatMic,
        OutputDevice::Sampler => OutputChannels::Sampler,
    }
}

fn profile_to_standard_mute_function(value: MuteFunction) -> BasicMuteFunction {
    match value {
        MuteFunction::All => BasicMuteFunction::All,
        MuteFunction::ToStream => BasicMuteFunction::ToStream,
        MuteFunction::ToVoiceChat => BasicMuteFunction::ToVoiceChat,
        MuteFunction::ToPhones => BasicMuteFunction::ToPhones,
        MuteFunction::ToLineOut => BasicMuteFunction::ToLineOut
    }
}

fn standard_to_profile_mute_function(value: BasicMuteFunction) -> MuteFunction {
    match value {
        BasicMuteFunction::All => MuteFunction::All,
        BasicMuteFunction::ToStream => MuteFunction::ToStream,
        BasicMuteFunction::ToVoiceChat => MuteFunction::ToVoiceChat,
        BasicMuteFunction::ToPhones => MuteFunction::ToPhones,
        BasicMuteFunction::ToLineOut => MuteFunction::ToLineOut
    }
}

fn standard_to_profile_fader_display(value: BasicColourDisplay) -> ColourDisplay {
    match value {
        BasicColourDisplay::TwoColour => ColourDisplay::TwoColour,
        BasicColourDisplay::Gradient => ColourDisplay::Gradient,
        BasicColourDisplay::Meter => ColourDisplay::Meter,
        BasicColourDisplay::GradientMeter => ColourDisplay::GradientMeter
    }
}

fn profile_to_standard_fader_display(value: ColourDisplay) -> BasicColourDisplay {
    match value {
        ColourDisplay::TwoColour => BasicColourDisplay::TwoColour,
        ColourDisplay::Gradient => BasicColourDisplay::Gradient,
        ColourDisplay::Meter => BasicColourDisplay::Meter,
        ColourDisplay::GradientMeter => BasicColourDisplay::Gradient
    }
}

fn standard_to_profile_colour_off_style(value: BasicColourOffStyle) -> ColourOffStyle {
    match value {
        BasicColourOffStyle::Dimmed => ColourOffStyle::Dimmed,
        BasicColourOffStyle::Colour2 => ColourOffStyle::Colour2,
        BasicColourOffStyle::DimmedColour2 => ColourOffStyle::DimmedColour2
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

fn profile_to_standard_sample_bank(bank: SampleBank) -> goxlr_types::SampleBank {
    match bank {
        SampleBank::A => goxlr_types::SampleBank::A,
        SampleBank::B => goxlr_types::SampleBank::B,
        SampleBank::C => goxlr_types::SampleBank::C
    }
}

fn standard_to_profile_sample_bank(bank: goxlr_types::SampleBank) -> SampleBank {
    match bank {
        goxlr_types::SampleBank::A => SampleBank::A,
        goxlr_types::SampleBank::B => SampleBank::B,
        goxlr_types::SampleBank::C => SampleBank::C
    }
}

fn sample_bank_to_simple_element(bank: SampleBank) -> SimpleElements {
    match bank {
        SampleBank::A => SimpleElements::SampleBankA,
        SampleBank::B => SimpleElements::SampleBankB,
        SampleBank::C => SimpleElements::SampleBankC
    }
}

fn profile_to_standard_preset(value: Preset) -> EffectBankPresets {
    match value {
        Preset::Preset1 => EffectBankPresets::Preset1,
        Preset::Preset2 => EffectBankPresets::Preset2,
        Preset::Preset3 => EffectBankPresets::Preset3,
        Preset::Preset4 => EffectBankPresets::Preset4,
        Preset::Preset5 => EffectBankPresets::Preset5,
        Preset::Preset6 => EffectBankPresets::Preset6
    }
}

fn standard_to_profile_preset(value: EffectBankPresets) -> Preset {
    match value {
        EffectBankPresets::Preset1 => Preset::Preset1,
        EffectBankPresets::Preset2 => Preset::Preset2,
        EffectBankPresets::Preset3 => Preset::Preset3,
        EffectBankPresets::Preset4 => Preset::Preset4,
        EffectBankPresets::Preset5 => Preset::Preset5,
        EffectBankPresets::Preset6 => Preset::Preset6
    }
}

fn get_colour_map_from_button(profile: &ProfileSettings, button: Buttons) -> &ColourMap {
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

fn get_profile_colour_map(profile: &ProfileSettings, colour_target: ColourTargets) -> &ColourMap {
    match colour_target {
        ColourTargets::Fader1Mute => profile.mute_button(0).colour_map(),
        ColourTargets::Fader2Mute => profile.mute_button(1).colour_map(),
        ColourTargets::Fader3Mute => profile.mute_button(2).colour_map(),
        ColourTargets::Fader4Mute => profile.mute_button(3).colour_map(),
        ColourTargets::Bleep => profile.simple_element(SimpleElements::Swear).colour_map(),
        ColourTargets::MicrophoneMute => profile.mute_chat().colour_map(),
        ColourTargets::EffectSelect1 => profile.effects(Preset::Preset1).colour_map(),
        ColourTargets::EffectSelect2 => profile.effects(Preset::Preset2).colour_map(),
        ColourTargets::EffectSelect3 => profile.effects(Preset::Preset3).colour_map(),
        ColourTargets::EffectSelect4 => profile.effects(Preset::Preset4).colour_map(),
        ColourTargets::EffectSelect5 => profile.effects(Preset::Preset5).colour_map(),
        ColourTargets::EffectSelect6 => profile.effects(Preset::Preset6).colour_map(),
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

fn get_profile_colour_map_mut(profile: &mut ProfileSettings, colour_target: ColourTargets) -> &mut ColourMap {
    match colour_target {
        ColourTargets::Fader1Mute => profile.mute_button_mut(0).colour_map_mut(),
        ColourTargets::Fader2Mute => profile.mute_button_mut(1).colour_map_mut(),
        ColourTargets::Fader3Mute => profile.mute_button_mut(2).colour_map_mut(),
        ColourTargets::Fader4Mute => profile.mute_button_mut(3).colour_map_mut(),
        ColourTargets::Bleep => profile.simple_element_mut(SimpleElements::Swear).colour_map_mut(),
        ColourTargets::MicrophoneMute => profile.mute_chat_mut().colour_map_mut(),
        ColourTargets::EffectSelect1 => profile.effects_mut(Preset::Preset1).colour_map_mut(),
        ColourTargets::EffectSelect2 => profile.effects_mut(Preset::Preset2).colour_map_mut(),
        ColourTargets::EffectSelect3 => profile.effects_mut(Preset::Preset3).colour_map_mut(),
        ColourTargets::EffectSelect4 => profile.effects_mut(Preset::Preset4).colour_map_mut(),
        ColourTargets::EffectSelect5 => profile.effects_mut(Preset::Preset5).colour_map_mut(),
        ColourTargets::EffectSelect6 => profile.effects_mut(Preset::Preset6).colour_map_mut(),
        ColourTargets::EffectFx => profile.simple_element_mut(SimpleElements::FxClear).colour_map_mut(),
        ColourTargets::EffectMegaphone => profile.megaphone_effect_mut().colour_map_mut(),
        ColourTargets::EffectRobot => profile.robot_effect_mut().colour_map_mut(),
        ColourTargets::EffectHardTune => profile.hardtune_effect_mut().colour_map_mut(),
        ColourTargets::SamplerSelectA => {
            profile.simple_element_mut(SimpleElements::SampleBankA).colour_map_mut()
        }
        ColourTargets::SamplerSelectB => {
            profile.simple_element_mut(SimpleElements::SampleBankB).colour_map_mut()
        }
        ColourTargets::SamplerSelectC => {
            profile.simple_element_mut(SimpleElements::SampleBankC).colour_map_mut()
        }
        ColourTargets::SamplerTopLeft => profile.sample_button_mut(TopLeft).colour_map_mut(),
        ColourTargets::SamplerTopRight => profile.sample_button_mut(TopRight).colour_map_mut(),
        ColourTargets::SamplerBottomLeft => profile.sample_button_mut(BottomLeft).colour_map_mut(),
        ColourTargets::SamplerBottomRight => profile.sample_button_mut(BottomRight).colour_map_mut(),
        ColourTargets::SamplerClear => profile.sample_button_mut(Clear).colour_map_mut(),
        ColourTargets::FadeMeter1 => profile.fader_mut(0).colour_map_mut(),
        ColourTargets::FadeMeter2 => profile.fader_mut(1).colour_map_mut(),
        ColourTargets::FadeMeter3 => profile.fader_mut(2).colour_map_mut(),
        ColourTargets::FadeMeter4 => profile.fader_mut(3).colour_map_mut(),
        ColourTargets::Scribble1 => profile.scribble_mut(0).colour_map_mut(),
        ColourTargets::Scribble2 => profile.scribble_mut(1).colour_map_mut(),
        ColourTargets::Scribble3 => profile.scribble_mut(2).colour_map_mut(),
        ColourTargets::Scribble4 => profile.scribble_mut(3).colour_map_mut(),
        ColourTargets::PitchEncoder => profile.pitch_encoder_mut().colour_map_mut(),
        ColourTargets::GenderEncoder => profile.gender_encoder_mut().colour_map_mut(),
        ColourTargets::ReverbEncoder => profile.reverb_encoder_mut().colour_map_mut(),
        ColourTargets::EchoEncoder => profile.echo_encoder_mut().colour_map_mut(),
        ColourTargets::LogoX => profile.simple_element_mut(SimpleElements::LogoX).colour_map_mut(),
        ColourTargets::Global => profile.simple_element_mut(SimpleElements::GlobalColour).colour_map_mut(),
    }
}

pub fn standard_to_colour_target(target: ButtonColourTargets) -> ColourTargets {
    match target {
        ButtonColourTargets::Fader1Mute => ColourTargets::Fader1Mute,
        ButtonColourTargets::Fader2Mute => ColourTargets::Fader2Mute,
        ButtonColourTargets::Fader3Mute => ColourTargets::Fader3Mute,
        ButtonColourTargets::Fader4Mute => ColourTargets::Fader4Mute,
        ButtonColourTargets::Bleep => ColourTargets::Bleep,
        ButtonColourTargets::Cough => ColourTargets::MicrophoneMute,
        ButtonColourTargets::EffectSelect1 => ColourTargets::EffectSelect1,
        ButtonColourTargets::EffectSelect2 => ColourTargets::EffectSelect2,
        ButtonColourTargets::EffectSelect3 => ColourTargets::EffectSelect3,
        ButtonColourTargets::EffectSelect4 => ColourTargets::EffectSelect4,
        ButtonColourTargets::EffectSelect5 => ColourTargets::EffectSelect5,
        ButtonColourTargets::EffectSelect6 => ColourTargets::EffectSelect6,
        ButtonColourTargets::EffectFx => ColourTargets::EffectFx,
        ButtonColourTargets::EffectMegaphone => ColourTargets::EffectMegaphone,
        ButtonColourTargets::EffectRobot => ColourTargets::EffectRobot,
        ButtonColourTargets::EffectHardTune => ColourTargets::EffectHardTune,
        ButtonColourTargets::SamplerSelectA => ColourTargets::SamplerSelectA,
        ButtonColourTargets::SamplerSelectB => ColourTargets::SamplerSelectB,
        ButtonColourTargets::SamplerSelectC => ColourTargets::SamplerSelectC,
        ButtonColourTargets::SamplerTopLeft => ColourTargets::SamplerTopLeft,
        ButtonColourTargets::SamplerTopRight => ColourTargets::SamplerTopRight,
        ButtonColourTargets::SamplerBottomLeft => ColourTargets::SamplerBottomLeft,
        ButtonColourTargets::SamplerBottomRight => ColourTargets::SamplerBottomRight,
        ButtonColourTargets::SamplerClear => ColourTargets::SamplerClear
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
