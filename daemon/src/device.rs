use crate::profile::{version_newer_or_equal_to, MicProfileAdapter, ProfileAdapter};
use crate::SettingsHandle;
use anyhow::Result;
use enumset::EnumSet;
use goxlr_ipc::{DeviceType, GoXLRCommand, HardwareStatus, MixerStatus};
use goxlr_types::{ChannelName, EffectKey, FaderName, InputDevice as BasicInputDevice, MicrophoneParamKey, MicrophoneType, OutputDevice as BasicOutputDevice, VersionNumber};
use goxlr_usb::buttonstate::{ButtonStates, Buttons};
use goxlr_usb::channelstate::ChannelState;
use goxlr_usb::goxlr::GoXLR;
use goxlr_usb::routing::{InputDevice, OutputDevice};
use goxlr_usb::rusb::UsbContext;
use log::debug;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use enum_map::EnumMap;
use strum::{EnumCount, IntoEnumIterator};
use goxlr_profile_loader::components::colours::ColourState;
use goxlr_profile_loader::components::mute::{MuteButton, MuteFunction};
use goxlr_usb::channelstate::ChannelState::{Muted, Unmuted};

#[derive(Debug)]
pub struct Device<T: UsbContext> {
    goxlr: GoXLR<T>,
    status: MixerStatus,
    last_buttons: EnumSet<Buttons>,
    button_states: EnumMap<Buttons, ButtonState>,
    profile: ProfileAdapter,
    mic_profile: MicProfileAdapter,
}

// Experimental code:
#[derive(Debug, Default, Copy, Clone)]
struct ButtonState {
    press_time: u128,
    hold_handled: bool
}

impl<T: UsbContext> Device<T> {
    pub fn new(
        goxlr: GoXLR<T>,
        hardware: HardwareStatus,
        profile_name: Option<String>,
        mic_profile_name: Option<String>,
        profile_directory: &Path,
    ) -> Result<Self> {
        let profile = ProfileAdapter::from_named_or_default(profile_name, profile_directory);
        let mic_profile =
            MicProfileAdapter::from_named_or_default(mic_profile_name, profile_directory);

        let status = MixerStatus {
            hardware,
            fader_a_assignment: ChannelName::Chat,
            fader_b_assignment: ChannelName::Chat,
            fader_c_assignment: ChannelName::Chat,
            fader_d_assignment: ChannelName::Chat,
            volumes: [255; ChannelName::COUNT],
            mic_gains: [0; MicrophoneType::COUNT],
            mic_type: MicrophoneType::Jack,
            router: Default::default(),
            profile_name: profile.name().to_owned(),
            mic_profile_name: mic_profile.name().to_owned(),
        };

        let mut device = Self {
            profile,
            mic_profile,
            goxlr,
            status,
            last_buttons: EnumSet::empty(),
            button_states: EnumMap::default(),
        };

        device.apply_profile()?;
        device.apply_mic_profile()?;

        Ok(device)
    }

    pub fn serial(&self) -> &str {
        &self.status.hardware.serial_number
    }

    pub fn status(&self) -> &MixerStatus {
        &self.status
    }

    pub fn profile(&self) -> &ProfileAdapter {
        &self.profile
    }

    pub fn mic_profile(&self) -> &MicProfileAdapter {
        &self.mic_profile
    }

    pub async fn monitor_inputs(&mut self, settings: &SettingsHandle) -> Result<()> {
        self.status.hardware.usb_device.has_kernel_driver_attached =
            self.goxlr.usb_device_has_kernel_driver_active()?;

        if let Ok((buttons, volumes)) = self.goxlr.get_button_states() {
            self.update_volumes_to(volumes);

            let pressed_buttons = buttons.difference(self.last_buttons);
            for button in pressed_buttons {
                // This is a new press, store it in the states..
                self.button_states[button] = ButtonState {
                    press_time: self.get_epoch_ms(),
                    hold_handled: false
                };

                self.on_button_down(button, settings).await?;
            }

            let released_buttons = self.last_buttons.difference(buttons);
            for button in released_buttons {
                let button_state = self.button_states[button];
                self.on_button_up(button, &button_state, settings).await?;

                self.button_states[button] = ButtonState {
                    press_time: 0,
                    hold_handled: false
                }
            }

            // Finally, iterate over our existing button states, and see if any have been
            // pressed for more than half a second and not handled.
            for button in buttons {
                if !self.button_states[button].hold_handled {
                    let now = self.get_epoch_ms();
                    if (now - self.button_states[button].press_time) > 500 {
                        self.on_button_hold(button, settings).await?;
                        self.button_states[button].hold_handled = true;
                    }
                }
            }

            self.last_buttons = buttons;
        }

        Ok(())
    }

