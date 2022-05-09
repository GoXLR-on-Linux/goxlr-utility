use crate::profile::{version_newer_or_equal_to, MicProfileAdapter, ProfileAdapter};
use crate::SettingsHandle;
use anyhow::Result;
use enumset::EnumSet;
use goxlr_ipc::{DeviceType, FaderStatus, GoXLRCommand, HardwareStatus, MixerStatus};
use goxlr_types::{ChannelName, EffectKey, FaderName, InputDevice as BasicInputDevice, MicrophoneParamKey, OutputDevice as BasicOutputDevice, VersionNumber };
use goxlr_usb::buttonstate::{ButtonStates, Buttons};
use goxlr_usb::channelstate::ChannelState;
use goxlr_usb::goxlr::GoXLR;
use goxlr_usb::routing::{InputDevice, OutputDevice};
use goxlr_usb::rusb::UsbContext;
use log::debug;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use enum_map::EnumMap;
use strum::{IntoEnumIterator};
use goxlr_profile_loader::components::mute::{MuteFunction};
use goxlr_usb::channelstate::ChannelState::{Muted, Unmuted};

#[derive(Debug)]
pub struct Device<T: UsbContext> {
    goxlr: GoXLR<T>,
    hardware: HardwareStatus,
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

        let mut device = Self {
            profile,
            mic_profile,
            goxlr,
            hardware,
            last_buttons: EnumSet::empty(),
            button_states: EnumMap::default(),
        };

        device.apply_profile()?;
        device.apply_mic_profile()?;

