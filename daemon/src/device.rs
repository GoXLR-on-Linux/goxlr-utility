use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Result};
use chrono::Local;
use enum_map::EnumMap;
use enumset::EnumSet;
use futures::executor::block_on;
use log::{debug, error, info};
use ritelinked::LinkedHashSet;
use strum::IntoEnumIterator;

use goxlr_ipc::{
    DeviceType, Display, FaderStatus, GoXLRCommand, HardwareStatus, Levels, MicSettings,
    MixerStatus, Settings,
};
use goxlr_profile_loader::components::mute::MuteFunction;
use goxlr_types::{
    Button, ChannelName, DisplayModeComponents, EffectBankPresets, EffectKey, EncoderName,
    FaderName, HardTuneSource, InputDevice as BasicInputDevice, MicrophoneParamKey, MuteState,
    OutputDevice as BasicOutputDevice, RobotRange, SampleBank, SampleButtons, SamplePlaybackMode,
    VersionNumber,
};
use goxlr_usb::buttonstate::{ButtonStates, Buttons};
use goxlr_usb::channelstate::ChannelState::{Muted, Unmuted};
use goxlr_usb::device::base::FullGoXLRDevice;
use goxlr_usb::routing::{InputDevice, OutputDevice};

use crate::audio::{AudioFile, AudioHandler};
use crate::files::find_file_in_path;
use crate::mic_profile::{MicProfileAdapter, DEFAULT_MIC_PROFILE_NAME};
use crate::profile::{
    usb_to_standard_button, version_newer_or_equal_to, ProfileAdapter, DEFAULT_PROFILE_NAME,
};
use crate::SettingsHandle;

pub struct Device<'a> {
    goxlr: Box<dyn FullGoXLRDevice>,
    hardware: HardwareStatus,
    last_buttons: EnumSet<Buttons>,
    button_states: EnumMap<Buttons, ButtonState>,
    fader_last_seen: EnumMap<FaderName, u8>,
    profile: ProfileAdapter,
    mic_profile: MicProfileAdapter,
    audio_handler: Option<AudioHandler>,
    hold_time: u16,
    vc_mute_also_mute_cm: bool,
    settings: &'a SettingsHandle,
}

// Experimental code:
#[derive(Debug, Default, Copy, Clone)]
struct ButtonState {
    press_time: u128,
    hold_handled: bool,
}

impl<'a> Device<'a> {
    pub fn new(
        goxlr: Box<dyn FullGoXLRDevice>,
        hardware: HardwareStatus,
        profile_name: Option<String>,
        mic_profile_name: Option<String>,
        profile_directory: &Path,
        mic_profile_directory: &Path,
        settings_handle: &'a SettingsHandle,
    ) -> Result<Self> {
        let mut device_type = "";
        if hardware.device_type == DeviceType::Mini {
            device_type = " Mini";
        }

        let profile = profile_name.unwrap_or_else(|| DEFAULT_PROFILE_NAME.to_string());
        let mic_profile = mic_profile_name.unwrap_or_else(|| DEFAULT_MIC_PROFILE_NAME.to_string());

        info!(
            "Configuring GoXLR{}, Profile: {}, Mic Profile: {}",
            device_type, profile, mic_profile
        );

        let profile = ProfileAdapter::from_named_or_default(profile, profile_directory);
        let mic_profile =
            MicProfileAdapter::from_named_or_default(mic_profile, mic_profile_directory);

        let audio_buffer =
            block_on(settings_handle.get_device_sampler_pre_buffer(&hardware.serial_number));
        let audio_loader = AudioHandler::new(audio_buffer);
        debug!("Created Audio Handler..");
        debug!("{:?}", audio_loader);

        if let Err(e) = &audio_loader {
            error!("Error Running Script: {}", e);
        }

        let mut audio_handler = None;
        if let Ok(audio) = audio_loader {
            debug!("Audio Handler Loaded OK..");
            audio_handler = Some(audio);
        }

        let hold_time = block_on(settings_handle.get_device_hold_time(&hardware.serial_number));
        let vc_mute_also_mute_cm = block_on(
            settings_handle.get_device_chat_mute_mutes_mic_to_chat(&hardware.serial_number),
        );
        let mut device = Self {
            profile,
            mic_profile,
            goxlr,
            hardware,
            hold_time,
            vc_mute_also_mute_cm,
            last_buttons: EnumSet::empty(),
            button_states: EnumMap::default(),
            fader_last_seen: EnumMap::default(),
            audio_handler,
            settings: settings_handle,
        };

        device.apply_profile()?;
        device.apply_mic_profile()?;

        Ok(device)
    }

    pub fn serial(&self) -> &str {
        &self.hardware.serial_number
    }

    pub fn status(&self) -> MixerStatus {
        let mut fader_map: EnumMap<FaderName, FaderStatus> = Default::default();
        for name in FaderName::iter() {
            fader_map[name] = self.get_fader_state(name);
        }

        let mut button_states: EnumMap<Button, bool> = Default::default();
        for (button, state) in self.button_states.iter() {
            if state.press_time > 0 {
                button_states[usb_to_standard_button(button)] = true;
            }
        }

        let mut volumes: EnumMap<ChannelName, u8> = Default::default();
        for channel in ChannelName::iter() {
            volumes[channel] = self.profile.get_channel_volume(channel);
        }

        let shutdown_commands = block_on(self.settings.get_device_shutdown_commands(self.serial()));
        let sampler_prerecord =
            block_on(self.settings.get_device_sampler_pre_buffer(self.serial()));

        MixerStatus {
            hardware: self.hardware.clone(),
            shutdown_commands,
            fader_status: fader_map,
            cough_button: self.profile.get_cough_status(),
            levels: Levels {
                volumes,
                bleep: self.mic_profile.bleep_level(),
                deess: self.mic_profile.get_deesser(),
            },
            router: self.profile.create_router(),
            mic_status: MicSettings {
                mic_type: self.mic_profile.mic_type(),
                mic_gains: self.mic_profile.mic_gains(),
                noise_gate: self.mic_profile.noise_gate_ipc(),
                equaliser: self.mic_profile.equalizer_ipc(),
                equaliser_mini: self.mic_profile.equalizer_mini_ipc(),
                compressor: self.mic_profile.compressor_ipc(),
            },
            lighting: self
                .profile
                .get_lighting_ipc(self.hardware.device_type == DeviceType::Mini),
            effects: self
                .profile
                .get_effects_ipc(self.hardware.device_type == DeviceType::Mini),
            sampler: self.profile.get_sampler_ipc(
                self.hardware.device_type == DeviceType::Mini,
                &self.audio_handler,
                sampler_prerecord,
            ),
            settings: Settings {
                display: Display {
                    gate: self.mic_profile.get_gate_display_mode(),
                    compressor: self.mic_profile.get_compressor_display_mode(),
                    equaliser: self.mic_profile.get_eq_display_mode(),
                    equaliser_fine: self.mic_profile.get_eq_fine_display_mode(),
                },
                mute_hold_duration: self.hold_time,
                vc_mute_also_mute_cm: self.vc_mute_also_mute_cm,
            },
            button_down: button_states,
            profile_name: self.profile.name().to_owned(),
            mic_profile_name: self.mic_profile.name().to_owned(),
        }
    }

    pub async fn shutdown(&mut self) {
        debug!("Shutting Down Device: {}", self.hardware.serial_number);

        let commands = self
            .settings
            .get_device_shutdown_commands(&self.hardware.serial_number)
            .await;

        for command in commands {
            debug!("{:?}", command);

            // These could fail, but fuck it, we gotta do it..
            let _ = self.perform_command(command).await;
        }
    }

    pub fn profile(&self) -> &ProfileAdapter {
        &self.profile
    }

    pub fn mic_profile(&self) -> &MicProfileAdapter {
        &self.mic_profile
    }

    pub async fn update_state(&mut self) -> Result<bool> {
        let mut state_updated = false;

        // Update any audio related states..
        if let Some(audio_handler) = &mut self.audio_handler {
            audio_handler.check_playing().await;
            state_updated = self.sync_sample_lighting().await?;
        }

        // Find any buttons that have been held, and action if needed.
        for button in self.last_buttons {
            if !self.button_states[button].hold_handled {
                let now = self.get_epoch_ms();
                if (now - self.button_states[button].press_time) > self.hold_time.into() {
                    if let Err(error) = self.on_button_hold(button).await {
                        error!("{}", error);
                    }
                    self.button_states[button].hold_handled = true;
                }
            }
        }

        Ok(state_updated)
    }

    pub async fn monitor_inputs(&mut self) -> Result<bool> {
        let state = self.goxlr.get_button_states()?;
        let mut changed = self.update_volumes_to(state.volumes)?;
        let result = self.update_encoders_to(state.encoders)?;
        if !changed {
            // Only change the value if it's not already true..
            changed = result;
        }

        let pressed_buttons = state.pressed.difference(self.last_buttons);
        for button in pressed_buttons {
            // This is a new press, store it in the states..
            self.button_states[button] = ButtonState {
                press_time: self.get_epoch_ms(),
                hold_handled: false,
            };

            if let Err(error) = self.on_button_down(button).await {
                error!("{}", error);
            }

            changed = true;
        }

        let released_buttons = self.last_buttons.difference(state.pressed);
        for button in released_buttons {
            let button_state = self.button_states[button];

            // Output errors, but don't throw them up the stack!
            if let Err(error) = self.on_button_up(button, &button_state).await {
                error!("{}", error);
            }

            self.button_states[button] = ButtonState {
                press_time: 0,
                hold_handled: false,
            };

            changed = true;
        }

        self.last_buttons = state.pressed;
        Ok(changed)
    }

    async fn on_button_down(&mut self, button: Buttons) -> Result<()> {
        debug!("Handling Button Down: {:?}", button);

        match button {
            Buttons::MicrophoneMute => {
                self.handle_cough_mute(true, false, false, false).await?;
            }
            Buttons::Bleep => {
                self.handle_swear_button(true).await?;
            }
            Buttons::SamplerBottomLeft => {
                self.handle_sample_button_down(SampleButtons::BottomLeft)
                    .await?;
            }
            Buttons::SamplerBottomRight => {
                self.handle_sample_button_down(SampleButtons::BottomRight)
                    .await?;
            }
            Buttons::SamplerTopLeft => {
                self.handle_sample_button_down(SampleButtons::TopLeft)
                    .await?;
            }
            Buttons::SamplerTopRight => {
                self.handle_sample_button_down(SampleButtons::TopRight)
                    .await?;
            }
            _ => {}
        }
        self.update_button_states()?;
        Ok(())
    }