    async fn on_button_down(&mut self, button: Buttons, _settings: &SettingsHandle) -> Result<()> {
        debug!("Handling Button Down: {:?}", button);

        match button {
            Buttons::MicrophoneMute => {
                self.handle_cough_mute(true, false, false, false).await?;
            }
            _ => {}
        }
        self.update_button_states()?;
        Ok(())
    }

    async fn on_button_hold(&mut self, button: Buttons, _settings: &SettingsHandle) -> Result<()> {
        debug!("Handling Button Hold: {:?}", button);
        match button {
            Buttons::Fader1Mute => {
                self.handle_fader_mute(FaderName::A, true).await?;
            }
            Buttons::Fader2Mute => {
                self.handle_fader_mute(FaderName::B, true).await?;
            }
            Buttons::Fader3Mute => {
                self.handle_fader_mute(FaderName::C, true).await?;
            }
            Buttons::Fader4Mute => {
                self.handle_fader_mute(FaderName::D, true).await?;
            }
            Buttons::MicrophoneMute => {
                self.handle_cough_mute(false, false, true, false).await?;
            }
            _ => {}
        }
        self.update_button_states()?;
        Ok(())
    }

    async fn on_button_up(&mut self, button: Buttons, state: &ButtonState, _settings: &SettingsHandle) -> Result<()> {
        debug!("Handling Button Release: {:?}, Has Long Press Handled: {:?}", button, state.hold_handled);
        match button {
            Buttons::Fader1Mute => {
                if !state.hold_handled {
                    self.handle_fader_mute(FaderName::A, false).await?;
                }
            }
            Buttons::Fader2Mute => {
                if !state.hold_handled {
                    self.handle_fader_mute(FaderName::B, false).await?;
                }
            }
            Buttons::Fader3Mute => {
                if !state.hold_handled {
                    self.handle_fader_mute(FaderName::C, false).await?;
                }
            }
            Buttons::Fader4Mute => {
                if !state.hold_handled {
                    self.handle_fader_mute(FaderName::D, false).await?;
                }
            }
            Buttons::MicrophoneMute => {
                self.handle_cough_mute(
                    false,
                    true,
                    false,
                    state.hold_handled).await?;
            }
            _ => {}
        }
        self.update_button_states()?;
        Ok(())
    }

