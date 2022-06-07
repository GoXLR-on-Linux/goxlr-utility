use std::collections::HashSet;
use anyhow::{anyhow, Context, Result};
use enumset::EnumSet;
use goxlr_profile_loader::components::colours::{Colour, ColourDisplay, ColourMap, ColourOffStyle, ColourState};
use goxlr_profile_loader::components::colours::ColourOffStyle::Dimmed;
use goxlr_profile_loader::components::mixer::{FullChannelList, InputChannels, OutputChannels};
use goxlr_profile_loader::mic_profile::MicProfileSettings;
use goxlr_profile_loader::profile::{Profile, ProfileSettings};
use goxlr_profile_loader::SampleButtons::{BottomLeft, BottomRight, Clear, TopLeft, TopRight};
use goxlr_types::{ChannelName, FaderName, InputDevice, MicrophoneType, OutputDevice, VersionNumber, MuteFunction as BasicMuteFunction, ColourDisplay as BasicColourDisplay, ColourOffStyle as BasicColourOffStyle, EffectBankPresets, MicrophoneParamKey, EffectKey};
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
use serde::de::IntoDeserializer;
use goxlr_ipc::{Compressor, Equaliser, EqualiserFrequency, EqualiserGain, EqualiserMini, EqualiserMiniFrequency, EqualiserMiniGain, NoiseGate};
use goxlr_profile_loader::components::echo::EchoEncoder;
use goxlr_profile_loader::components::gender::GenderEncoder;
use goxlr_profile_loader::components::hardtune::HardtuneEffect;
use goxlr_profile_loader::components::megaphone::{MegaphoneEffect, Preset};
use goxlr_profile_loader::components::mute::{MuteButton, MuteFunction};
use goxlr_profile_loader::components::mute_chat::MuteChat;
use goxlr_profile_loader::components::pitch::{PitchEncoder, PitchEncoderBase, PitchStyle};
use goxlr_profile_loader::components::reverb::ReverbEncoder;
use goxlr_profile_loader::components::robot::RobotEffect;
use goxlr_profile_loader::components::simple::SimpleElements;
use goxlr_usb::buttonstate::{Buttons, ButtonStates};
use crate::SettingsHandle;

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

    pub fn set_mute_button_off_style(&mut self, fader: FaderName, off_style: BasicColourOffStyle) {
        self.get_mute_button_mut(fader).colour_map_mut().set_off_style(
            standard_to_profile_colour_off_style(off_style)
        );
    }

    // TODO: This should (and hopefully will) be *FAR* more generic!
    pub fn set_mute_button_colours(&mut self, fader: FaderName, colour_one: String, colour_two: Option<String>) -> Result<()> {
        let colours = self.get_mute_button_mut(fader).colour_map_mut();
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

    pub fn set_mute_chat_off_style(&mut self, off_style: BasicColourOffStyle) {
        self.profile.settings_mut().mute_chat_mut().colour_map_mut().set_off_style(
            standard_to_profile_colour_off_style(off_style)
        );
    }

    // TODO: This should (and hopefully will) be *FAR* more generic!
    pub fn set_mute_chat_colours(&mut self, colour_one: String, colour_two: Option<String>) -> Result<()> {
        let colours = self.profile.settings_mut().mute_chat_mut().colour_map_mut();
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



    /** Bleep Button **/
    pub fn set_swear_off_style(&mut self, off_style: BasicColourOffStyle) {
        self.profile.settings_mut().simple_element_mut(SimpleElements::Swear).colour_map_mut().set_off_style(
            standard_to_profile_colour_off_style(off_style)
        );
    }

    // TODO: This should (and hopefully will) be *FAR* more generic!
    pub fn set_swear_colours(&mut self, colour_one: String, colour_two: Option<String>) -> Result<()> {
        let colours = self.profile.settings_mut().simple_element_mut(SimpleElements::Swear).colour_map_mut();
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
            attack: self.profile.gate().attack(),
            release: self.profile.gate().release(),
            enabled: self.profile.gate().enabled(),
            attenuation: self.profile.gate().attenuation()
        }
    }

    pub fn compressor_ipc(&self) -> Compressor {
        Compressor {
            threshold: self.profile.compressor().threshold(),
            ratio: self.profile.compressor().ratio(),
            attack: self.profile.compressor().attack(),
            release: self.profile.compressor().release(),
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
            MicrophoneParamKey::GateAttenuation => self.i8_to_f32(self.profile.gate().attenuation()),
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
            EffectKey::DisableMic => 0,
            EffectKey::BleepLevel => block_on(settings.get_device_bleep_volume(serial)).unwrap_or(-20).into(),
            EffectKey::GateMode => 2,   // Not a profile setting, hard coded in Windows
            EffectKey::GateEnabled => 1,    // Used for 'Mic Testing' in the UI
            EffectKey::GateThreshold => self.profile.gate().threshold().into(),
            EffectKey::GateAttenuation => self.profile.gate().attenuation().into(),
            EffectKey::GateAttack => self.profile.gate().attack().into(),
            EffectKey::GateRelease => self.profile.gate().release().into(),
            EffectKey::Unknown14b => 0,

            // For Frequencies, we need to accurately reverse the profile -> value settings, until
            // then, we'll hard code them.
            EffectKey::Equalizer31HzFrequency => 15,
            EffectKey::Equalizer63HzFrequency => 40,
            EffectKey::Equalizer125HzFrequency => 63,
            EffectKey::Equalizer250HzFrequency => 87,
            EffectKey::Equalizer500HzFrequency => 111,
            EffectKey::Equalizer1KHzFrequency => 135,
            EffectKey::Equalizer2KHzFrequency => 159,
            EffectKey::Equalizer4KHzFrequency => 183,
            EffectKey::Equalizer8KHzFrequency => 207,
            EffectKey::Equalizer16KHzFrequency => 231,
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

            // TODO: Verify PitchCharacter key and how it works..
            EffectKey::PitchAmount => main_profile.get_active_pitch_profile().knob_position().into(),
            EffectKey::PitchStyle => *main_profile.get_active_pitch_profile().style() as i32,
            EffectKey::PitchCharacter => 0, // TODO: Might have different flags depending on Style?

            // TODO: Gender Style is Missing?
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
        // keys.insert(EffectKey::Equalizer31HzGain);
        // keys.insert(EffectKey::Equalizer63HzGain);
        // keys.insert(EffectKey::Equalizer125HzGain);
        // keys.insert(EffectKey::Equalizer250HzGain);
        // keys.insert(EffectKey::Equalizer500HzGain);
        // keys.insert(EffectKey::Equalizer1KHzGain);
        // keys.insert(EffectKey::Equalizer2KHzGain);
        // keys.insert(EffectKey::Equalizer4KHzGain);
        // keys.insert(EffectKey::Equalizer8KHzGain);
        // keys.insert(EffectKey::Equalizer16KHzGain);
        //
        // keys.insert(EffectKey::Equalizer31HzFrequency);
        // keys.insert(EffectKey::Equalizer63HzFrequency);
        // keys.insert(EffectKey::Equalizer125HzFrequency);
        // keys.insert(EffectKey::Equalizer250HzFrequency);
        // keys.insert(EffectKey::Equalizer500HzFrequency);
        // keys.insert(EffectKey::Equalizer1KHzFrequency);
        // keys.insert(EffectKey::Equalizer2KHzFrequency);
        // keys.insert(EffectKey::Equalizer4KHzFrequency);
        // keys.insert(EffectKey::Equalizer8KHzFrequency);
        // keys.insert(EffectKey::Equalizer16KHzFrequency);


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
        set.insert(EffectKey::PitchStyle);
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