    async fn on_button_hold(&mut self, button: Buttons) -> Result<()> {
        debug!("Handling Button Hold: {:?}", button);

        // Fader mute buttons maintain their own state check, so it can be programmatically called.
        match button {
            Buttons::Fader1Mute => {
                self.handle_fader_mute(FaderName::A, true).await?;
                return Ok(());
            }
            Buttons::Fader2Mute => {
                self.handle_fader_mute(FaderName::B, true).await?;
                return Ok(());
            }
            Buttons::Fader3Mute => {
                self.handle_fader_mute(FaderName::C, true).await?;
                return Ok(());
            }
            Buttons::Fader4Mute => {
                self.handle_fader_mute(FaderName::D, true).await?;
                return Ok(());
            }
            Buttons::MicrophoneMute => {
                self.handle_cough_mute(false, false, true, false).await?;
            }
            _ => {}
        }
        self.update_button_states()?;
        Ok(())
    }

    async fn on_button_up(&mut self, button: Buttons, state: &ButtonState) -> Result<()> {
        debug!(
            "Handling Button Release: {:?}, Has Long Press Handled: {:?}",
            button, state.hold_handled
        );
        match button {
            Buttons::Fader1Mute => {
                if !state.hold_handled {
                    self.handle_fader_mute(FaderName::A, false).await?;
                    return Ok(());
                }
            }
            Buttons::Fader2Mute => {
                if !state.hold_handled {
                    self.handle_fader_mute(FaderName::B, false).await?;
                    return Ok(());
                }
            }
            Buttons::Fader3Mute => {
                if !state.hold_handled {
                    self.handle_fader_mute(FaderName::C, false).await?;
                    return Ok(());
                }
            }
            Buttons::Fader4Mute => {
                if !state.hold_handled {
                    self.handle_fader_mute(FaderName::D, false).await?;
                    return Ok(());
                }
            }
            Buttons::MicrophoneMute => {
                self.handle_cough_mute(false, true, false, state.hold_handled)
                    .await?;
            }
            Buttons::Bleep => {
                self.handle_swear_button(false).await?;
            }
            Buttons::EffectSelect1 => {
                self.load_effect_bank(EffectBankPresets::Preset1).await?;
            }
            Buttons::EffectSelect2 => {
                self.load_effect_bank(EffectBankPresets::Preset2).await?;
            }
            Buttons::EffectSelect3 => {
                self.load_effect_bank(EffectBankPresets::Preset3).await?;
            }
            Buttons::EffectSelect4 => {
                self.load_effect_bank(EffectBankPresets::Preset4).await?;
            }
            Buttons::EffectSelect5 => {
                self.load_effect_bank(EffectBankPresets::Preset5).await?;
            }
            Buttons::EffectSelect6 => {
                self.load_effect_bank(EffectBankPresets::Preset6).await?;
            }

            // The following 3 are simple, but will need more work once effects are
            // actually applied!
            Buttons::EffectMegaphone => {
                self.set_megaphone(!self.profile.is_megaphone_enabled(true))
                    .await?;
            }
            Buttons::EffectRobot => {
                self.set_robot(!self.profile.is_robot_enabled(true)).await?;
            }
            Buttons::EffectHardTune => {
                self.set_hardtune(!self.profile.is_hardtune_enabled(true))
                    .await?;
            }
            Buttons::EffectFx => {
                self.set_effects(!self.profile.is_fx_enabled()).await?;
            }

            Buttons::SamplerSelectA => {
                self.load_sample_bank(SampleBank::A).await?;
                self.load_colour_map()?;
            }
            Buttons::SamplerSelectB => {
                self.load_sample_bank(SampleBank::B).await?;
                self.load_colour_map()?;
            }
            Buttons::SamplerSelectC => {
                self.load_sample_bank(SampleBank::C).await?;
                self.load_colour_map()?;
            }

            Buttons::SamplerBottomLeft => {
                self.handle_sample_button_release(SampleButtons::BottomLeft)
                    .await?;
            }
            Buttons::SamplerBottomRight => {
                self.handle_sample_button_release(SampleButtons::BottomRight)
                    .await?;
            }
            Buttons::SamplerTopLeft => {
                self.handle_sample_button_release(SampleButtons::TopLeft)
                    .await?;
            }
            Buttons::SamplerTopRight => {
                self.handle_sample_button_release(SampleButtons::TopRight)
                    .await?;
            }
            Buttons::SamplerClear => {
                self.profile
                    .set_sample_clear_active(!self.profile.is_sample_clear_active())?;
            }
        }
        self.update_button_states()?;
        Ok(())
    }

    async fn handle_fader_mute(&mut self, fader: FaderName, held: bool) -> Result<()> {
        // OK, so a fader button has been pressed, we need to determine behaviour, based on the colour map..
        let (muted_to_x, muted_to_all, mute_function) = self.profile.get_mute_button_state(fader);

        // Should we be muting this fader to all channels?
        if held || (!muted_to_x && mute_function == MuteFunction::All) {
            if held && muted_to_all {
                // Holding the button when it's already muted to all does nothing.
                return Ok(());
            }
            self.mute_fader_to_all(fader, held).await?;
        }

        // Button has been pressed, and we're already in some kind of muted state..
        if !held && muted_to_x {
            self.unmute_fader(fader).await?;
        }

        // Button has been pressed, we're not muted, and we need to transient mute..
        if !held && !muted_to_x && mute_function != MuteFunction::All {
            self.mute_fader_to_x(fader).await?;
        }
        Ok(())
    }

    async fn unmute_chat_if_muted(&mut self) -> Result<()> {
        let (_mute_toggle, muted_to_x, muted_to_all, _mute_function) =
            self.profile.get_mute_chat_button_state();

        if muted_to_x || muted_to_all {
            self.handle_cough_mute(true, false, false, false).await?;
        }

        Ok(())
    }

    // This one's a little obnoxious because it's heavily settings dependent, so will contain a
    // large volume of comments working through states, feel free to remove them later :)
    async fn handle_cough_mute(
        &mut self,
        press: bool,
        release: bool,
        held: bool,
        held_called: bool,
    ) -> Result<()> {
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
            }