    async fn handle_fader_mute(
        &mut self,
        fader: FaderName,
        held: bool,
    ) -> Result<()> {
        // OK, so a fader button has been pressed, we need to determine behaviour, based on the colour map..
        let channel = self.profile.get_fader_assignment(fader);
        let current_volume = self.profile.get_channel_volume(channel);

        let mute_config: &mut MuteButton = self.profile.get_mute_button(fader);
        let colour_map = mute_config.colour_map();

        // We should be safe to straight unwrap these, state and blink are always present.
        let muted_to_x = colour_map.state().as_ref().unwrap() == &ColourState::On;
        let muted_to_all = colour_map.blink().as_ref().unwrap() == &ColourState::On;
        let mute_function = mute_config.mute_function().clone();

        // Map the channel to BasicInputDevice in case we need it later..
        let basic_input = match channel {
            ChannelName::Mic => Some(BasicInputDevice::Microphone),
            ChannelName::LineIn => Some(BasicInputDevice::LineIn),
            ChannelName::Console => Some(BasicInputDevice::Console),
            ChannelName::System => Some(BasicInputDevice::System),
            ChannelName::Game => Some(BasicInputDevice::Game),
            ChannelName::Chat => Some(BasicInputDevice::Chat),
            ChannelName::Sample => Some(BasicInputDevice::Samples),
            ChannelName::Music => Some(BasicInputDevice::Music),
            _ => None,
        };

        // Should we be muting this fader to all channels?
        if held || (!muted_to_x && mute_function == MuteFunction::All) {
            if held && muted_to_all {
                // Holding the button when it's already muted to all does nothing.
                return Ok(());
            }

            mute_config.set_previous_volume(current_volume);

            self.goxlr.set_volume(channel, 0)?;
            self.goxlr.set_channel_state(channel, Muted)?;

            mute_config.colour_map().set_state(Some(ColourState::On));
            if held {
                mute_config.colour_map().set_blink(Some(ColourState::On));
            }

            self.profile.set_channel_volume(channel, 0);
            self.status.set_channel_volume(channel, 0);

            return Ok(());
        }

        // Button has been pressed, and we're already in some kind of muted state..
        if !held && muted_to_x {
            // Disable the lighting regardless of action
            mute_config.colour_map().set_state(Some(ColourState::Off));
            mute_config.colour_map().set_blink(Some(ColourState::Off));

            if muted_to_all || mute_function == MuteFunction::All {
                let previous_volume = mute_config.previous_volume();

                self.goxlr.set_volume(channel, mute_config.previous_volume())?;
                self.profile.set_channel_volume(channel, previous_volume);
                self.status.set_channel_volume(channel, previous_volume);

                if channel != ChannelName::Mic || (channel == ChannelName::Mic && !self.mic_muted_by_cough()) {
                    self.goxlr.set_channel_state(channel, ChannelState::Unmuted)?;
                }
            } else {
                if basic_input.is_some() {
                    self.apply_routing(basic_input.unwrap())?;
                }
            }

            return Ok(());
        }

        if !held && !muted_to_x && mute_function != MuteFunction::All {
            // Mute channel to X via transient routing table update
            mute_config.colour_map().set_state(Some(ColourState::On));
            if basic_input.is_some() {
                self.apply_routing(basic_input.unwrap())?;
            }
        }
        Ok(())
    }

    // This one's a little obnoxious because it's heavily settings dependent, so will contain a
    // large volume of comments working through states, feel free to remove them later :)
    async fn handle_cough_mute(&mut self, press: bool, release: bool, held: bool, held_called: bool) -> Result<()> {
        // This *GENERALLY* works in the same way as other mute buttons, however we need to
        // accommodate the hold and toggle behaviours, so lets grab the config.
        let mute_config = self.profile.get_mute_chat();

        // Identical behaviour, different variable locations..
        let mute_toggle = mute_config.is_cough_toggle();
        let muted_to_x = mute_config.cough_button_on();
        let muted_to_all = mute_config.blink() == &ColourState::On;
        let mute_function = mute_config.cough_mute_source().clone();

        // Ok, lets handle things in order, was this button just pressed?
        if press {
            if mute_toggle {
                // Mute toggles are only handled on release.
                return Ok(());
            }

            // Enable the cough button in all cases..
            mute_config.set_cough_button_on(true);
            if mute_function == MuteFunction::All {
                // In this scenario, we should just set cough_button_on and mute the channel.
                self.goxlr.set_channel_state(ChannelName::Mic, Muted)?;
                return Ok(());
            }

            self.apply_routing(BasicInputDevice::Microphone)?;
            return Ok(());
        }

        if held {
            if !mute_toggle {
                // Holding in this scenario just keeps the channel muted, so no change here.
                return Ok(())
            }

            // We're togglable, so enable blink, set cough_button_on, mute the channel fully and
            // remove any transient routing which may be set.
            mute_config.set_cough_button_on(true);
            mute_config.set_blink(ColourState::On);
            self.goxlr.set_channel_state(ChannelName::Mic, Muted)?;
            self.apply_routing(BasicInputDevice::Microphone)?;
            return Ok(())
        }

        if release {
            if mute_toggle {
                 if held_called {
                     // We don't need to do anything here, a long press has already been handled.
                     return Ok(())
                 }

                if muted_to_x || muted_to_all {
                    mute_config.set_cough_button_on(false);
                    mute_config.set_blink(ColourState::Off);

                    if muted_to_all || (muted_to_x && mute_function == MuteFunction::All) {
                        if !self.mic_muted_by_fader() {
                            self.goxlr.set_channel_state(ChannelName::Mic, Unmuted)?;
                        }
                    }

                    if muted_to_x && mute_function != MuteFunction::All {
                        self.apply_routing(BasicInputDevice::Microphone)?;
                    }

                    return Ok(())
                }

                // In all cases, enable the button
                mute_config.set_cough_button_on(true);

                if mute_function == MuteFunction::All {
                    self.goxlr.set_channel_state(ChannelName::Mic, Muted)?;
                    return Ok(())
                }

                // Update the transient routing..
                self.apply_routing(BasicInputDevice::Microphone)?;
                return Ok(())
            }

            mute_config.set_cough_button_on(false);
            if mute_function == MuteFunction::All {
                if !self.mic_muted_by_fader() {
                    self.goxlr.set_channel_state(ChannelName::Chat, Unmuted)?;
                }
                return Ok(())
            }

            // Disable button and refresh transient routing
            self.apply_routing(BasicInputDevice::Microphone)?;
            return Ok(())
        }

        Ok(())
    }

