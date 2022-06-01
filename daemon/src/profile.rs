use anyhow::{anyhow, Context, Result};
use enumset::EnumSet;
use goxlr_profile_loader::components::colours::{Colour, ColourDisplay, ColourMap, ColourOffStyle, ColourState};
use goxlr_profile_loader::components::colours::ColourOffStyle::Dimmed;
use goxlr_profile_loader::components::mixer::{FullChannelList, InputChannels, OutputChannels};
use goxlr_profile_loader::mic_profile::MicProfileSettings;
use goxlr_profile_loader::profile::{Profile, ProfileSettings};
use goxlr_profile_loader::SampleButtons::{BottomLeft, BottomRight, Clear, TopLeft, TopRight};
use goxlr_types::{ChannelName, FaderName, InputDevice, MicrophoneType, OutputDevice, VersionNumber, MuteFunction as BasicMuteFunction, ColourDisplay as BasicColourDisplay, ColourOffStyle as BasicColourOffStyle, EffectBankPresets};
use goxlr_usb::colouring::ColourTargets;
use log::error;
use std::fs::{create_dir_all, File};
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use strum::EnumCount;
use strum::IntoEnumIterator;
use byteorder::{ByteOrder, LittleEndian};
use enum_map::EnumMap;
use goxlr_profile_loader::components::megaphone::Preset;
use goxlr_profile_loader::components::mute::{MuteButton, MuteFunction};
use goxlr_profile_loader::components::mute_chat::MuteChat;
use goxlr_profile_loader::components::pitch::PitchStyle;
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

    pub fn get_gender_value(&self) -> i8 {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().gender_encoder().get_preset(current).knob_position()
    }

    pub fn set_gender_value(&mut self, value: i8) {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings_mut().gender_encoder_mut().get_preset_mut(current).set_knob_position(value)
    }

    pub fn get_reverb_value(&self) -> i8 {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().reverb_encoder().get_preset(current).knob_position()
    }

    pub fn set_reverb_value(&mut self, value: i8) {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings_mut().reverb_encoder_mut().get_preset_mut(current).set_knob_position(value)
    }

    pub fn get_echo_value(&self) -> i8 {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().echo_encoder().get_preset(current).knob_position()
    }

    pub fn set_echo_value(&mut self, value: i8) {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings_mut().echo_encoder_mut().get_preset_mut(current).set_knob_position(value)
    }

    pub fn is_hardtune_enabled(&self) -> bool {
        self.profile.settings().hardtune_effect().colour_map().get_state()
    }

    pub fn is_pitch_narrow(&self) -> bool {
        let current = self.profile.settings().context().selected_effects();
        self.profile.settings().pitch_encoder().get_preset(current).style() == &PitchStyle::Narrow
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