        Ok(device)
    }

    pub fn serial(&self) -> &str {
        &self.hardware.serial_number
    }

    pub fn status(&self) -> MixerStatus {
        MixerStatus {
            hardware: self.hardware.clone(),
            fader_a_assignment: self.get_fader_state(FaderName::A),
            fader_b_assignment: self.get_fader_state(FaderName::B),
            fader_c_assignment: self.get_fader_state(FaderName::C),
            fader_d_assignment: self.get_fader_state(FaderName::D),
            volumes: self.profile.get_volumes(),
            router: self.profile.create_router(),
            mic_gains: self.mic_profile.mic_gains(),
            mic_type: self.mic_profile.mic_type(),
            profile_name: self.profile.name().to_owned(),
            mic_profile_name: self.mic_profile.name().to_owned()
        }
    }

    pub fn profile(&self) -> &ProfileAdapter {
        &self.profile
    }

    pub fn mic_profile(&self) -> &MicProfileAdapter {
        &self.mic_profile
    }

    pub async fn monitor_inputs(&mut self, settings: &SettingsHandle) -> Result<()> {
        self.hardware.usb_device.has_kernel_driver_attached =
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
            },
            Buttons::Bleep => {
                self.handle_swear_button(true).await?;
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
            },
            Buttons::Bleep => {
                self.handle_swear_button(false).await?;
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

        let (muted_to_x, muted_to_all, mute_function) = self.profile.get_mute_button_state(fader);

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

            self.profile.set_mute_button_previous_volume(fader, current_volume);

            self.goxlr.set_volume(channel, 0)?;
            self.goxlr.set_channel_state(channel, Muted)?;

            self.profile.set_mute_button_on(fader, true);

            if held {
                self.profile.set_mute_button_blink(fader, true);
            }

            self.profile.set_channel_volume(channel, 0);

            return Ok(());
        }

        // Button has been pressed, and we're already in some kind of muted state..
        if !held && muted_to_x {
            // Disable the lighting regardless of action
            self.profile.set_mute_button_on(fader, false);
            self.profile.set_mute_button_blink(fader, false);

            if muted_to_all || mute_function == MuteFunction::All {
                let previous_volume = self.profile.get_mute_button_previous_volume(fader);

                self.goxlr.set_volume(channel, previous_volume)?;
                self.profile.set_channel_volume(channel, previous_volume);

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
            self.profile.set_mute_button_on(fader, true);
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
        let (mute_toggle, muted_to_x, muted_to_all, mute_function) =
            self.profile.get_mute_chat_button_state();

        // Ok, lets handle things in order, was this button just pressed?
        if press {
            if mute_toggle {
                // Mute toggles are only handled on release.
                return Ok(());
            }

            // Enable the cough button in all cases..
            self.profile.set_mute_chat_button_on(true);

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
            self.profile.set_mute_chat_button_on(true);
            self.profile.set_mute_chat_button_blink(true);

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
                    self.profile.set_mute_chat_button_on(false);
                    self.profile.set_mute_chat_button_blink(false);

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
                self.profile.set_mute_chat_button_on(true);

                if mute_function == MuteFunction::All {
                    self.goxlr.set_channel_state(ChannelName::Mic, Muted)?;
                    return Ok(())
                }

                // Update the transient routing..
                self.apply_routing(BasicInputDevice::Microphone)?;
                return Ok(())
            }

            self.profile.set_mute_chat_button_on(false);
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

    async fn handle_swear_button(&mut self, press: bool) -> Result<()> {
        // Pretty simple, turn the light on when pressed, off when released..
        self.profile.set_swear_button_on(press);
        Ok(())
    }

    fn mic_muted_by_fader(&self) -> bool {
        // Is the mute button even assigned to a fader?
        let mic_fader_id = self.profile.get_mic_fader_id();

        if mic_fader_id == 4 {
            return false;
        }

        let fader = self.profile.fader_from_id(mic_fader_id);
        let (muted_to_x, muted_to_all, mute_function) = self.profile.get_mute_button_state(fader);

        return muted_to_all || (muted_to_x && mute_function == MuteFunction::All);
    }

    fn mic_muted_by_cough(&self) -> bool {
        let (_mute_toggle, muted_to_x, muted_to_all, mute_function) =
            self.profile.get_mute_chat_button_state();

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
                self.goxlr.set_volume(channel, volume)?;
            }
            GoXLRCommand::SetMicrophoneGain(mic_type, gain) => {
                self.goxlr.set_microphone_gain(mic_type, gain.into())?;
                self.mic_profile.set_mic_type(mic_type);
                self.mic_profile.set_mic_gain(mic_type, gain);
            }
            GoXLRCommand::SetRouter(input, output, enabled) => {
                debug!("Setting Routing: {:?} {:?} {}", input, output, enabled);
                self.profile.set_routing(input, output, enabled);

                // Apply the change..
                self.apply_routing(input)?;
            }
            GoXLRCommand::ListProfiles() => {
                // Need to send a response.. No idea how that works yet :D
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
            GoXLRCommand::SaveProfile() => {
                let profile_directory = settings.get_profile_directory().await;
                let profile_name = settings.get_device_profile_name(self.serial()).await;

                if let Some(profile_name) = profile_name {
                    self.profile.to_named(profile_name, &profile_directory)?;
                }

            }
            GoXLRCommand::SaveMicProfile() => {

            }
        }

        Ok(())
    }

    fn update_button_states(&mut self) -> Result<()> {
        let button_states = self.create_button_states();
        self.goxlr.set_button_states(button_states)?;
        Ok(())
    }

    fn create_button_states(&self) -> [ButtonStates; 24] {
        let mut result = [ButtonStates::DimmedColour1; 24];

        for button in Buttons::iter() {
            result[button as usize] = self.profile.get_button_colour_state(button);
        }

        // Replace the Cough Button button data with correct data.
        result[Buttons::MicrophoneMute as usize] = self.profile.get_mute_chat_button_colour_state();
        result
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

    fn apply_transient_routing(&self, input: BasicInputDevice, router: &mut EnumMap<BasicOutputDevice, bool>) {
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
                self.apply_transient_fader_routing(fader, router);
            }
        }
        self.apply_transient_cough_routing(router);
    }

    fn apply_transient_fader_routing(&self, fader: FaderName, router: &mut EnumMap<BasicOutputDevice, bool>) {
        let (muted_to_x, muted_to_all, mute_function) = self.profile.get_mute_button_state(fader);
        self.apply_transient_channel_routing(muted_to_x, muted_to_all, mute_function, router);
    }

    fn apply_transient_cough_routing(&self, router: &mut EnumMap<BasicOutputDevice, bool>) {
        // Same deal, pull out the current state, make needed changes.
        let (_mute_toggle, muted_to_x, muted_to_all, mute_function) =
            self.profile.get_mute_chat_button_state();

        self.apply_transient_channel_routing(muted_to_x, muted_to_all, mute_function, router);
    }

    fn apply_transient_channel_routing(&self, muted_to_x: bool, muted_to_all: bool, mute_function: MuteFunction, router: &mut EnumMap<BasicOutputDevice, bool>) {
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
        let mut router = self.profile.get_router(input);
        self.apply_transient_routing(input, &mut router);
        debug!("Applying Routing to {:?}:", input);
        debug!("{:?}", router);

        self.apply_channel_routing(input, router)?;

        Ok(())
    }

    fn apply_mute_from_profile(&mut self, fader: FaderName) -> Result<()> {
        // Basically stripped down behaviour from handle_fader_mute which simply applies stuff.
        let channel = self.profile.get_fader_assignment(fader);

        let (muted_to_x, muted_to_all, mute_function) = self.profile.get_mute_button_state(fader);
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
        let (mute_toggle, muted_to_x, muted_to_all, mute_function) =
            self.profile.get_mute_chat_button_state();

        // Firstly, if toggle is to hold and anything is muted, clear it.
        if !mute_toggle && muted_to_x {
            self.profile.set_mute_chat_button_on(false);
            self.profile.set_mute_chat_button_blink(false);
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
                self.profile.set_mic_fader_id(fader as u8);
            }


            self.goxlr.set_fader(fader, new_channel)?;
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
            let (muted_to_x, _muted_to_all, _mute_function) = self.profile.get_mute_button_state(fader);

            if muted_to_x {
                // Simulate a mute button tap, this should restore everything..
                self.handle_fader_mute(fader, false).await?;
            }

            // Check to see if we are dispatching of the mic channel, if so set the id.
            if existing_channel == ChannelName::Mic {
                self.profile.set_mic_fader_id(4);
            }

            // Now set the new fader..
            self.profile.set_fader_assignment(fader, new_channel);
            self.goxlr.set_fader(fader, new_channel)?;

            return Ok(());
        }

        // This will always be set here..
        let fader_to_switch = fader_to_switch.unwrap();

        // So we need to switch the faders and mute settings, but nothing else actually changes,
        // we'll simply switch the faders and mute buttons in the config, then apply to the
        // GoXLR.
        self.profile.switch_fader_assignment(fader, fader_to_switch);

        // Are either of the moves being done by the mic channel?
        if new_channel == ChannelName::Mic {
            self.profile.set_mic_fader_id(fader as u8);
        }

        if existing_channel == ChannelName::Mic {
            self.profile.set_mic_fader_id(fader_to_switch as u8);
        }



        // Now switch the faders on the GoXLR..
        self.goxlr.set_fader(fader, new_channel)?;
        self.goxlr.set_fader(fader_to_switch, existing_channel)?;

        // Finally update the button colours..
        self.update_button_states()?;

        Ok(())
    }

    fn get_fader_state(&self, fader: FaderName) -> FaderStatus {
        FaderStatus {
            channel: self.profile().get_fader_assignment(fader),
            mute_type: self.profile().get_mute_button_behaviour(fader)
        }
    }

    fn apply_profile(&mut self) -> Result<()> {
        // Set volumes first, applying mute may modify stuff..
        for channel in ChannelName::iter() {
            let channel_volume = self.profile.get_channel_volume(channel);
            self.goxlr.set_volume(channel, channel_volume)?;
        }

        // Prepare the faders, and configure channel mute states
        for fader in FaderName::iter() {
            self.goxlr.set_fader(fader, self.profile.get_fader_assignment(fader))?;
            self.apply_mute_from_profile(fader)?;
        }

        self.apply_cough_from_profile()?;

        // Load the colour Map..
        let use_1_3_40_format = version_newer_or_equal_to(
            &self.hardware.versions.firmware,
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

        // For profile load, we should configure all the input channels from the profile,
        // this is split so we can do tweaks in places where needed.
        for input in BasicInputDevice::iter() {
            self.apply_routing(input)?;
        }

        Ok(())
    }

    fn apply_mic_profile(&mut self) -> Result<()> {
        self.goxlr.set_microphone_gain(
            self.mic_profile.mic_type(),
            self.mic_profile.mic_gains()[self.mic_profile.mic_type() as usize],
        )?;

        // I can't think of a cleaner way of doing this..
        let params = self.mic_profile.mic_params();

        // The EQ from the mini is seemingly always sent regardless of the device in use, the
        // full device will replace it via Effects later.
        let eq_gains = self.mic_profile.get_eq_gain_mini();
        let eq_freqs = self.mic_profile.get_eq_freq_mini();

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

            (MicrophoneParamKey::Equalizer90HzFrequency, &eq_freqs[0]),
            (MicrophoneParamKey::Equalizer250HzFrequency, &eq_freqs[1]),
            (MicrophoneParamKey::Equalizer500HzFrequency, &eq_freqs[2]),
            (MicrophoneParamKey::Equalizer1KHzFrequency, &eq_freqs[3]),
            (MicrophoneParamKey::Equalizer3KHzFrequency, &eq_freqs[4]),
            (MicrophoneParamKey::Equalizer8KHzFrequency, &eq_freqs[5]),

            (MicrophoneParamKey::Equalizer90HzGain, &eq_gains[0]),
            (MicrophoneParamKey::Equalizer250HzGain, &eq_gains[1]),
            (MicrophoneParamKey::Equalizer500HzGain, &eq_gains[2]),
            (MicrophoneParamKey::Equalizer1KHzGain, &eq_gains[3]),
            (MicrophoneParamKey::Equalizer3KHzGain, &eq_gains[4]),
            (MicrophoneParamKey::Equalizer8KHzGain, &eq_gains[5]),

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

            // We don't use this effect key under Linux (mostly due to there being other ways
            // to mute a channel), so we'll set this to 0 just in case someone is coming from
            // windows where it *IS* used during mic muting.
            (EffectKey::DisableMic, 0),



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
        if self.hardware.device_type == DeviceType::Full {
            self.goxlr.set_effect_values(&[
                (EffectKey::Equalizer31HzGain, eq_gains[0]),
                (EffectKey::Equalizer63HzGain, eq_gains[1]),
                (EffectKey::Equalizer125HzGain, eq_gains[2]),
                (EffectKey::Equalizer250HzGain, eq_gains[3]),
                (EffectKey::Equalizer500HzGain, eq_gains[4]),
                (EffectKey::Equalizer1KHzGain, eq_gains[5]),
                (EffectKey::Equalizer2KHzGain, eq_gains[6]),
                (EffectKey::Equalizer4KHzGain, eq_gains[7]),
                (EffectKey::Equalizer8KHzGain, eq_gains[8]),
                (EffectKey::Equalizer16KHzGain, eq_gains[9]),

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