    fn mic_muted_by_fader(&mut self) -> bool {
        // Is the mute button even assigned to a fader?
        let mic_fader_id = self.profile.get_mute_chat().mic_fader_id();

        if mic_fader_id == 4 {
            return false;
        }

        let mute_config = self.profile.get_mute_button_by_id(mic_fader_id);
        let colour_map = mute_config.colour_map();

        // We should be safe to straight unwrap these, state and blink are always present.
        let muted_to_x = colour_map.state().as_ref().unwrap() == &ColourState::On;
        let muted_to_all = colour_map.blink().as_ref().unwrap() == &ColourState::On;
        let mute_function = mute_config.mute_function().clone();

        return muted_to_all || (muted_to_x && mute_function == MuteFunction::All);
    }

    fn mic_muted_by_cough(&mut self) -> bool {
        let mute_config = self.profile.get_mute_chat();

        let muted_to_x = mute_config.cough_button_on();
        let muted_to_all = mute_config.blink() == &ColourState::On;
        let mute_function = mute_config.cough_mute_source().clone();

        return muted_to_all || (muted_to_x && mute_function == MuteFunction::All);
    }

    fn update_volumes_to(&mut self, volumes: [u8; 4]) {
        for fader in FaderName::iter() {
            let channel = self.profile.get_fader_assignment(fader);
            let old_volume = self.profile.get_channel_volume(channel);

            let new_volume = volumes[fader as usize];
            if new_volume != old_volume {
                debug!(
                    "Updating {} volume from {} to {} as a human moved the fader",
                    channel, old_volume, new_volume
                );
                self.profile.set_channel_volume(channel, new_volume);
            }
        }
    }

    pub async fn perform_command(
        &mut self,
        command: GoXLRCommand,
        settings: &SettingsHandle,
    ) -> Result<()> {
        match command {
            GoXLRCommand::AssignFader(fader, channel) => {
                self.set_fader(fader, channel).await?;
            }
            GoXLRCommand::SetVolume(channel, volume) => {
                self.profile.set_channel_volume(channel, volume);
                self.status.set_channel_volume(channel, volume);
                self.goxlr.set_volume(channel, volume)?;
            }
            GoXLRCommand::SetMicrophoneGain(mic_type, gain) => {
                self.goxlr.set_microphone_gain(mic_type, gain.into())?;
                self.mic_profile.set_mic_type(mic_type);
                self.mic_profile.set_mic_gain(mic_type, gain);

                // Sync with Status..
                self.status.mic_type = mic_type;
                self.status.mic_gains[mic_type as usize] = gain;
            }
            GoXLRCommand::LoadProfile(profile_name) => {
                let profile_directory = settings.get_profile_directory().await;
                self.profile = ProfileAdapter::from_named(profile_name, &profile_directory)?;
                self.apply_profile()?;
                settings
                    .set_device_profile_name(self.serial(), self.profile.name())
                    .await;
                settings.save().await;
            }
            GoXLRCommand::LoadMicProfile(mic_profile_name) => {
                let profile_directory = settings.get_profile_directory().await;
                self.mic_profile =
                    MicProfileAdapter::from_named(mic_profile_name, &profile_directory)?;
                self.apply_mic_profile()?;
                settings
                    .set_device_mic_profile_name(self.serial(), self.mic_profile.name())
                    .await;
                settings.save().await;
            }
        }

        Ok(())
    }