            self.apply_routing(BasicInputDevice::Microphone)?;
            return Ok(());
        }

        if held {
            if !mute_toggle {
                // Holding in this scenario just keeps the channel muted, so no change here.
                return Ok(());
            }

            // We're togglable, so enable blink, set cough_button_on, mute the channel fully and
            // remove any transient routing which may be set.
            self.profile.set_mute_chat_button_on(true);
            self.profile.set_mute_chat_button_blink(true);

            self.goxlr.set_channel_state(ChannelName::Mic, Muted)?;
            self.apply_routing(BasicInputDevice::Microphone)?;
            return Ok(());
        }

        if release {
            if mute_toggle {
                if held_called {
                    // We don't need to do anything here, a long press has already been handled.
                    return Ok(());
                }

                if muted_to_x || muted_to_all {
                    self.profile.set_mute_chat_button_on(false);
                    self.profile.set_mute_chat_button_blink(false);

                    if (muted_to_all || (muted_to_x && mute_function == MuteFunction::All))
                        && !self.mic_muted_by_fader()
                    {
                        self.goxlr.set_channel_state(ChannelName::Mic, Unmuted)?;
                    }

                    self.apply_routing(BasicInputDevice::Microphone)?;
                    return Ok(());
                }

                // In all cases, enable the button
                self.profile.set_mute_chat_button_on(true);

                if mute_function == MuteFunction::All {
                    self.goxlr.set_channel_state(ChannelName::Mic, Muted)?;
                }

                // Update the transient routing..
                self.apply_routing(BasicInputDevice::Microphone)?;
                return Ok(());
            }

            self.profile.set_mute_chat_button_on(false);
            if mute_function == MuteFunction::All && !self.mic_muted_by_fader() {
                self.goxlr.set_channel_state(ChannelName::Mic, Unmuted)?;
            }

            // Disable button and refresh transient routing
            self.apply_routing(BasicInputDevice::Microphone)?;
            return Ok(());
        }

        Ok(())
    }

    async fn mute_fader_to_x(&mut self, fader: FaderName) -> Result<()> {
        let (muted_to_x, muted_to_all, mute_function) = self.profile.get_mute_button_state(fader);
        let channel = self.profile.get_fader_assignment(fader);
        if muted_to_all {
            bail!("Unable to Transition from MutedToAll to MutedToX");
        }

        if muted_to_x || muted_to_all {
            return Ok(());
        }

        if mute_function == MuteFunction::All {
            // Throw this across to the 'Mute to All' code..
            return self.mute_fader_to_all(fader, false).await;
        }

        let input = self.get_basic_input_from_channel(channel);
        self.profile.set_mute_button_on(fader, true)?;
        if input.is_some() {
            self.apply_routing(input.unwrap())?;
        }
        self.update_button_states()?;
        Ok(())
    }

    async fn mute_fader_to_all(&mut self, fader: FaderName, blink: bool) -> Result<()> {
        let (muted_to_x, muted_to_all, mute_function) = self.profile.get_mute_button_state(fader);
        let channel = self.profile.get_fader_assignment(fader);

        // Are we already muted to all?
        if muted_to_all {
            return Ok(());
        }

        // If we did this on Mute to X, we don't need to do it again..
        if !(muted_to_x && mute_function == MuteFunction::All) {
            let volume = self.profile.get_channel_volume(channel);
            self.profile.set_mute_previous_volume(fader, volume)?;
            self.goxlr.set_volume(channel, 0)?;
            self.goxlr.set_channel_state(channel, Muted)?;
            self.profile.set_mute_button_on(fader, true)?;
        }

        if blink {
            self.profile.set_mute_button_blink(fader, true)?;
        }
        self.profile.set_channel_volume(channel, 0)?;

        // If we're Chat, we may need to transiently route the Microphone..
        if channel == ChannelName::Chat {
            self.apply_routing(BasicInputDevice::Microphone)?;
        }

        if channel == ChannelName::Mic {
            self.apply_routing(BasicInputDevice::Microphone)?;
        }

        self.update_button_states()?;
        Ok(())
    }

    async fn unmute_fader(&mut self, fader: FaderName) -> Result<()> {
        let (muted_to_x, muted_to_all, mute_function) = self.profile.get_mute_button_state(fader);
        let channel = self.profile.get_fader_assignment(fader);

        if !muted_to_x && !muted_to_all {
            // Nothing to do.
            debug!("Doing Nothing?");
            return Ok(());
        }

        // Disable the lighting regardless of action
        self.profile.set_mute_button_on(fader, false)?;
        self.profile.set_mute_button_blink(fader, false)?;

        if muted_to_all || mute_function == MuteFunction::All {
            // This fader has previously been 'Muted to All', we need to restore the volume..
            let previous_volume = self.profile.get_mute_button_previous_volume(fader);

            self.goxlr.set_volume(channel, previous_volume)?;
            self.profile.set_channel_volume(channel, previous_volume)?;

            if channel != ChannelName::Mic
                || (channel == ChannelName::Mic && !self.mic_muted_by_cough())
            {
                self.goxlr.set_channel_state(channel, Unmuted)?;
            }

            // As before, we might need transient Mic Routing..
            if channel == ChannelName::Chat {
                self.apply_routing(BasicInputDevice::Microphone)?;
            }

            if channel == ChannelName::Mic {
                self.apply_routing(BasicInputDevice::Microphone)?;
            }
        }

        // Always do a Transient Routing update, just in case we went from Mute to X -> Mute to All
        let input = self.get_basic_input_from_channel(channel);
        if mute_function != MuteFunction::All && input.is_some() {
            self.apply_routing(input.unwrap())?;
        }

        self.update_button_states()?;
        Ok(())
    }

    fn get_basic_input_from_channel(&self, channel: ChannelName) -> Option<BasicInputDevice> {
        match channel {
            ChannelName::Mic => Some(BasicInputDevice::Microphone),
            ChannelName::LineIn => Some(BasicInputDevice::LineIn),
            ChannelName::Console => Some(BasicInputDevice::Console),
            ChannelName::System => Some(BasicInputDevice::System),
            ChannelName::Game => Some(BasicInputDevice::Game),
            ChannelName::Chat => Some(BasicInputDevice::Chat),
            ChannelName::Sample => Some(BasicInputDevice::Samples),
            ChannelName::Music => Some(BasicInputDevice::Music),
            _ => None,
        }
    }

    async fn handle_swear_button(&mut self, press: bool) -> Result<()> {
        // Pretty simple, turn the light on when pressed, off when released..
        self.profile.set_swear_button_on(press)?;
        Ok(())
    }

    async fn load_sample_bank(&mut self, bank: SampleBank) -> Result<()> {
        self.profile.load_sample_bank(bank)?;

        // Sync the state of active playback..
        if let Some(audio) = &self.audio_handler {
            for button in SampleButtons::iter() {
                if audio.is_sample_playing(bank, button) {
                    self.profile.set_sample_button_state(button, true)?;
                }
            }
        }
        Ok(())
    }

    async fn handle_sample_button_down(&mut self, button: SampleButtons) -> Result<()> {
        debug!(
            "Handling Sample Button, clear state: {}",
            self.profile.is_sample_clear_active()
        );

        // We don't do anything if clear is flashing..
        if self.profile.is_sample_clear_active() {
            debug!("Sample Clear is Active, ignoring..");
            return Ok(());
        }

        if self.audio_handler.is_none() {
            return Err(anyhow!(
                "Not handling button, audio handler not configured."
            ));
        }

        // Grab the currently active bank..
        let sample_bank = self.profile.get_active_sample_bank();

        if !self.profile.current_sample_bank_has_samples(button) {
            let file_date = Local::now().format("%Y-%m-%dT%H%M%S").to_string();
            let full_name = format!("Recording_{file_date}.wav");

            self.record_audio_file(button, full_name).await?;
            return Ok(());
        }

        // Firstly, get the playback mode for this button..
        let mode = self.profile.get_sample_playback_mode(button);

        // Execute behaviour depending on mode, note that the 'fade' options aren't directly
        // supported, so we'll just map their equivalent 'Stop' action
        return match mode {
            SamplePlaybackMode::PlayNext
            | SamplePlaybackMode::StopOnRelease
            | SamplePlaybackMode::FadeOnRelease => {
                // In all three of these cases, we will always play audio on button down.
                //let file = self.profile.get_sample_file(button);
                let mut audio = self.profile.get_next_track(button)?;
                if mode == SamplePlaybackMode::FadeOnRelease {
                    audio.fade_on_stop = true;
                }
                self.play_audio_file(sample_bank, button, audio, false)
                    .await?;
                Ok(())
            }
            SamplePlaybackMode::PlayStop
            | SamplePlaybackMode::PlayFade
            | SamplePlaybackMode::Loop => {
                let audio_handler = self.audio_handler.as_mut().unwrap();
                // In these cases, we may be required to stop playback.
                if audio_handler.is_sample_playing(sample_bank, button)
                    && !audio_handler.is_sample_stopping(sample_bank, button)
                {
                    // Sample is playing, we need to stop it.
                    audio_handler
                        .stop_playback(sample_bank, button, false)
                        .await?;
                    Ok(())
                } else {
                    // Play the next file.
                    let mut audio = self.profile.get_next_track(button)?;

                    if mode == SamplePlaybackMode::PlayFade {
                        audio.fade_on_stop = true;
                    }

                    let loop_track = mode == SamplePlaybackMode::Loop;

                    self.play_audio_file(sample_bank, button, audio, loop_track)
                        .await?;
                    Ok(())
                }
            }
        };
    }

    async fn handle_sample_button_release(&mut self, button: SampleButtons) -> Result<()> {
        // If clear is flashing, remove all samples from the button, disable the clearer and return..
        if self.profile.is_sample_clear_active() {
            debug!("Sample Clear Active..");

            self.profile.clear_all_samples(button);

            debug!("Cleared samples..");
            self.profile.set_sample_clear_active(false)?;

            debug!("Disabled Buttons..");
            self.load_colour_map()?;

            debug!("Reset Colours");
            return Ok(());
        }

        // We only need to either a) Stop recording, or b) Handle Stop On Release..
        if self.audio_handler.is_none() {
            return Err(anyhow!(
                "Not handling button, audio handler not configured."
            ));
        }

        let sample_bank = self.profile.get_active_sample_bank();
        if !self.profile.current_sample_bank_has_samples(button) {
            if self
                .audio_handler
                .as_mut()
                .unwrap()
                .is_sample_recording(sample_bank, button)
            {
                let file_name = self
                    .audio_handler
                    .as_mut()
                    .unwrap()
                    .stop_record(sample_bank, button)?;

                // Stop flashing the button..
                self.profile.set_sample_button_blink(button, false)?;

                if let Some(file_name) = file_name {
                    self.profile.add_sample_file(sample_bank, button, file_name);

                    // Reload the Colour Map..
                    self.load_colour_map()?;
                }
            }
            return Ok(());
        }

        let mode = self.profile.get_sample_playback_mode(button);
        match mode {
            SamplePlaybackMode::StopOnRelease | SamplePlaybackMode::FadeOnRelease => {
                self.audio_handler
                    .as_mut()
                    .unwrap()
                    .stop_playback(sample_bank, button, false)
                    .await?;
                return Ok(());
            }
            _ => {}
        }

        Ok(())
    }

    /// A Simple Method that simply starts playback on the Sampler Channel..
    async fn play_audio_file(
        &mut self,
        bank: SampleBank,
        button: SampleButtons,
        mut audio: AudioFile,
        loop_track: bool,
    ) -> Result<()> {
        // Fill out the path..
        let sample_path = self.get_path_for_sample(audio.file).await?;
        audio.file = sample_path;

        if let Some(audio_handler) = &mut self.audio_handler {
            audio_handler.stop_playback(bank, button, true).await?;

            let result = audio_handler
                .play_for_button(bank, button, audio, loop_track)
                .await;

            if result.is_ok() {
                self.profile.set_sample_button_state(button, true)?;
            } else {
                error!("{}", result.err().unwrap());
            }
        }
        Ok(())
    }

    async fn stop_sample_playback(
        &mut self,
        bank: SampleBank,
        button: SampleButtons,
    ) -> Result<()> {
        if let Some(audio_handler) = &mut self.audio_handler {
            audio_handler.stop_playback(bank, button, false).await?;
        }

        Ok(())
    }

    async fn record_audio_file(&mut self, button: SampleButtons, file_name: String) -> Result<()> {
        let sample_bank = self.profile.get_active_sample_bank();

        // Create the full Path..
        let mut sample_path = self.settings.get_samples_directory().await;
        sample_path = sample_path.join("Recorded");
        sample_path = sample_path.join(file_name);

        if let Some(audio_handler) = &mut self.audio_handler {
            let result = audio_handler.record_for_button(sample_path, sample_bank, button);
            if result.is_ok() {
                self.profile.set_sample_button_blink(button, true)?;
            }
        }

        Ok(())
    }

    async fn get_path_for_sample(&mut self, part: PathBuf) -> Result<PathBuf> {
        let sample_path = self.settings.get_samples_directory().await;
        if let Some(file) = find_file_in_path(sample_path, part) {
            return Ok(file);
        }
        bail!("Sample Not Found");
    }

    async fn sync_sample_lighting(&mut self) -> Result<bool> {
        if self.audio_handler.is_none() {
            // No audio handler, no point.
            return Ok(false);
        }

        let mut changed = false;
        for button in SampleButtons::iter() {
            let playing = self
                .audio_handler
                .as_ref()
                .unwrap()
                .is_sample_playing(self.profile.get_active_sample_bank(), button);

            if self.profile.is_sample_active(button) && !playing {
                self.profile.set_sample_button_state(button, false)?;
                changed = true;
            }
        }

        if changed {
            self.update_button_states()?;
        }

        Ok(changed)
    }

    async fn load_effect_bank(&mut self, preset: EffectBankPresets) -> Result<()> {
        self.profile.load_effect_bank(preset)?;
        self.load_effects()?;
        self.set_pitch_mode()?;

        self.apply_effects(self.mic_profile.get_reverb_keyset())?;
        self.apply_effects(self.mic_profile.get_megaphone_keyset())?;
        self.apply_effects(self.mic_profile.get_robot_keyset())?;
        self.apply_effects(self.mic_profile.get_hardtune_keyset())?;
        self.apply_effects(self.mic_profile.get_echo_keyset())?;
        self.apply_effects(self.mic_profile.get_pitch_keyset())?;
        self.apply_effects(self.mic_profile.get_gender_keyset())?;

        Ok(())
    }

    async fn set_megaphone(&mut self, enabled: bool) -> Result<()> {
        self.profile.set_megaphone(enabled)?;
        self.apply_effects(LinkedHashSet::from_iter([EffectKey::MegaphoneEnabled]))?;
        Ok(())
    }

    async fn set_robot(&mut self, enabled: bool) -> Result<()> {
        self.profile.set_robot(enabled)?;
        self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotEnabled]))?;
        Ok(())
    }

    async fn set_hardtune(&mut self, enabled: bool) -> Result<()> {
        self.profile.set_hardtune(enabled)?;
        self.apply_effects(LinkedHashSet::from_iter([EffectKey::HardTuneEnabled]))?;
        self.set_pitch_mode()?;

        // When changing the Hard Tune amount, we need to update the pitch encoder..
        let pitch = self.profile.get_pitch_encoder_position();
        self.goxlr.set_encoder_value(EncoderName::Pitch, pitch)?;
        self.profile.set_pitch_knob_position(pitch)?;
        self.apply_effects(LinkedHashSet::from_iter([EffectKey::PitchAmount]))?;
        Ok(())
    }

    async fn set_effects(&mut self, enabled: bool) -> Result<()> {
        self.profile.set_effects(enabled)?;

        // When this changes, we need to update all the 'Enabled' keys..
        let mut key_updates = LinkedHashSet::new();
        key_updates.insert(EffectKey::Encoder1Enabled);
        key_updates.insert(EffectKey::Encoder2Enabled);
        key_updates.insert(EffectKey::Encoder3Enabled);
        key_updates.insert(EffectKey::Encoder4Enabled);

        key_updates.insert(EffectKey::MegaphoneEnabled);
        key_updates.insert(EffectKey::HardTuneEnabled);
        key_updates.insert(EffectKey::RobotEnabled);
        self.apply_effects(key_updates)?;

        Ok(())
    }

    fn mic_muted_by_fader(&self) -> bool {
        // Is the mute button even assigned to a fader?
        if self.profile.is_mic_on_fader() {
            let fader = self.profile.get_mic_fader();
            let (muted_to_x, muted_to_all, mute_function) =
                self.profile.get_mute_button_state(fader);

            return muted_to_all || (muted_to_x && mute_function == MuteFunction::All);
        }
        false
    }

    fn mic_muted_by_cough(&self) -> bool {
        let (_mute_toggle, muted_to_x, muted_to_all, mute_function) =
            self.profile.get_mute_chat_button_state();

        muted_to_all || (muted_to_x && mute_function == MuteFunction::All)
    }

    fn update_volumes_to(&mut self, volumes: [u8; 4]) -> Result<bool> {
        let mut value_changed = false;

        for fader in FaderName::iter() {
            let new_volume = volumes[fader as usize];
            if self.hardware.device_type == DeviceType::Mini
                && new_volume == self.fader_last_seen[fader]
            {
                continue;
            }
            self.fader_last_seen[fader] = new_volume;

            let channel = self.profile.get_fader_assignment(fader);
            let old_volume = self.profile.get_channel_volume(channel);

            if new_volume != old_volume {
                debug!(
                    "Updating {} volume from {} to {} as a human moved the fader",
                    channel, old_volume, new_volume
                );
                value_changed = true;
                self.profile.set_channel_volume(channel, new_volume)?;
            }
        }
        Ok(value_changed)
    }

    fn update_encoders_to(&mut self, encoders: [i8; 4]) -> Result<bool> {
        // Ok, this is funky, due to the way pitch works, the encoder 'value' doesn't match
        // the profile value if hardtune is enabled, so we'll pre-emptively calculate pitch here..
        let mut value_changed = false;

        if self.profile.calculate_pitch_knob_position(encoders[0])
            != self.profile.get_pitch_knob_position()
        {
            debug!(
                "Updating PITCH value from {} to {} as human moved the dial",
                self.profile.get_pitch_knob_position(),
                encoders[0]
            );
            value_changed = true;
            self.profile.set_pitch_knob_position(encoders[0])?;
            self.apply_effects(LinkedHashSet::from_iter([EffectKey::PitchAmount]))?;
        }

        if encoders[1] != self.profile.get_gender_value() {
            debug!(
                "Updating GENDER value from {} to {} as human moved the dial",
                self.profile.get_gender_value(),
                encoders[1]
            );
            value_changed = true;
            self.profile.set_gender_value(encoders[1])?;
            self.apply_effects(LinkedHashSet::from_iter([EffectKey::GenderAmount]))?;
        }

        if encoders[2] != self.profile.get_reverb_value() {
            debug!(
                "Updating REVERB value from {} to {} as human moved the dial",
                self.profile.get_reverb_value(),
                encoders[2]
            );
            value_changed = true;
            self.profile.set_reverb_value(encoders[2])?;
            self.apply_effects(LinkedHashSet::from_iter([EffectKey::ReverbAmount]))?;
        }

        if encoders[3] != self.profile.get_echo_value() {
            debug!(
                "Updating ECHO value from {} to {} as human moved the dial",
                self.profile.get_echo_value(),
                encoders[3]
            );
            value_changed = true;
            self.profile.set_echo_value(encoders[3])?;
            self.apply_effects(LinkedHashSet::from_iter([EffectKey::EchoAmount]))?;
        }

        Ok(value_changed)
    }

    pub async fn perform_command(&mut self, command: GoXLRCommand) -> Result<()> {
        match command {
            GoXLRCommand::SetShutdownCommands(commands) => {
                self.settings
                    .set_device_shutdown_commands(self.serial(), commands)
                    .await;
                self.settings.save().await;
            }
            GoXLRCommand::SetSamplerPreBufferDuration(duration) => {
                self.settings
                    .set_device_sampler_pre_buffer(self.serial(), duration)
                    .await;
                self.settings.save().await;
            }

            GoXLRCommand::SetFader(fader, channel) => {
                self.set_fader(fader, channel).await?;
            }
            GoXLRCommand::SetFaderMuteFunction(fader, behaviour) => {
                if self.profile.get_mute_button_behaviour(fader) == behaviour {
                    // Settings are the same..
                    return Ok(());
                }

                // Unmute the channel to prevent weirdness, then set new behaviour
                self.unmute_fader(fader).await?;
                self.profile.set_mute_button_behaviour(fader, behaviour);
            }

            GoXLRCommand::SetVolume(channel, volume) => {
                self.goxlr.set_volume(channel, volume)?;
                self.profile.set_channel_volume(channel, volume)?;
            }

            GoXLRCommand::SetCoughMuteFunction(mute_function) => {
                if self.profile.get_chat_mute_button_behaviour() == mute_function {
                    // Settings are the same..
                    return Ok(());
                }

                // Unmute the channel to prevent weirdness, then set new behaviour
                self.unmute_chat_if_muted().await?;
                self.profile.set_chat_mute_button_behaviour(mute_function);
            }
            GoXLRCommand::SetCoughIsHold(is_hold) => {
                self.unmute_chat_if_muted().await?;
                self.profile.set_chat_mute_button_is_held(is_hold);
            }
            GoXLRCommand::SetSwearButtonVolume(volume) => {
                self.mic_profile.set_bleep_level(volume)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::BleepLevel]))?;
                self.apply_mic_params(HashSet::from([MicrophoneParamKey::BleepLevel]))?;
            }
            GoXLRCommand::SetMicrophoneType(mic_type) => {
                self.mic_profile.set_mic_type(mic_type)?;
                self.apply_mic_gain()?;
            }
            GoXLRCommand::SetMicrophoneGain(mic_type, gain) => {
                self.mic_profile.set_mic_type(mic_type)?;
                self.mic_profile.set_mic_gain(mic_type, gain)?;
                self.apply_mic_gain()?;
            }
            GoXLRCommand::SetRouter(input, output, enabled) => {
                debug!("Setting Routing: {:?} {:?} {}", input, output, enabled);
                self.profile.set_routing(input, output, enabled);

                // Apply the change..
                self.apply_routing(input)?;
            }

            GoXLRCommand::SetElementDisplayMode(element, display) => match element {
                DisplayModeComponents::NoiseGate => {
                    self.mic_profile.set_gate_display_mode(display);
                }
                DisplayModeComponents::Equaliser => {
                    self.mic_profile.set_eq_display_mode(display);
                }
                DisplayModeComponents::Compressor => {
                    // TODO: Apply 'Simple' compressor values..
                    self.mic_profile.set_compressor_display_mode(display);
                }
                DisplayModeComponents::EqFineTune => {
                    self.mic_profile.set_eq_fine_display_mode(display);
                }
            },

            // Equaliser
            GoXLRCommand::SetEqMiniGain(gain, value) => {
                let param = self.mic_profile.set_mini_eq_gain(gain, value)?;
                self.apply_mic_params(HashSet::from([param]))?;
            }
            GoXLRCommand::SetEqMiniFreq(freq, value) => {
                let param = self.mic_profile.set_mini_eq_freq(freq, value)?;
                self.apply_mic_params(HashSet::from([param]))?;
            }
            GoXLRCommand::SetEqGain(gain, value) => {
                let param = self.mic_profile.set_eq_gain(gain, value)?;
                self.apply_effects(LinkedHashSet::from_iter([param]))?;
            }
            GoXLRCommand::SetEqFreq(freq, value) => {
                let param = self.mic_profile.set_eq_freq(freq, value)?;
                self.apply_effects(LinkedHashSet::from_iter([param]))?;
            }
            GoXLRCommand::SetGateThreshold(value) => {
                self.mic_profile.set_gate_threshold(value)?;
                self.apply_mic_params(HashSet::from([MicrophoneParamKey::GateThreshold]))?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::GateThreshold]))?;
            }

            // Noise Gate
            GoXLRCommand::SetGateAttenuation(percentage) => {
                self.mic_profile.set_gate_attenuation(percentage)?;
                self.apply_mic_params(HashSet::from([MicrophoneParamKey::GateAttenuation]))?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::GateAttenuation]))?;
            }
            GoXLRCommand::SetGateAttack(attack_time) => {
                self.mic_profile.set_gate_attack(attack_time)?;
                self.apply_mic_params(HashSet::from([MicrophoneParamKey::GateAttack]))?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::GateAttack]))?;
            }
            GoXLRCommand::SetGateRelease(release_time) => {
                self.mic_profile.set_gate_release(release_time)?;
                self.apply_mic_params(HashSet::from([MicrophoneParamKey::GateRelease]))?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::GateRelease]))?;
            }
            GoXLRCommand::SetGateActive(active) => {
                self.mic_profile.set_gate_active(active)?;

                // GateEnabled appears to only be an effect key.
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::GateEnabled]))?;
            }

            // Compressor
            GoXLRCommand::SetCompressorThreshold(value) => {
                self.mic_profile.set_compressor_threshold(value)?;
                self.apply_mic_params(HashSet::from([MicrophoneParamKey::CompressorThreshold]))?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::CompressorThreshold]))?;
            }
            GoXLRCommand::SetCompressorRatio(ratio) => {
                self.mic_profile.set_compressor_ratio(ratio)?;
                self.apply_mic_params(HashSet::from([MicrophoneParamKey::CompressorRatio]))?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::CompressorRatio]))?;
            }
            GoXLRCommand::SetCompressorAttack(value) => {
                self.mic_profile.set_compressor_attack(value)?;
                self.apply_mic_params(HashSet::from([MicrophoneParamKey::CompressorAttack]))?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::CompressorAttack]))?;
            }
            GoXLRCommand::SetCompressorReleaseTime(value) => {
                self.mic_profile.set_compressor_release(value)?;
                self.apply_mic_params(HashSet::from([MicrophoneParamKey::CompressorRelease]))?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::CompressorRelease]))?;
            }
            GoXLRCommand::SetCompressorMakeupGain(value) => {
                self.mic_profile.set_compressor_makeup(value)?;
                self.apply_mic_params(HashSet::from([MicrophoneParamKey::CompressorMakeUpGain]))?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::CompressorMakeUpGain]))?;
            }

            GoXLRCommand::SetDeeser(percentage) => {
                self.mic_profile.set_deesser(percentage)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::DeEsser]))?;
            }

            // Colouring..
            GoXLRCommand::SetFaderDisplayStyle(fader, display) => {
                self.profile.set_fader_display(fader, display)?;
                self.set_fader_display_from_profile(fader)?;
            }
            GoXLRCommand::SetFaderColours(fader, top, bottom) => {
                // Need to get the fader colour map, and set values..
                self.profile.set_fader_colours(fader, top, bottom)?;
                self.load_colour_map()?;
            }
            GoXLRCommand::SetAllFaderColours(top, bottom) => {
                // I considered this as part of SetFaderColours, but spamming a new colour map
                // for every fader change seemed excessive, this allows us to set them all before
                // reloading.
                for fader in FaderName::iter() {
                    self.profile
                        .set_fader_colours(fader, top.to_owned(), bottom.to_owned())?;
                }
                self.load_colour_map()?;
            }
            GoXLRCommand::SetAllFaderDisplayStyle(display_style) => {
                for fader in FaderName::iter() {
                    self.profile.set_fader_display(fader, display_style)?;
                    self.set_fader_display_from_profile(fader)?;
                }
            }
            GoXLRCommand::SetButtonColours(target, colour, colour2) => {
                self.profile
                    .set_button_colours(target, colour, colour2.as_ref())?;

                // Reload the colour map and button states..
                self.load_colour_map()?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetButtonOffStyle(target, off_style) => {
                self.profile.set_button_off_style(target, off_style)?;

                self.load_colour_map()?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetButtonGroupColours(target, colour, colour_2) => {
                self.profile
                    .set_group_button_colours(target, colour, colour_2)?;

                self.load_colour_map()?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetButtonGroupOffStyle(target, off_style) => {
                self.profile.set_group_button_off_style(target, off_style)?;
                self.load_colour_map()?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetSimpleColour(target, colour) => {
                self.profile.set_simple_colours(target, colour)?;
                self.load_colour_map()?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetEncoderColour(target, colour, colour_2, colour_3) => {
                self.profile
                    .set_encoder_colours(target, colour, colour_2, colour_3)?;
                self.load_colour_map()?;
            }
            GoXLRCommand::SetSampleColour(target, colour, colour_2, colour_3) => {
                self.profile
                    .set_sampler_colours(target, colour, colour_2, colour_3)?;
                self.profile.sync_sample_if_active(target)?;
                self.load_colour_map()?;
            }
            GoXLRCommand::SetSampleOffStyle(target, style) => {
                self.profile.set_sampler_off_style(target, style)?;
                self.load_colour_map()?;
                self.update_button_states()?;
            }

            // Effects
            GoXLRCommand::LoadEffectPreset(name) => {
                let presets_directory = self.settings.get_presets_directory().await;
                self.profile.load_preset(name, vec![&presets_directory])?;

                let current_effect_bank = self.profile.get_active_effect_bank();

                // Force a reload of this effect bank..
                // TODO: This is slightly sloppy, as it will make unneeded changes.
                // TODO: Loading a profile should be separate from an 'event'.
                self.load_effect_bank(current_effect_bank).await?;
                self.update_button_states()?;
            }

            GoXLRCommand::RenameActivePreset(name) => {
                let current_bank = self
                    .profile
                    .profile()
                    .settings()
                    .context()
                    .selected_effects();
                self.profile
                    .profile_mut()
                    .settings_mut()
                    .effects_mut(current_bank)
                    .set_name(name)?;
            }

            GoXLRCommand::SaveActivePreset() => {
                let preset_directory = self.settings.get_presets_directory().await;
                let current = self
                    .profile
                    .profile()
                    .settings()
                    .context()
                    .selected_effects();
                let mut name =
                    String::from(self.profile.profile().settings().effects(current).name());
                name = name.replace(' ', "_");

                self.profile.write_preset(name, &preset_directory)?;
            }

            // Reverb
            GoXLRCommand::SetReverbStyle(style) => {
                self.profile.set_reverb_style(style)?;
                self.apply_effects(self.mic_profile.get_reverb_keyset())?;
            }
            GoXLRCommand::SetReverbAmount(amount) => {
                self.profile
                    .get_active_reverb_profile_mut()
                    .set_percentage_amount(amount)?;

                let encoder_value = self.profile.get_reverb_value();
                self.goxlr
                    .set_encoder_value(EncoderName::Reverb, encoder_value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::ReverbAmount]))?;
            }
            GoXLRCommand::SetReverbDecay(value) => {
                self.profile
                    .get_active_reverb_profile_mut()
                    .set_decay_millis(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::ReverbDecay]))?;
            }
            GoXLRCommand::SetReverbEarlyLevel(value) => {
                self.profile
                    .get_active_reverb_profile_mut()
                    .set_early_level(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::ReverbEarlyLevel]))?;
            }
            GoXLRCommand::SetReverbTailLevel(value) => {
                self.profile
                    .get_active_reverb_profile_mut()
                    .set_tail_level(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::ReverbTailLevel]))?;
            }
            GoXLRCommand::SetReverbPreDelay(value) => {
                self.profile
                    .get_active_reverb_profile_mut()
                    .set_predelay(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::ReverbPredelay]))?;
            }
            GoXLRCommand::SetReverbLowColour(value) => {
                self.profile
                    .get_active_reverb_profile_mut()
                    .set_low_color(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::ReverbLowColor]))?;
            }
            GoXLRCommand::SetReverbHighColour(value) => {
                self.profile
                    .get_active_reverb_profile_mut()
                    .set_hi_color(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::ReverbHighColor]))?;
            }
            GoXLRCommand::SetReverbHighFactor(value) => {
                self.profile
                    .get_active_reverb_profile_mut()
                    .set_hi_factor(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::ReverbHighFactor]))?;
            }
            GoXLRCommand::SetReverbDiffuse(value) => {
                self.profile
                    .get_active_reverb_profile_mut()
                    .set_diffuse(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::ReverbDiffuse]))?;
            }
            GoXLRCommand::SetReverbModSpeed(value) => {
                self.profile
                    .get_active_reverb_profile_mut()
                    .set_mod_speed(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::ReverbModSpeed]))?;
            }
            GoXLRCommand::SetReverbModDepth(value) => {
                self.profile
                    .get_active_reverb_profile_mut()
                    .set_mod_depth(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::ReverbModDepth]))?;
            }

            // Echo..
            GoXLRCommand::SetEchoStyle(value) => {
                self.profile.set_echo_style(value)?;
                self.apply_effects(self.mic_profile.get_echo_keyset())?;
            }
            GoXLRCommand::SetEchoAmount(value) => {
                self.profile
                    .get_active_echo_profile_mut()
                    .set_percentage_value(value)?;

                let encoder_value = self.profile.get_echo_value();
                self.goxlr
                    .set_encoder_value(EncoderName::Echo, encoder_value)?;

                self.apply_effects(LinkedHashSet::from_iter([EffectKey::EchoAmount]))?;
            }
            GoXLRCommand::SetEchoFeedback(value) => {
                self.profile
                    .get_active_echo_profile_mut()
                    .set_feedback(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::EchoFeedback]))?;
            }
            GoXLRCommand::SetEchoTempo(value) => {
                self.profile
                    .get_active_echo_profile_mut()
                    .set_tempo(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::EchoTempo]))?;
            }
            GoXLRCommand::SetEchoDelayLeft(value) => {
                self.profile
                    .get_active_echo_profile_mut()
                    .set_time_left(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::EchoDelayL]))?;
            }
            GoXLRCommand::SetEchoDelayRight(value) => {
                self.profile
                    .get_active_echo_profile_mut()
                    .set_time_right(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::EchoDelayR]))?;
            }
            GoXLRCommand::SetEchoFeedbackLeft(value) => {
                self.profile
                    .get_active_echo_profile_mut()
                    .set_feedback_left(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::EchoFeedbackL]))?;
            }
            GoXLRCommand::SetEchoFeedbackRight(value) => {
                self.profile
                    .get_active_echo_profile_mut()
                    .set_feedback_right(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::EchoFeedbackR]))?;
            }
            GoXLRCommand::SetEchoFeedbackXFBRtoL(value) => {
                self.profile
                    .get_active_echo_profile_mut()
                    .set_xfb_r_to_l(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::EchoXFBRtoL]))?;
            }
            GoXLRCommand::SetEchoFeedbackXFBLtoR(value) => {
                self.profile
                    .get_active_echo_profile_mut()
                    .set_xfb_l_to_r(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::EchoXFBLtoR]))?;
            }

            // Pitch
            GoXLRCommand::SetPitchStyle(value) => {
                self.profile.set_pitch_style(value)?;
                self.set_pitch_mode()?;

                // Force set the encoder position, when going from Wide -> Narrow, the encoder
                // will still return it's 'Wide' value during polls which error out otherwise.
                let value = self.profile.get_pitch_encoder_position();
                self.goxlr.set_encoder_value(EncoderName::Pitch, value)?;
            }
            GoXLRCommand::SetPitchAmount(value) => {
                let hard_tune_enabled = self.profile.is_hardtune_enabled(true);
                self.profile
                    .get_active_pitch_profile_mut()
                    .set_knob_position(value, hard_tune_enabled)?;

                let value = self.profile.get_pitch_encoder_position();
                self.goxlr.set_encoder_value(EncoderName::Pitch, value)?;

                self.apply_effects(LinkedHashSet::from_iter([EffectKey::PitchAmount]))?;
            }
            GoXLRCommand::SetPitchCharacter(value) => {
                self.profile
                    .get_active_pitch_profile_mut()
                    .set_inst_ratio(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::PitchCharacter]))?;
            }

            // Gender
            GoXLRCommand::SetGenderStyle(value) => {
                self.profile.set_gender_style(value)?;
                self.apply_effects(self.mic_profile.get_gender_keyset())?;
            }
            GoXLRCommand::SetGenderAmount(value) => {
                self.profile
                    .get_active_gender_profile_mut()
                    .set_amount(value)?;
                let value = self.profile.get_gender_value();
                self.goxlr.set_encoder_value(EncoderName::Gender, value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::GenderAmount]))?;
            }

            GoXLRCommand::SetMegaphoneStyle(value) => {
                self.profile.set_megaphone_style(value)?;
                self.apply_effects(self.mic_profile.get_megaphone_keyset())?;
            }
            GoXLRCommand::SetMegaphoneAmount(value) => {
                self.profile
                    .get_active_megaphone_profile_mut()
                    .set_trans_dist_amt(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::MegaphoneAmount]))?;
            }
            GoXLRCommand::SetMegaphonePostGain(value) => {
                self.profile
                    .get_active_megaphone_profile_mut()
                    .set_trans_postgain(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::MegaphonePostGain]))?;
            }

            // Robot
            GoXLRCommand::SetRobotStyle(value) => {
                self.profile.set_robot_style(value)?;
                self.apply_effects(self.mic_profile.get_robot_keyset())?;
            }
            GoXLRCommand::SetRobotGain(range, value) => {
                let profile = self.profile.get_active_robot_profile_mut();
                match range {
                    RobotRange::Low => {
                        profile.set_vocoder_low_gain(value)?;
                        self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotLowGain]))?;
                    }
                    RobotRange::Medium => {
                        profile.set_vocoder_mid_gain(value)?;
                        self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotMidGain]))?;
                    }
                    RobotRange::High => {
                        profile.set_vocoder_high_gain(value)?;
                        self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotHiGain]))?;
                    }
                }
            }
            GoXLRCommand::SetRobotFreq(range, value) => {
                let profile = self.profile.get_active_robot_profile_mut();
                match range {
                    RobotRange::Low => {
                        profile.set_vocoder_low_freq(value)?;
                        self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotLowFreq]))?;
                    }
                    RobotRange::Medium => {
                        profile.set_vocoder_mid_freq(value)?;
                        self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotMidFreq]))?;
                    }
                    RobotRange::High => {
                        profile.set_vocoder_high_freq(value)?;
                        self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotHiFreq]))?;
                    }
                }
            }
            GoXLRCommand::SetRobotWidth(range, value) => {
                let profile = self.profile.get_active_robot_profile_mut();
                match range {
                    RobotRange::Low => {
                        profile.set_vocoder_low_bw(value)?;
                        self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotLowWidth]))?;
                    }
                    RobotRange::Medium => {
                        profile.set_vocoder_mid_bw(value)?;
                        self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotMidWidth]))?;
                    }
                    RobotRange::High => {
                        profile.set_vocoder_high_bw(value)?;
                        self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotHiWidth]))?;
                    }
                }
            }
            GoXLRCommand::SetRobotWaveform(value) => {
                self.profile
                    .get_active_robot_profile_mut()
                    .set_synthosc_waveform(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotWaveform]))?;
            }
            GoXLRCommand::SetRobotPulseWidth(value) => {
                self.profile
                    .get_active_robot_profile_mut()
                    .set_synthosc_pulse_width(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotPulseWidth]))?;
            }
            GoXLRCommand::SetRobotThreshold(value) => {
                self.profile
                    .get_active_robot_profile_mut()
                    .set_vocoder_gate_threshold(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotThreshold]))?;
            }
            GoXLRCommand::SetRobotDryMix(value) => {
                self.profile
                    .get_active_robot_profile_mut()
                    .set_dry_mix(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotDryMix]))?;
            }

            // Hard Tune
            GoXLRCommand::SetHardTuneStyle(value) => {
                self.profile.set_hardtune_style(value)?;
                self.apply_effects(self.mic_profile.get_hardtune_keyset())?;
            }
            GoXLRCommand::SetHardTuneAmount(value) => {
                self.profile
                    .get_active_hardtune_profile_mut()
                    .set_amount(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::HardTuneAmount]))?;
            }
            GoXLRCommand::SetHardTuneRate(value) => {
                self.profile
                    .get_active_hardtune_profile_mut()
                    .set_rate(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::HardTuneRate]))?;
            }
            GoXLRCommand::SetHardTuneWindow(value) => {
                self.profile
                    .get_active_hardtune_profile_mut()
                    .set_window(value)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::HardTuneWindow]))?;
            }
            GoXLRCommand::SetHardTuneSource(value) => {
                if self.profile.get_hardtune_source() == value {
                    // Do nothing, we're already there.
                    return Ok(());
                }

                // We need to update the Routing table to reflect this change..
                if value == HardTuneSource::All || self.profile.is_active_hardtune_source_all() {
                    self.profile.set_hardtune_source(value)?;

                    // One way or another, we need to update all the inputs..
                    self.apply_routing(BasicInputDevice::Music)?;
                    self.apply_routing(BasicInputDevice::Game)?;
                    self.apply_routing(BasicInputDevice::LineIn)?;
                    self.apply_routing(BasicInputDevice::System)?;
                } else {
                    let current = self.profile.get_active_hardtune_source();
                    self.profile.set_hardtune_source(value)?;
                    let new = self.profile.get_active_hardtune_source();

                    // Remove from current, add to New.
                    self.apply_routing(current)?;
                    self.apply_routing(new)?;
                }

                // TODO: Check this..
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::HardTuneKeySource]))?;
            }

            // Sampler..
            GoXLRCommand::SetSamplerFunction(bank, button, function) => {
                self.profile.set_sampler_function(bank, button, function);
            }
            GoXLRCommand::SetSamplerOrder(bank, button, order) => {
                self.profile.set_sampler_play_order(bank, button, order);
            }
            GoXLRCommand::AddSample(bank, button, filename) => {
                let path = self
                    .get_path_for_sample(PathBuf::from(filename.clone()))
                    .await?;

                // Add the Sample, and Grab the created track..
                let track = self.profile.add_sample_file(bank, button, filename);

                // If we have an audio handler, try to calcuate the Gain..
                if let Some(audio_handler) = &mut self.audio_handler {
                    // TODO: Find a way to do this asynchronously..
                    // Currently this will block the main thread until the calculation is complete,
                    // obviously this is less than ideal. We can't hold the track because it also
                    // needs to exist in the profile and could be removed prior to the calculation
                    // completing (causing Cross Thread Mutability issues), consider looking into
                    // refcounters to see if this can be solved.
                    //
                    // Bonus issue here, if too many commands are sent while this is happening, the
                    // entire daemon will lock up :D
                    if let Some(gain) = audio_handler.calculate_gain(&path)? {
                        // Gain was calculated, Apply it to the track..
                        track.normalized_gain = gain;
                    }
                }

                // Update the lighting..
                self.load_colour_map()?;
            }
            GoXLRCommand::SetSampleStartPercent(bank, button, index, percent) => {
                self.profile
                    .set_sample_start_pct(bank, button, index, percent)?;
            }
            GoXLRCommand::SetSampleStopPercent(bank, button, index, percent) => {
                self.profile
                    .set_sample_stop_pct(bank, button, index, percent)?;
            }
            GoXLRCommand::RemoveSampleByIndex(bank, button, index) => {
                let remaining = self
                    .profile
                    .remove_sample_file_by_index(bank, button, index);

                if remaining == 0 {
                    self.load_colour_map()?;
                }
            }
            GoXLRCommand::PlaySampleByIndex(bank, button, index) => {
                self.play_audio_file(
                    bank,
                    button,
                    self.profile.get_track_by_index(bank, button, index)?,
                    false,
                )
                .await?;
                self.update_button_states()?;
            }
            GoXLRCommand::PlayNextSample(bank, button) => {
                let track = self.profile.get_track_by_bank_button(bank, button)?;
                self.play_audio_file(bank, button, track, false).await?;
                self.update_button_states()?;
            }
            GoXLRCommand::StopSamplePlayback(bank, button) => {
                self.stop_sample_playback(bank, button).await?;
                self.update_button_states()?;
            }

            GoXLRCommand::SetScribbleIcon(fader, icon) => {
                self.profile.set_scribble_icon(fader, icon);
                self.apply_scribble(fader)?;
            }
            GoXLRCommand::SetScribbleText(fader, text) => {
                self.profile.set_scribble_text(fader, text);
                self.apply_scribble(fader)?;
            }
            GoXLRCommand::SetScribbleNumber(fader, number) => {
                self.profile.set_scribble_number(fader, number);
                self.apply_scribble(fader)?;
            }
            GoXLRCommand::SetScribbleInvert(fader, inverted) => {
                self.profile.set_scribble_inverted(fader, inverted);
                self.apply_scribble(fader)?;
            }

            // Profiles
            GoXLRCommand::NewProfile(profile_name) => {
                let profile_directory = self.settings.get_profile_directory().await;

                // Do a new file verification check..
                ProfileAdapter::can_create_new_file(profile_name.clone(), &profile_directory)?;

                // Force load the default embedded profile..
                self.profile = ProfileAdapter::default();
                self.apply_profile()?;

                // Save the profile under a new name (although, don't overwrite if exists!)
                self.profile
                    .write_profile(profile_name.clone(), &profile_directory, false)?;

                // Save the profile in the settings
                self.settings
                    .set_device_profile_name(self.serial(), profile_name.as_str())
                    .await;
                self.settings.save().await;
            }
            GoXLRCommand::LoadProfile(profile_name) => {
                let profile_directory = self.settings.get_profile_directory().await;

                self.profile = ProfileAdapter::from_named(profile_name, &profile_directory)?;
                self.apply_profile()?;
                self.settings
                    .set_device_profile_name(self.serial(), self.profile.name())
                    .await;
                self.settings.save().await;
            }
            GoXLRCommand::LoadProfileColours(profile_name) => {
                debug!("Loading Colours For Profile: {}", profile_name);
                let profile_directory = self.settings.get_profile_directory().await;
                let profile = ProfileAdapter::from_named(profile_name, &profile_directory)?;
                debug!("Profile Loaded, Applying Colours..");
                self.profile.load_colour_profile(profile);
                self.load_colour_map()?;
                self.update_button_states()?;
            }
            GoXLRCommand::SaveProfile() => {
                let profile_directory = self.settings.get_profile_directory().await;
                let profile_name = self.settings.get_device_profile_name(self.serial()).await;

                if let Some(profile_name) = profile_name {
                    self.profile
                        .write_profile(profile_name, &profile_directory, true)?;
                }
            }
            GoXLRCommand::SaveProfileAs(profile_name) => {
                let profile_directory = self.settings.get_profile_directory().await;

                // Do a new file verification check..
                ProfileAdapter::can_create_new_file(profile_name.clone(), &profile_directory)?;

                self.profile
                    .write_profile(profile_name.clone(), &profile_directory, false)?;

                // Save the new name in the settings
                self.settings
                    .set_device_profile_name(self.serial(), profile_name.as_str())
                    .await;

                self.settings.save().await;
            }
            GoXLRCommand::DeleteProfile(profile_name) => {
                let profile_directory = self.settings.get_profile_directory().await;
                self.profile
                    .delete_profile(profile_name.clone(), &profile_directory)?;
            }
            GoXLRCommand::NewMicProfile(mic_profile_name) => {
                let mic_profile_directory = self.settings.get_mic_profile_directory().await;

                // Verify we can create this file..
                MicProfileAdapter::can_create_new_file(
                    mic_profile_name.clone(),
                    &mic_profile_directory,
                )?;

                // As above, load the default profile, then save as a new profile.
                self.mic_profile = MicProfileAdapter::default();

                self.mic_profile.write_profile(
                    mic_profile_name.clone(),
                    &mic_profile_directory,
                    false,
                )?;

                // Save the new name in the settings
                self.settings
                    .set_device_mic_profile_name(self.serial(), mic_profile_name.as_str())
                    .await;

                self.settings.save().await;
            }
            GoXLRCommand::LoadMicProfile(mic_profile_name) => {
                let mic_profile_directory = self.settings.get_mic_profile_directory().await;
                self.mic_profile =
                    MicProfileAdapter::from_named(mic_profile_name, &mic_profile_directory)?;
                self.apply_mic_profile()?;
                self.settings
                    .set_device_mic_profile_name(self.serial(), self.mic_profile.name())
                    .await;
                self.settings.save().await;
            }
            GoXLRCommand::SaveMicProfile() => {
                let mic_profile_directory = self.settings.get_mic_profile_directory().await;
                let mic_profile_name = self
                    .settings
                    .get_device_mic_profile_name(self.serial())
                    .await;

                if let Some(profile_name) = mic_profile_name {
                    self.mic_profile
                        .write_profile(profile_name, &mic_profile_directory, true)?;
                }
            }
            GoXLRCommand::SaveMicProfileAs(profile_name) => {
                let profile_directory = self.settings.get_mic_profile_directory().await;

                MicProfileAdapter::can_create_new_file(profile_name.clone(), &profile_directory)?;

                self.mic_profile
                    .write_profile(profile_name.clone(), &profile_directory, false)?;

                // Save the new name in the settings
                self.settings
                    .set_device_mic_profile_name(self.serial(), profile_name.as_str())
                    .await;

                self.settings.save().await;
            }
            GoXLRCommand::DeleteMicProfile(profile_name) => {
                let profile_directory = self.settings.get_mic_profile_directory().await;
                self.mic_profile
                    .delete_profile(profile_name.clone(), &profile_directory)?;
            }

            GoXLRCommand::SetMuteHoldDuration(duration) => {
                self.hold_time = duration;
                self.settings
                    .set_device_mute_hold_duration(self.serial(), duration)
                    .await;
                self.settings.save().await;
            }

            GoXLRCommand::SetVCMuteAlsoMuteCM(value) => {
                self.vc_mute_also_mute_cm = value;
                self.settings
                    .set_device_vc_mute_also_mute_cm(self.serial(), value)
                    .await;
                self.settings.save().await;
            }

            GoXLRCommand::SetActiveEffectPreset(preset) => {
                self.load_effect_bank(preset).await?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetActiveSamplerBank(bank) => {
                self.load_sample_bank(bank).await?;
                self.load_colour_map()?;
            }
            GoXLRCommand::SetMegaphoneEnabled(enabled) => {
                self.set_megaphone(enabled).await?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetRobotEnabled(enabled) => {
                self.set_robot(enabled).await?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetHardTuneEnabled(enabled) => {
                self.set_hardtune(enabled).await?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetFXEnabled(enabled) => {
                self.set_effects(enabled).await?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetFaderMuteState(fader, state) => match state {
                MuteState::Unmuted => self.unmute_fader(fader).await?,
                MuteState::MutedToX => self.mute_fader_to_x(fader).await?,
                MuteState::MutedToAll => self.mute_fader_to_all(fader, true).await?,
            },
            GoXLRCommand::SetCoughMuteState(_state) => {}
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
    fn apply_channel_routing(
        &mut self,
        input: BasicInputDevice,
        router: EnumMap<BasicOutputDevice, bool>,
    ) -> Result<()> {
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

        // We need to handle hardtune configuration here as well..
        let hardtune_position = OutputDevice::HardTune.position();
        if self.profile.is_active_hardtune_source_all() {
            match input {
                BasicInputDevice::Music
                | BasicInputDevice::Game
                | BasicInputDevice::LineIn
                | BasicInputDevice::System => {
                    debug!("Light HardTune Enabled for Channel: {:?}", input);
                    left[hardtune_position] = 0x04;
                    right[hardtune_position] = 0x04;
                }
                _ => {}
            }
        } else {
            // We need to match only against a specific target..
            if input == self.profile.get_active_hardtune_source() {
                debug!("Hard HardTune Enabled for Channel: {:?}", input);
                left[hardtune_position] = 0x10;
                right[hardtune_position] = 0x10;
            }
        }

        self.goxlr.set_routing(left_input, left)?;
        self.goxlr.set_routing(right_input, right)?;

        Ok(())
    }

    fn apply_transient_routing(
        &self,
        input: BasicInputDevice,
        router: &mut EnumMap<BasicOutputDevice, bool>,
    ) -> Result<()> {
        // Not all channels are routable, so map the inputs to channels before checking..
        let channel_name = match input {
            BasicInputDevice::Microphone => ChannelName::Mic,
            BasicInputDevice::Chat => ChannelName::Chat,
            BasicInputDevice::Music => ChannelName::Music,
            BasicInputDevice::Game => ChannelName::Game,
            BasicInputDevice::Console => ChannelName::Console,
            BasicInputDevice::LineIn => ChannelName::LineIn,
            BasicInputDevice::System => ChannelName::System,
            BasicInputDevice::Samples => ChannelName::Sample,
        };

        for fader in FaderName::iter() {
            if self.profile.get_fader_assignment(fader) == channel_name {
                self.apply_transient_fader_routing(channel_name, fader, router)?;
            }
        }

        // Chat Mic has a Transient routing option related to the Voice Chat channel, we need
        // to ensure that if we're handling the mic, we handle it here.
        if channel_name == ChannelName::Mic {
            self.apply_transient_chat_mic_mute(router)?;
        }

        self.apply_transient_cough_routing(channel_name, router)
    }

    fn apply_transient_fader_routing(
        &self,
        channel_name: ChannelName,
        fader: FaderName,
        router: &mut EnumMap<BasicOutputDevice, bool>,
    ) -> Result<()> {
        let (muted_to_x, muted_to_all, mute_function) = self.profile.get_mute_button_state(fader);
        self.apply_transient_channel_routing(
            channel_name,
            muted_to_x,
            muted_to_all,
            mute_function,
            router,
        )
    }

    fn apply_transient_cough_routing(
        &self,
        channel_name: ChannelName,
        router: &mut EnumMap<BasicOutputDevice, bool>,
    ) -> Result<()> {
        // Same deal, pull out the current state, make needed changes.
        let (_mute_toggle, muted_to_x, muted_to_all, mute_function) =
            self.profile.get_mute_chat_button_state();

        self.apply_transient_channel_routing(
            channel_name,
            muted_to_x,
            muted_to_all,
            mute_function,
            router,
        )
    }

    fn apply_transient_chat_mic_mute(
        &self,
        router: &mut EnumMap<BasicOutputDevice, bool>,
    ) -> Result<()> {
        // Kinda annoying, try to locate the fader with the Chat Mic Assigned..
        for fader in FaderName::iter() {
            if self.profile.get_fader_assignment(fader) == ChannelName::Chat {
                // Get the Mute State..
                let (muted_to_x, muted_to_all, mute_function) =
                    self.profile.get_mute_button_state(fader);
                if muted_to_all || (muted_to_x && mute_function == MuteFunction::All) {
                    let mute_to_chat = self.vc_mute_also_mute_cm;
                    if mute_to_chat {
                        router[BasicOutputDevice::ChatMic] = false;
                    }
                }
            }
        }

        Ok(())
    }

    fn apply_transient_channel_routing(
        &self,
        channel_name: ChannelName,
        muted_to_x: bool,
        muted_to_all: bool,
        mute_function: MuteFunction,
        router: &mut EnumMap<BasicOutputDevice, bool>,
    ) -> Result<()> {
        if !muted_to_x || muted_to_all || mute_function == MuteFunction::All {
            if channel_name == ChannelName::Mic
                && (muted_to_all || (muted_to_x && mute_function == MuteFunction::All))
            {
                // In the case of the mic, if we're muted to all, we should drop routing to All other channels..
                router[BasicOutputDevice::Headphones] = false;
                router[BasicOutputDevice::ChatMic] = false;
                router[BasicOutputDevice::LineOut] = false;
                router[BasicOutputDevice::BroadcastMix] = false;
            }
            return Ok(());
        }

        match mute_function {
            MuteFunction::All => {}
            MuteFunction::ToStream => router[BasicOutputDevice::BroadcastMix] = false,
            MuteFunction::ToVoiceChat => router[BasicOutputDevice::ChatMic] = false,
            MuteFunction::ToPhones => router[BasicOutputDevice::Headphones] = false,
            MuteFunction::ToLineOut => router[BasicOutputDevice::LineOut] = false,
        };

        Ok(())
    }

    fn apply_routing(&mut self, input: BasicInputDevice) -> Result<()> {
        // Load the routing for this channel from the profile..
        let mut router = self.profile.get_router(input);
        self.apply_transient_routing(input, &mut router)?;
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
            return Ok(());
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
            return Ok(());
        }

        let muted_by_fader = if self.profile.is_mic_on_fader() {
            // We need to check this fader's mute button..
            let fader = self.profile.get_mic_fader();
            let (muted_to_x, muted_to_all, mute_function) =
                self.profile.get_mute_button_state(fader);
            muted_to_all || (muted_to_x && mute_function == MuteFunction::All)
        } else {
            false
        };

        if muted_to_all || (muted_to_x && mute_function == MuteFunction::All) || muted_by_fader {
            self.goxlr.set_channel_state(ChannelName::Mic, Muted)?;
        } else {
            self.goxlr.set_channel_state(ChannelName::Mic, Unmuted)?;
        }
        Ok(())
    }

    async fn set_fader(&mut self, fader: FaderName, new_channel: ChannelName) -> Result<()> {
        // A couple of things need to happen when a fader change occurs depending on scenario..
        if new_channel == self.profile.get_fader_assignment(fader) {
            // We don't need to do anything at all in theory, set the fader anyway..
            if new_channel == ChannelName::Mic {
                self.profile.set_mic_fader(fader)?;
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
            self.unmute_fader(fader).await?;

            // Check to see if we are dispatching of the mic channel, if so set the id.
            if existing_channel == ChannelName::Mic {
                self.profile.clear_mic_fader();
            }

            // Now set the new fader..
            self.profile.set_fader_assignment(fader, new_channel);
            self.goxlr.set_fader(fader, new_channel)?;

            // Due to motorised faders, the internal 'old' channel may be incorrectly set,
            // despite our config here being valid. So we'll force update the old channel.
            self.goxlr.set_volume(
                existing_channel,
                self.profile.get_channel_volume(existing_channel),
            )?;

            // Remember to update the button states after change..
            self.update_button_states()?;

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
            self.profile.set_mic_fader(fader)?;
        }

        if existing_channel == ChannelName::Mic {
            self.profile.set_mic_fader(fader_to_switch)?;
        }

        // Now switch the faders on the GoXLR..
        self.goxlr.set_fader(fader, new_channel)?;
        self.goxlr.set_fader(fader_to_switch, existing_channel)?;

        self.apply_scribble(fader)?;
        self.apply_scribble(fader_to_switch)?;

        // Finally update the button colours..
        self.update_button_states()?;

        Ok(())
    }

    fn get_fader_state(&self, fader: FaderName) -> FaderStatus {
        FaderStatus {
            channel: self.profile().get_fader_assignment(fader),
            mute_type: self.profile().get_mute_button_behaviour(fader),
            scribble: self
                .profile()
                .get_scribble_ipc(fader, self.hardware.device_type == DeviceType::Mini),
            mute_state: self.profile.get_ipc_mute_state(fader),
        }
    }

    fn set_fader_display_from_profile(&mut self, fader: FaderName) -> Result<()> {
        self.goxlr.set_fader_display_mode(
            fader,
            self.profile.is_fader_gradient(fader),
            self.profile.is_fader_meter(fader),
        )?;
        Ok(())
    }

    fn load_colour_map(&mut self) -> Result<()> {
        // The new colour format occurred on different firmware versions depending on device,
        // so do the check here.

        let use_1_3_40_format: bool = match self.hardware.device_type {
            DeviceType::Unknown => true,
            DeviceType::Full => version_newer_or_equal_to(
                &self.hardware.versions.firmware,
                VersionNumber(1, 3, 40, 0),
            ),
            DeviceType::Mini => version_newer_or_equal_to(
                &self.hardware.versions.firmware,
                VersionNumber(1, 1, 8, 0),
            ),
        };

        let colour_map = self.profile.get_colour_map(use_1_3_40_format);

        if use_1_3_40_format {
            self.goxlr.set_button_colours_1_3_40(colour_map)?;
        } else {
            let mut map: [u8; 328] = [0; 328];
            map.copy_from_slice(&colour_map[0..328]);
            self.goxlr.set_button_colours(map)?;
        }

        Ok(())
    }

    fn apply_profile(&mut self) -> Result<()> {
        // Set volumes first, applying mute may modify stuff..
        debug!("Applying Profile..");

        debug!("Setting Faders..");
        let mut mic_assigned_to_fader = false;

        // Prepare the faders, and configure channel mute states
        for fader in FaderName::iter() {
            let assignment = self.profile.get_fader_assignment(fader);

            debug!("Setting Fader {} to {:?}", fader, assignment);
            self.goxlr.set_fader(fader, assignment)?;

            // Force Mic Fader Assignment
            if assignment == ChannelName::Mic {
                mic_assigned_to_fader = true;
                self.profile.set_mic_fader(fader)?;
            }

            debug!("Applying Mute Profile for {}", fader);
            self.apply_mute_from_profile(fader)?;

            if self.hardware.device_type == DeviceType::Full {
                self.apply_scribble(fader)?;
            }
        }

        if !mic_assigned_to_fader {
            self.profile.clear_mic_fader();
        }

        debug!("Applying Cough button settings..");
        self.apply_cough_from_profile()?;

        debug!("Loading Colour Map..");
        self.load_colour_map()?;

        debug!("Setting Fader display modes..");
        for fader in FaderName::iter() {
            debug!("Setting display for {}", fader);
            self.set_fader_display_from_profile(fader)?;
        }

        debug!("Setting Channel Volumes..");
        for channel in ChannelName::iter() {
            let channel_volume = self.profile.get_channel_volume(channel);
            debug!("Setting volume for {} to {}", channel, channel_volume);
            self.goxlr.set_volume(channel, channel_volume)?;
        }

        debug!("Updating button states..");
        self.update_button_states()?;

        debug!("Applying Routing..");
        // For profile load, we should configure all the input channels from the profile,
        // this is split so we can do tweaks in places where needed.
        for input in BasicInputDevice::iter() {
            self.apply_routing(input)?;
        }

        Ok(())
    }

    /// Applies a Set of Microphone Parameters based on input, designed this way
    /// so that commands and other abstract entities can apply a subset of params
    fn apply_mic_params(&mut self, params: HashSet<MicrophoneParamKey>) -> Result<()> {
        let mut vec = Vec::new();
        for param in params {
            vec.push((param, self.mic_profile.get_param_value(param)));
        }
        self.goxlr.set_mic_param(vec.as_slice())?;
        Ok(())
    }

    fn apply_effects(&mut self, params: LinkedHashSet<EffectKey>) -> Result<()> {
        let mut vec = Vec::new();
        for effect in params {
            vec.push((
                effect,
                self.mic_profile.get_effect_value(effect, self.profile()),
            ));
        }

        for effect in &vec {
            let (key, value) = effect;
            debug!("Setting {:?} to {}", key, value);
        }
        self.goxlr.set_effect_values(vec.as_slice())?;
        Ok(())
    }

    fn apply_mic_gain(&mut self) -> Result<()> {
        let mic_type = self.mic_profile.mic_type();
        let gain = self.mic_profile.mic_gains()[mic_type];
        self.goxlr.set_microphone_gain(mic_type, gain)?;

        Ok(())
    }

    fn apply_mic_profile(&mut self) -> Result<()> {
        // Configure the microphone..
        self.apply_mic_gain()?;

        let mut keys = HashSet::new();
        for param in MicrophoneParamKey::iter() {
            keys.insert(param);
        }

        // Remove all gain settings, and re-add the relevant one.
        keys.remove(&MicrophoneParamKey::DynamicGain);
        keys.remove(&MicrophoneParamKey::CondenserGain);
        keys.remove(&MicrophoneParamKey::JackGain);
        keys.insert(self.mic_profile.mic_type().get_gain_param());
        self.apply_mic_params(keys)?;

        let mut keys = LinkedHashSet::new();
        keys.extend(self.mic_profile.get_common_keys());

        if self.hardware.device_type == DeviceType::Full {
            keys.extend(self.mic_profile.get_full_keys());
        }

        self.apply_effects(keys)?;

        if self.hardware.device_type == DeviceType::Full {
            self.set_pitch_mode()?;
            self.load_effects()?;
        }
        Ok(())
    }

    fn load_effects(&mut self) -> Result<()> {
        // For now, we'll simply set the knob positions, more to come!
        let mut value = self.profile.get_pitch_encoder_position();
        self.goxlr.set_encoder_value(EncoderName::Pitch, value)?;

        value = self.profile.get_echo_value();
        self.goxlr.set_encoder_value(EncoderName::Echo, value)?;

        value = self.profile.get_gender_value();
        self.goxlr.set_encoder_value(EncoderName::Gender, value)?;

        value = self.profile.get_reverb_value();
        self.goxlr.set_encoder_value(EncoderName::Reverb, value)?;

        Ok(())
    }

    fn apply_scribble(&mut self, fader: FaderName) -> Result<()> {
        let icon_path = block_on(self.settings.get_icons_directory());

        let scribble = self.profile.get_scribble_image(fader, &icon_path);
        self.goxlr.set_fader_scribble(fader, scribble)?;

        Ok(())
    }

    fn set_pitch_mode(&mut self) -> Result<()> {
        if self.hardware.device_type != DeviceType::Full {
            // Not a Full GoXLR, nothing to do.
            return Ok(());
        }

        self.goxlr.set_encoder_mode(
            EncoderName::Pitch,
            self.profile.get_pitch_mode(),
            self.profile.get_pitch_resolution(),
        )?;
        Ok(())
    }

    // Get the current time in millis..
    fn get_epoch_ms(&self) -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }
}