    fn update_button_states(&mut self) -> Result<()> {
        let button_states = self.create_button_states();
        self.goxlr.set_button_states(button_states)?;
        Ok(())
    }

    fn create_button_states(&mut self) -> [ButtonStates; 24] {
        let mut result = [ButtonStates::DimmedColour1; 24];

        result[Buttons::Fader1Mute as usize] = self.get_fader_mute_button_state(FaderName::A);
        result[Buttons::Fader2Mute as usize] = self.get_fader_mute_button_state(FaderName::B);
        result[Buttons::Fader3Mute as usize] = self.get_fader_mute_button_state(FaderName::C);
        result[Buttons::Fader4Mute as usize] = self.get_fader_mute_button_state(FaderName::D);

        result[Buttons::MicrophoneMute as usize] = self.get_cough_button_state();
        result
    }

    fn get_fader_mute_button_state(&mut self, fader: FaderName) -> ButtonStates {
        // TODO: Potentially abstract this out, most buttons behave the same.
        let mute_config = self.profile.get_mute_button(fader);
        let colour_map = mute_config.colour_map();

        if colour_map.blink().as_ref().unwrap() == &ColourState::On {
            return ButtonStates::Flashing;
        }

        if colour_map.state().as_ref().unwrap() == &ColourState::On {
            return ButtonStates::Colour1;
        }

        return ButtonStates::DimmedColour1;
    }

    // Slightly obnoxious, the variables for this come from the MuteChat object, not the ColourMap!
    fn get_cough_button_state(&mut self) -> ButtonStates {
        let mute_config = self.profile.get_mute_chat();

        if mute_config.blink() == &ColourState::On {
            return ButtonStates::Flashing;
        }

        if mute_config.cough_button_on() {
            return ButtonStates::Colour1;
        }

        return ButtonStates::DimmedColour1;
    }


    // This applies routing for a single input channel..
    fn apply_channel_routing(&mut self, input: BasicInputDevice, router: EnumMap<BasicOutputDevice, bool>) -> Result<()> {
        let (left_input, right_input) = InputDevice::from_basic(&input);
        let mut left = [0; 22];
        let mut right = [0; 22];

        for output in BasicOutputDevice::iter() {
            if router[output] {
                let (left_output, right_output) = OutputDevice::from_basic(&output);

                left[left_output.position()] = 0x20;
                right[right_output.position()] = 0x20;
            }
        }
        self.goxlr.set_routing(left_input, left)?;
        self.goxlr.set_routing(right_input, right)?;

        Ok(())
    }

    fn apply_transient_routing(&mut self, input: BasicInputDevice, mut router: EnumMap<BasicOutputDevice, bool>) {
        // Not all channels are routable, so map the inputs to channels before checking..
        let channel_name = match input {
            BasicInputDevice::Microphone => ChannelName::Mic,
            BasicInputDevice::Chat => ChannelName::Chat,
            BasicInputDevice::Music => ChannelName::Music,
            BasicInputDevice::Game => ChannelName::Game,
            BasicInputDevice::Console => ChannelName::Console,
            BasicInputDevice::LineIn => ChannelName::LineIn,
            BasicInputDevice::System => ChannelName::System,
            BasicInputDevice::Samples => ChannelName::Sample
        };

        for fader in FaderName::iter() {
            if self.profile.get_fader_assignment(fader) == channel_name {
                self.apply_transient_fader_routing(fader, &mut router);
            }
        }
        self.apply_transient_cough_routing(&mut router);
    }

    fn apply_transient_fader_routing(&mut self, fader: FaderName, router: &mut EnumMap<BasicOutputDevice, bool>) {
        // We need to check the state of this, so pull the relevant parts..
        let mute_config: &mut MuteButton = self.profile.get_mute_button(fader);
        let colour_map = mute_config.colour_map();

        let muted_to_x = colour_map.state().as_ref().unwrap() == &ColourState::On;
        let muted_to_all = colour_map.blink().as_ref().unwrap() == &ColourState::On;
        let mute_function = mute_config.mute_function().clone();

        self.apply_transient_channel_routing(muted_to_x, muted_to_all, mute_function, router);
    }

    fn apply_transient_cough_routing(&mut self, router: &mut EnumMap<BasicOutputDevice, bool>) {
        // Same deal, pull out the current state, make needed changes.
        let mute_config = self.profile.get_mute_chat();

        let muted_to_x = mute_config.cough_button_on();
        let muted_to_all = mute_config.blink() == &ColourState::On;
        let mute_function = mute_config.cough_mute_source().clone();

        self.apply_transient_channel_routing(muted_to_x, muted_to_all, mute_function, router);
    }

    fn apply_transient_channel_routing(&mut self, muted_to_x: bool, muted_to_all: bool, mute_function: MuteFunction, router: &mut EnumMap<BasicOutputDevice, bool>) {
        if !muted_to_x || muted_to_all || mute_function == MuteFunction::All {
            return;
        }

        match mute_function {
            MuteFunction::All => {}
            MuteFunction::ToStream => router[BasicOutputDevice::BroadcastMix] = false,
            MuteFunction::ToVoiceChat => router[BasicOutputDevice::ChatMic] = false,
            MuteFunction::ToPhones => router[BasicOutputDevice::Headphones] = false,
            MuteFunction::ToLineOut => router[BasicOutputDevice::LineOut] = false
        }
    }


    fn apply_routing(&mut self, input: BasicInputDevice) -> Result<()> {
        // Load the routing for this channel from the profile..
        let router = self.profile.get_router(input);
        self.apply_transient_routing(input, router);

        debug!("Applying Routing to {:?}:", input);
        debug!("{:?}", router);

        self.apply_channel_routing(input, router)?;

        Ok(())
    }

    fn apply_mute_from_profile(&mut self, fader: FaderName) -> Result<()> {
        // Basically stripped down behaviour from handle_fader_mute which simply applies stuff.
        let channel = self.profile.get_fader_assignment(fader);

        let mute_config: &mut MuteButton = self.profile.get_mute_button(fader);
        let colour_map = mute_config.colour_map();

        let muted_to_all = colour_map.blink().as_ref().unwrap() == &ColourState::On;
        let muted_to_x = colour_map.state().as_ref().unwrap() == &ColourState::On;
        let mute_function = mute_config.mute_function().clone();

        if muted_to_all || (muted_to_x && mute_function == MuteFunction::All) {
            // This channel should be fully muted
            self.goxlr.set_channel_state(channel, Muted)?;
        }

        // This channel isn't supposed to be muted (The Router will handle anything else).
        self.goxlr.set_channel_state(channel, Unmuted)?;
        Ok(())
    }

    fn apply_cough_from_profile(&mut self) -> Result<()> {
        // As above, but applies the cough profile.
        let mute_config = self.profile.get_mute_chat();

        let mute_toggle = mute_config.is_cough_toggle();
        let muted_to_x = mute_config.cough_button_on();
        let muted_to_all = mute_config.blink() == &ColourState::On;
        let mute_function = mute_config.cough_mute_source().clone();

        // Firstly, if toggle is to hold and anything is muted, clear it.
        if !mute_toggle && muted_to_x {
            mute_config.set_cough_button_on(false);
            mute_config.set_blink(ColourState::Off);
            return Ok(())
        }

        if muted_to_all || (muted_to_x && mute_function == MuteFunction::All) {
           self.goxlr.set_channel_state(ChannelName::Mic, Muted)?;
        }
        Ok(())
    }

    async fn set_fader(&mut self, fader: FaderName, new_channel: ChannelName) -> Result<()> {
        // A couple of things need to happen when a fader change occurs depending on scenario..
        if new_channel == self.profile.get_fader_assignment(fader) {
            // We don't need to do anything at all in theory, set the fader anyway..
            if new_channel == ChannelName::Mic {
                self.profile.get_mute_chat().set_mic_fader_id(fader as u8);
            }

            self.goxlr.set_fader(fader, new_channel)?;
            self.status.set_fader_assignment(fader, new_channel);
            return Ok(());
        }

        // Firstly, get the state and settings of the fader..
        let existing_channel = self.profile.get_fader_assignment(fader);

        // Go over the faders, see if the new channel is already bound..
        let mut fader_to_switch: Option<FaderName> = None;
        for fader_name in FaderName::iter() {
            if fader_name != fader && self.profile.get_fader_assignment(fader_name) == new_channel {
                fader_to_switch = Some(fader_name);
            }
        }

        if fader_to_switch.is_none() {
            // Whatever is on the fader already is going away, per windows behaviour we need to
            // ensure any mute behaviour is restored as it can no longer be tracked.
            let mute_config: &mut MuteButton = self.profile.get_mute_button(fader);
            let colour_map = mute_config.colour_map();

            let muted_to_x = colour_map.state().as_ref().unwrap() == &ColourState::On;

            if muted_to_x {
                // Simulate a mute button tap, this should restore everything..
                self.handle_fader_mute(fader, false).await?;
            }

            // Check to see if we are dispatching of the mic channel, if so set the id.
            if existing_channel == ChannelName::Mic {
                self.profile.get_mute_chat().set_mic_fader_id(4);
            }

            // Now set the new fader..
            self.profile.set_fader_assignment(fader, new_channel);
            self.goxlr.set_fader(fader, new_channel)?;

            self.status.set_fader_assignment(fader, new_channel);

            return Ok(());
        }

        // So we need to switch the faders and mute settings, but nothing else actually changes,
        // we'll simply switch the faders and mute buttons in the config, then apply to the
        // GoXLR.
        self.profile.switch_fader_assignment(fader, fader_to_switch.unwrap());

        // Are either of the moves being done by the mic channel?
        if new_channel == ChannelName::Mic {
            self.profile.get_mute_chat().set_mic_fader_id(fader as u8);
        }

        if existing_channel == ChannelName::Mic {
            self.profile.get_mute_chat().set_mic_fader_id(fader_to_switch.unwrap() as u8);
        }

        // Now switch the faders on the GoXLR..
        self.goxlr.set_fader(fader, new_channel)?;
        self.goxlr.set_fader(fader_to_switch.unwrap(), existing_channel)?;

        // Sync MixerStatus..
        self.status.set_fader_assignment(fader, new_channel);
        self.status.set_fader_assignment(fader_to_switch.unwrap(), existing_channel);

        // Finally update the button colours..
        self.update_button_states()?;

        Ok(())
    }

    fn apply_profile(&mut self) -> Result<()> {
        self.status.profile_name = self.profile.name().to_owned();

        // Set volumes first, applying mute may modify stuff..
        for channel in ChannelName::iter() {
            let channel_volume = self.profile.get_channel_volume(channel);
            self.goxlr.set_volume(channel, channel_volume)?;
            self.status.set_channel_volume(channel, channel_volume);
        }

        // Prepare the faders, and configure channel mute states
        for fader in FaderName::iter() {
            self.goxlr.set_fader(fader, self.profile.get_fader_assignment(fader))?;
            self.status.set_fader_assignment(fader, self.profile.get_fader_assignment(fader));
            self.apply_mute_from_profile(fader)?;
        }

        self.apply_cough_from_profile()?;

        // Load the colour Map..
        let use_1_3_40_format = version_newer_or_equal_to(
            &self.status.hardware.versions.firmware,
            VersionNumber(1, 3, 40, 0),
        );
        let colour_map = self.profile.get_colour_map(use_1_3_40_format);

        if use_1_3_40_format {
            self.goxlr.set_button_colours_1_3_40(colour_map)?;
        } else {
            let mut map: [u8; 328] = [0; 328];
            map.copy_from_slice(&colour_map[0..328]);
            self.goxlr.set_button_colours(map)?;
        }

        for fader in FaderName::iter() {
            self.goxlr.set_fader_display_mode(
                fader,
                self.profile.is_fader_gradient(fader),
                self.profile.is_fader_meter(fader)
            )?;
        }

        self.update_button_states()?;

        let router = self.profile.create_router();
        self.status.router = router;

        // For profile load, we should configure all the input channels from the profile,
        // this is split so we can do tweaks in places where needed.
        for input in BasicInputDevice::iter() {
            self.apply_routing(input)?;
        }

        Ok(())
    }

    fn apply_mic_profile(&mut self) -> Result<()> {
        self.status.mic_profile_name = self.mic_profile.name().to_owned();

        self.goxlr.set_microphone_gain(
            self.mic_profile.mic_type(),
            self.mic_profile.mic_gains()[self.mic_profile.mic_type() as usize],
        )?;

        // Sync with Status..
        self.status.mic_gains = self.mic_profile.mic_gains();
        self.status.mic_type = self.mic_profile.mic_type();

        // I can't think of a cleaner way of doing this..
        let params = self.mic_profile.mic_params();
        self.goxlr.set_mic_param(&[
            (MicrophoneParamKey::GateThreshold, &params[0]),
            (MicrophoneParamKey::GateAttack, &params[1]),
            (MicrophoneParamKey::GateRelease, &params[2]),
            (MicrophoneParamKey::GateAttenuation, &params[3]),
            (MicrophoneParamKey::CompressorThreshold, &params[4]),
            (MicrophoneParamKey::CompressorRatio, &params[5]),
            (MicrophoneParamKey::CompressorAttack, &params[6]),
            (MicrophoneParamKey::CompressorRelease, &params[7]),
            (MicrophoneParamKey::CompressorMakeUpGain, &params[8]),
        ])?;

        let main_effects = self.mic_profile.mic_effects();
        let eq_gains = self.mic_profile.get_eq_gain();
        let eq_freq = self.mic_profile.get_eq_freq();

        self.goxlr.set_effect_values(&[
            (EffectKey::DeEsser, self.mic_profile.get_deesser()),

            (EffectKey::GateThreshold, main_effects[0]),
            (EffectKey::GateAttack, main_effects[1]),
            (EffectKey::GateRelease, main_effects[2]),
            (EffectKey::GateAttenuation, main_effects[3]),
            (EffectKey::CompressorThreshold, main_effects[4]),
            (EffectKey::CompressorRatio, main_effects[5]),
            (EffectKey::CompressorAttack, main_effects[6]),
            (EffectKey::CompressorRelease, main_effects[7]),
            (EffectKey::CompressorMakeUpGain, main_effects[8]),

            (EffectKey::GateEnabled, 1),
            (EffectKey::BleepLevel, -10),
            (EffectKey::GateMode, 2),


            // Disable all the voice effects, these are enabled by default and seem
            // to mess with the initial mic!
            (EffectKey::Encoder1Enabled, 0),
            (EffectKey::Encoder2Enabled, 0),
            (EffectKey::Encoder3Enabled, 0),
            (EffectKey::Encoder4Enabled, 0),
            (EffectKey::RobotEnabled, 0),
            (EffectKey::HardTuneEnabled, 0),
            (EffectKey::MegaphoneEnabled, 0),
        ])?;

        // Apply EQ only on the 'Full' device
        if self.status.hardware.device_type == DeviceType::Full {
            self.goxlr.set_effect_values(&[
                (EffectKey::Equalizer31HzValue, eq_gains[0]),
                (EffectKey::Equalizer63HzValue, eq_gains[1]),
                (EffectKey::Equalizer125HzValue, eq_gains[2]),
                (EffectKey::Equalizer250HzValue, eq_gains[3]),
                (EffectKey::Equalizer500HzValue, eq_gains[4]),
                (EffectKey::Equalizer1KHzValue, eq_gains[5]),
                (EffectKey::Equalizer2KHzValue, eq_gains[6]),
                (EffectKey::Equalizer4KHzValue, eq_gains[7]),
                (EffectKey::Equalizer8KHzValue, eq_gains[8]),
                (EffectKey::Equalizer16KHzValue, eq_gains[9]),

                (EffectKey::Equalizer31HzFrequency, eq_freq[0]),
                (EffectKey::Equalizer63HzFrequency, eq_freq[1]),
                (EffectKey::Equalizer125HzFrequency, eq_freq[2]),
                (EffectKey::Equalizer250HzFrequency, eq_freq[3]),
                (EffectKey::Equalizer500HzFrequency, eq_freq[4]),
                (EffectKey::Equalizer1KHzFrequency, eq_freq[5]),
                (EffectKey::Equalizer2KHzFrequency, eq_freq[6]),
                (EffectKey::Equalizer4KHzFrequency, eq_freq[7]),
                (EffectKey::Equalizer8KHzFrequency, eq_freq[8]),
                (EffectKey::Equalizer16KHzFrequency, eq_freq[9]),
            ])?;
        }

        Ok(())
    }

    // Get the current time in millis..
    fn get_epoch_ms(&self) -> u128 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
    }
}