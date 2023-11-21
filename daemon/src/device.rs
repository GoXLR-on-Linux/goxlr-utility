use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Result};
use chrono::Local;
use enum_map::EnumMap;
use enumset::EnumSet;
use log::{debug, error, info};
use ritelinked::LinkedHashSet;
use strum::IntoEnumIterator;
use tokio::sync::mpsc::Sender;

use goxlr_ipc::{
    Display, FaderStatus, GoXLRCommand, HardwareStatus, Levels, MicSettings, MixerStatus,
    SampleProcessState, Settings,
};
use goxlr_profile_loader::components::mute::MuteFunction;
use goxlr_types::{
    Button, ChannelName, DeviceType, DisplayModeComponents, EffectBankPresets, EffectKey,
    EncoderName, FaderName, HardTuneSource, InputDevice as BasicInputDevice, MicrophoneParamKey,
    Mix, MuteState, OutputDevice as BasicOutputDevice, RobotRange, SampleBank, SampleButtons,
    SamplePlaybackMode, VersionNumber, WaterfallDirection,
};
use goxlr_usb::animation::{AnimationMode, WaterFallDir};
use goxlr_usb::buttonstate::{ButtonStates, Buttons};
use goxlr_usb::channelstate::ChannelState;
use goxlr_usb::channelstate::ChannelState::{Muted, Unmuted};
use goxlr_usb::device::base::FullGoXLRDevice;
use goxlr_usb::routing::{InputDevice, OutputDevice};

use crate::audio::{AudioFile, AudioHandler};
use crate::events::EventTriggers;
use crate::events::EventTriggers::TTSMessage;
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
    fader_pause_until: EnumMap<FaderName, PauseUntil>,
    profile: ProfileAdapter,
    mic_profile: MicProfileAdapter,
    audio_handler: Option<AudioHandler>,
    hold_time: u16,
    vc_mute_also_mute_cm: bool,
    settings: &'a SettingsHandle,
    global_events: Sender<EventTriggers>,

    last_sample_error: Option<String>,
}

#[derive(Debug, Default, Copy, Clone)]
struct PauseUntil {
    paused: bool,
    until: u8,
}

#[derive(Debug, Default, Copy, Clone)]
struct ButtonState {
    press_time: u128,
    hold_handled: bool,
}

// Used when loading profiles to provide the previous
// profile's settings for comparison.
#[derive(Default)]
pub(crate) struct CurrentState {
    pub(crate) faders: EnumMap<FaderName, ChannelName>,
    pub(crate) mute_state: EnumMap<ChannelName, ChannelState>,
    pub(crate) volumes: EnumMap<ChannelName, u8>,
}

impl<'a> Device<'a> {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        goxlr: Box<dyn FullGoXLRDevice>,
        hardware: HardwareStatus,
        profile_name: Option<String>,
        mic_profile_name: Option<String>,
        profile_directory: &Path,
        mic_profile_directory: &Path,
        settings_handle: &'a SettingsHandle,
        global_events: Sender<EventTriggers>,
    ) -> Result<Device<'a>> {
        debug!("New Device Loading..");

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

        let mut audio_handler = None;
        if hardware.device_type == DeviceType::Full {
            let audio_buffer = settings_handle
                .get_device_sampler_pre_buffer(&hardware.serial_number)
                .await;
            let audio_loader = AudioHandler::new(audio_buffer);
            debug!("Created Audio Handler..");
            debug!("{:?}", audio_loader);

            if let Err(e) = &audio_loader {
                error!("Error Running Script: {}", e);
            }

            if let Ok(audio) = audio_loader {
                debug!("Audio Handler Loaded OK..");
                audio_handler.replace(audio);
            }
        } else {
            debug!("Not Spawning Audio Handler, Device is Mini!");
        }

        let hold_time = settings_handle
            .get_device_hold_time(&hardware.serial_number)
            .await;
        let vc_mute_also_mute_cm = settings_handle
            .get_device_chat_mute_mutes_mic_to_chat(&hardware.serial_number)
            .await;

        debug!("--- DEVICE INFO ---");
        debug!("Firmware: {:?}", hardware.versions.firmware);
        debug!("DICE: {:?}", hardware.versions.dice);
        debug!("Type: {:?}", hardware.device_type);
        debug!("-------------------");

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
            fader_pause_until: EnumMap::default(),
            audio_handler,
            settings: settings_handle,
            global_events,

            last_sample_error: None,
        };

        device.apply_profile(None).await?;
        device.apply_mic_profile().await?;

        Ok(device)
    }

    pub fn serial(&self) -> &str {
        &self.hardware.serial_number
    }

    pub async fn status(&self) -> MixerStatus {
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

        let shutdown_commands = self
            .settings
            .get_device_shutdown_commands(self.serial())
            .await;

        let sampler_prerecord = self
            .settings
            .get_device_sampler_pre_buffer(self.serial())
            .await;

        let monitor_with_fx = self
            .settings
            .get_enable_monitor_with_fx(self.serial())
            .await;

        let locked_faders = self.settings.get_device_lock_faders(self.serial()).await;

        let submix_supported = self.device_supports_submixes();

        let mut sample_progress = None;
        let mut sample_error = None;

        if let Some(audio_handler) = &self.audio_handler {
            if audio_handler.is_calculating() {
                if let Ok(value) = audio_handler.get_calculating_progress() {
                    sample_progress.replace(value);
                }
            }
        }

        if let Some(error) = &self.last_sample_error {
            sample_error.replace(error.clone());
        }

        let is_mini = self.hardware.device_type == DeviceType::Mini;

        MixerStatus {
            hardware: self.hardware.clone(),
            shutdown_commands,
            fader_status: fader_map,
            cough_button: self.profile.get_cough_status(),
            levels: Levels {
                submix_supported: self.device_supports_submixes(),
                output_monitor: self.profile.get_monitoring_mix(),
                volumes,
                submix: self.profile.get_submixes_ipc(submix_supported),
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
                .get_lighting_ipc(is_mini, self.device_supports_animations()),
            effects: self.profile.get_effects_ipc(is_mini),
            sampler: self.profile.get_sampler_ipc(
                is_mini,
                &self.audio_handler,
                sampler_prerecord,
                SampleProcessState {
                    progress: sample_progress,
                    last_error: sample_error,
                },
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
                enable_monitor_with_fx: monitor_with_fx,
                lock_faders: locked_faders,
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

    pub async fn sleep(&mut self) {
        debug!("Sleeping...");
    }

    pub async fn wake(&mut self) {
        debug!("Waking...");
    }

    pub fn profile(&self) -> &ProfileAdapter {
        &self.profile
    }

    pub fn mic_profile(&self) -> &MicProfileAdapter {
        &self.mic_profile
    }

    pub async fn update_state(&mut self) -> Result<bool> {
        let mut state_updated = false;
        let mut refresh_colour_map = false;

        // Update any audio related states..
        if let Some(audio_handler) = &mut self.audio_handler {
            // Check the status of any processing audio files..
            if audio_handler.is_calculating() && audio_handler.is_calculating_complete()? {
                // Handling has been finished, pull all the data and add it to the profile.

                let result = audio_handler.get_and_clear_calculating_result()?;
                if result.result.is_err() {
                    if let Err(error) = result.result {
                        // We need to somehow push this to the user (via DaemonStatus probably)..
                        self.last_sample_error = Some(error.to_string());
                    }
                } else {
                    let bank = result.bank;
                    let button = result.button;

                    let filename = result.file.file_name().unwrap();
                    let filename = filename.to_string_lossy().to_string();

                    let track = self.profile.add_sample_file(bank, button, filename);
                    track.normalized_gain = result.gain;

                    refresh_colour_map = true;
                }
                state_updated = true;
            }

            if audio_handler.is_calculating() {
                // We need to update the percentage in DaemonStatus
                debug!("Progress: {}", audio_handler.get_calculating_progress()?);
                state_updated = true;
            }

            if audio_handler.check_playing().await && !state_updated {
                state_updated = true;
            }

            if self.sync_sample_lighting().await? && !state_updated {
                state_updated = true;
            };

            if refresh_colour_map {
                self.load_colour_map().await?;
            }
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
        let mut changed = self.update_volumes_to(state.volumes).await?;
        let result = self.update_encoders_to(state.encoders).await?;
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
                self.load_colour_map().await?;
            }
            Buttons::SamplerSelectB => {
                self.load_sample_bank(SampleBank::B).await?;
                self.load_colour_map().await?;
            }
            Buttons::SamplerSelectC => {
                self.load_sample_bank(SampleBank::C).await?;
                self.load_colour_map().await?;
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
                self.handle_sample_clear().await?;
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

        let target = tts_target(mute_function);
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
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::MicInputMute]))?;
            }

            let message = format!("Mic Muted{}", target);
            let _ = self.global_events.send(TTSMessage(message)).await;

            self.apply_routing(BasicInputDevice::Microphone).await?;
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

            let message = "Mic Muted".to_string();
            let _ = self.global_events.send(TTSMessage(message)).await;

            self.goxlr.set_channel_state(ChannelName::Mic, Muted)?;
            self.apply_effects(LinkedHashSet::from_iter([EffectKey::MicInputMute]))?;
            self.apply_routing(BasicInputDevice::Microphone).await?;
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
                        self.apply_effects(LinkedHashSet::from_iter([EffectKey::MicInputMute]))?;
                    }

                    let message = "Mic Unmuted".to_string();
                    let _ = self.global_events.send(TTSMessage(message)).await;
                    self.apply_routing(BasicInputDevice::Microphone).await?;
                    return Ok(());
                }

                // In all cases, enable the button
                self.profile.set_mute_chat_button_on(true);

                if mute_function == MuteFunction::All {
                    self.goxlr.set_channel_state(ChannelName::Mic, Muted)?;
                    self.apply_effects(LinkedHashSet::from_iter([EffectKey::MicInputMute]))?;
                }

                let message = format!("Mic Muted{}", target);
                let _ = self.global_events.send(TTSMessage(message)).await;

                // Update the transient routing..
                self.apply_routing(BasicInputDevice::Microphone).await?;
                return Ok(());
            }

            self.profile.set_mute_chat_button_on(false);
            if mute_function == MuteFunction::All && !self.mic_muted_by_fader() {
                self.goxlr.set_channel_state(ChannelName::Mic, Unmuted)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::MicInputMute]))?;
            }

            let message = "Mic Unmuted".to_string();
            let _ = self.global_events.send(TTSMessage(message)).await;

            // Disable button and refresh transient routing
            self.apply_routing(BasicInputDevice::Microphone).await?;
            return Ok(());
        }

        Ok(())
    }

    async fn mute_fader_to_x(&mut self, fader: FaderName) -> Result<()> {
        let (muted_to_x, muted_to_all, mute_function) = self.profile.get_mute_button_state(fader);
        let target = tts_target(mute_function);

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

        // Ok, we need to announce where we're muted to..
        let name = self.profile.get_fader_assignment(fader);
        let message = format!("{} Muted{}", name, target);
        let _ = self.global_events.send(TTSMessage(message)).await;

        let input = self.get_basic_input_from_channel(channel);
        self.profile.set_mute_button_on(fader, true)?;
        if input.is_some() {
            self.apply_routing(input.unwrap()).await?;
        }
        self.update_button_states()?;
        Ok(())
    }

    async fn mute_fader_to_all(&mut self, fader: FaderName, blink: bool) -> Result<()> {
        let (muted_to_x, muted_to_all, mute_function) = self.profile.get_mute_button_state(fader);
        let channel = self.profile.get_fader_assignment(fader);
        let lock_faders = self.settings.get_device_lock_faders(self.serial()).await;

        // Are we already muted to all?
        if muted_to_all {
            return Ok(());
        }

        // If we did this on Mute to X, we don't need to do it again..
        if !(muted_to_x && mute_function == MuteFunction::All) {
            let volume = self.profile.get_channel_volume(channel);

            // Per the latest official release, the mini no longer sets the volume to 0 on mute
            if self.hardware.device_type != DeviceType::Mini {
                // We need to set the previous volume regardless, because if the below setting
                // changes, we need to correctly reset the position.
                self.profile.set_mute_previous_volume(fader, volume)?;

                if !lock_faders {
                    // User has asked us not to move the volume,
                    self.goxlr.set_volume(channel, 0)?;
                }
            }
            self.goxlr.set_channel_state(channel, Muted)?;
            self.profile.set_mute_button_on(fader, true)?;
        }

        let name = self.profile.get_fader_assignment(fader);
        let message = format!("{} Muted", name);
        let _ = self.global_events.send(TTSMessage(message)).await;

        if blink {
            self.profile.set_mute_button_blink(fader, true)?;
        }

        if self.hardware.device_type != DeviceType::Mini && !lock_faders {
            // Again, only apply this if we're a full device
            self.profile.set_channel_volume(channel, 0)?;
        } else {
            // Reload the colour map on the mini (will disable fader lighting)
            self.load_colour_map().await?;
        }

        // If we're Chat, we may need to transiently route the Microphone..
        if channel == ChannelName::Chat {
            self.apply_routing(BasicInputDevice::Microphone).await?;
        }

        if channel == ChannelName::Mic {
            self.apply_routing(BasicInputDevice::Microphone).await?;
        }

        self.update_button_states()?;
        Ok(())
    }

    async fn unmute_fader(&mut self, fader: FaderName) -> Result<()> {
        let (muted_to_x, muted_to_all, mute_function) = self.profile.get_mute_button_state(fader);
        let channel = self.profile.get_fader_assignment(fader);
        let lock_faders = self.settings.get_device_lock_faders(self.serial()).await;

        if !muted_to_x && !muted_to_all {
            // Nothing to do.
            return Ok(());
        }

        // Disable the lighting regardless of action
        self.profile.set_mute_button_on(fader, false)?;
        self.profile.set_mute_button_blink(fader, false)?;

        if muted_to_all || mute_function == MuteFunction::All {
            // This fader has previously been 'Muted to All', we need to restore the volume..
            let previous_volume = self.profile.get_mute_button_previous_volume(fader);

            if channel != ChannelName::Mic
                || (channel == ChannelName::Mic && !self.mic_muted_by_cough())
            {
                self.goxlr.set_channel_state(channel, Unmuted)?;
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::MicInputMute]))?;
            }

            // As with mute, the mini doesn't modify volumes on mute / unmute
            if self.hardware.device_type != DeviceType::Mini && !lock_faders {
                self.goxlr.set_volume(channel, previous_volume)?;
                self.profile.set_channel_volume(channel, previous_volume)?;
            } else {
                if self.needs_submix_correction(channel) {
                    // This is a special case, when calling unmute on submix firmware, the LineOut
                    // and Headphones don't set correctly, so we need to forcibly restore the
                    // volume. This does mean unlatching though :(
                    let current_volume = self.profile.get_channel_volume(channel);
                    self.goxlr.set_volume(channel, current_volume)?;
                }

                // Reload the Minis colour Map to re-establish colours.
                self.load_colour_map().await?;
            }

            // As before, we might need transient Mic Routing..
            if channel == ChannelName::Chat {
                self.apply_routing(BasicInputDevice::Microphone).await?;
            }

            if channel == ChannelName::Mic {
                self.apply_routing(BasicInputDevice::Microphone).await?;
            }
        }

        // Always do a Transient Routing update, just in case we went from Mute to X -> Mute to All
        let input = self.get_basic_input_from_channel(channel);
        if mute_function != MuteFunction::All && input.is_some() {
            self.apply_routing(input.unwrap()).await?;
        }

        let name = self.profile.get_fader_assignment(fader);
        let message = format!("{} unmuted", name);
        let _ = self.global_events.send(TTSMessage(message)).await;

        self.update_button_states()?;
        Ok(())
    }

    fn lock_faders(&mut self) -> Result<()> {
        if self.hardware.device_type == DeviceType::Mini {
            return Ok(());
        }

        for fader in FaderName::iter() {
            if self.profile.get_fader_mute_state(fader) == Muted {
                // Ok, to lock the fader, we need to restore this to it's stored value..
                let volume = self.profile.get_mute_button_previous_volume(fader);
                let channel = self.profile.get_fader_assignment(fader);

                // Set the volume of the channel back to where it should be
                self.goxlr.set_volume(channel, volume)?;
            }
        }
        Ok(())
    }

    fn unlock_faders(&mut self) -> Result<()> {
        if self.hardware.device_type == DeviceType::Mini {
            return Ok(());
        }

        // We need to drop any muted faders to 0 volume..
        for fader in FaderName::iter() {
            if self.profile.get_fader_mute_state(fader) == Muted {
                // Get the current volume for the fader..
                let channel = self.profile.get_fader_assignment(fader);
                let volume = self.profile.get_channel_volume(channel);

                // Set the previous volume
                self.profile.set_mute_previous_volume(fader, volume)?;

                // Set the volume of the channel to 0
                self.goxlr.set_volume(channel, 0)?;
            }
        }

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
        // Send the TTS Message..
        let tts_message = format!("Sample {}", bank);
        let _ = self.global_events.send(TTSMessage(tts_message)).await;

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

    pub async fn validate_sampler(&mut self) -> Result<()> {
        let sample_path = self.settings.get_samples_directory().await;
        for bank in SampleBank::iter() {
            for button in SampleButtons::iter() {
                let tracks = self.profile.get_sample_bank(bank, button);
                tracks.retain(|track| {
                    let file = PathBuf::from(track.track.clone());

                    // Simply, if this returns None, the file isn't present.
                    find_file_in_path(sample_path.clone(), file).is_some()
                });
            }
        }

        // Because we may have removed the 'last' sample on a button, we need to refresh
        // the states to make sure everything is correctly updated.
        self.load_colour_map().await?;
        self.update_button_states()
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

    async fn stop_all_samples(&mut self) -> Result<()> {
        if let Some(audio) = &mut self.audio_handler {
            for bank in SampleBank::iter() {
                for button in SampleButtons::iter() {
                    if audio.is_sample_playing(bank, button) {
                        audio.stop_playback(bank, button, true).await?;
                        self.profile.set_sample_button_state(button, false)?;
                    }
                    if audio.sample_recording(bank, button) {
                        audio.stop_record(bank, button)?;
                        self.profile.set_sample_button_blink(button, false)?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_sample_clear(&mut self) -> Result<()> {
        if let Some(audio) = &self.audio_handler {
            let state = self.profile.is_sample_clear_active();
            if !audio.is_sample_recording() {
                let message = format!("Sample Clear {}", tts_bool_to_state(!state));
                self.global_events.send(TTSMessage(message)).await?;

                self.profile.set_sample_clear_active(!state)?;
            }
        }
        Ok(())
    }

    async fn handle_sample_button_release(&mut self, button: SampleButtons) -> Result<()> {
        let active_bank = self.profile.get_active_sample_bank();
        // If clear is flashing, remove all samples from the button, disable the clearer and return..
        if self.profile.is_sample_clear_active() {
            debug!("Stopping any playing samples..");
            if let Some(handler) = &mut self.audio_handler {
                // Force stop of anything playing back on this button.
                handler.stop_playback(active_bank, button, true).await?;
            }

            debug!("Clearing Samples on Button..");
            self.profile.clear_all_samples(button);

            debug!("Cleared samples..");
            self.profile.set_sample_clear_active(false)?;

            debug!("Disabled Buttons..");
            self.load_colour_map().await?;

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
                .sample_recording(sample_bank, button)
            {
                let file_name = self
                    .audio_handler
                    .as_mut()
                    .unwrap()
                    .stop_record(sample_bank, button)?;

                if let Some(file_name) = file_name {
                    self.profile.add_sample_file(sample_bank, button, file_name);
                }
            }
            // In all cases, we should stop the colour flashing.
            self.profile.set_sample_button_blink(button, false)?;
            self.load_colour_map().await?;

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
        // Send the TTS Message..
        let preset_name = self.profile.get_effect_name(preset);
        let tts_message = format!("Effects {}, {}", preset as u8 + 1, preset_name);
        let _ = self.global_events.send(TTSMessage(tts_message)).await;

        self.profile.load_effect_bank(preset)?;
        self.load_encoder_effects()?;
        self.set_pitch_mode()?;

        self.apply_effects(self.mic_profile.get_fx_keys())?;

        Ok(())
    }

    async fn set_megaphone(&mut self, enabled: bool) -> Result<()> {
        // Send the TTS Message..
        let tts_message = format!("Megaphone {}", tts_bool_to_state(enabled));
        let _ = self.global_events.send(TTSMessage(tts_message)).await;

        self.profile.set_megaphone(enabled)?;
        self.apply_effects(LinkedHashSet::from_iter([EffectKey::MegaphoneEnabled]))?;
        Ok(())
    }

    async fn set_robot(&mut self, enabled: bool) -> Result<()> {
        // Send the TTS Message..
        let tts_message = format!("Robot {}", tts_bool_to_state(enabled));
        let _ = self.global_events.send(TTSMessage(tts_message)).await;

        self.profile.set_robot(enabled)?;
        self.apply_effects(LinkedHashSet::from_iter([EffectKey::RobotEnabled]))?;
        Ok(())
    }

    async fn set_hardtune(&mut self, enabled: bool) -> Result<()> {
        // Send the TTS Message..
        let tts_message = format!("Hard tune {}", tts_bool_to_state(enabled));
        let _ = self.global_events.send(TTSMessage(tts_message)).await;

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
        // Send the TTS Message..
        let tts_message = format!("Effects {}", tts_bool_to_state(enabled));
        let _ = self.global_events.send(TTSMessage(tts_message)).await;

        self.profile.set_effects(enabled)?;

        // When this changes, we need to update all the 'Enabled' keys..
        self.apply_effects(self.mic_profile.get_enabled_keyset())?;

        // Re-apply routing to the Mic in case monitoring needs to be enabled / disabled..
        self.apply_routing(BasicInputDevice::Microphone).await?;

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

    async fn update_volumes_to(&mut self, volumes: [u8; 4]) -> Result<bool> {
        let mut value_changed = false;

        for fader in FaderName::iter() {
            let new_volume = volumes[fader as usize];
            if self.hardware.device_type == DeviceType::Mini {
                if new_volume == self.fader_last_seen[fader] {
                    continue;
                }
            } else if self.fader_pause_until[fader].paused {
                let until = self.fader_pause_until[fader].until;

                // Calculate min and max, make sure we don't overflow..
                let min = match until < 5 {
                    true => 0,
                    false => until - 5,
                };

                let max = match until > 250 {
                    true => 255,
                    false => until + 5,
                };

                // Are we in this range?
                if !((min)..=(max)).contains(&new_volume) {
                    continue;
                } else {
                    self.fader_pause_until[fader].paused = false;
                }
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

                // Update the Submix..
                self.update_submix_for(channel, new_volume)?;
            }
        }
        Ok(value_changed)
    }

    fn update_submix_for(&mut self, channel: ChannelName, volume: u8) -> Result<()> {
        if self.device_supports_submixes() && self.profile.is_submix_enabled() {
            if let Some(mix) = self.profile.get_submix_from_channel(channel) {
                if !self.profile.submix_linked(mix) {
                    return Ok(());
                }

                let mix_current_volume = self.profile.get_submix_volume(mix);
                let ratio = self.profile.get_submix_ratio(mix);

                let linked_volume = (volume as f64 * ratio) as u8;

                if linked_volume != mix_current_volume {
                    self.profile.set_submix_volume(mix, linked_volume)?;
                    self.goxlr.set_sub_volume(mix, linked_volume)?;
                }
            }
        }
        Ok(())
    }

    async fn update_encoders_to(&mut self, encoders: [i8; 4]) -> Result<bool> {
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

            let user_value = self
                .mic_profile
                .get_effect_value(EffectKey::PitchAmount, self.profile());
            let message = format!("Pitch {}", user_value);
            let _ = self.global_events.send(TTSMessage(message)).await;
        }

        if encoders[1] != self.profile.get_gender_value() {
            debug!(
                "Updating GENDER value from {} to {} as human moved the dial",
                self.profile.get_gender_value(),
                encoders[1]
            );

            let current_value = self
                .mic_profile
                .get_effect_value(EffectKey::GenderAmount, self.profile());

            self.profile.set_gender_value(encoders[1])?;
            value_changed = true;

            let new_value = self
                .mic_profile
                .get_effect_value(EffectKey::GenderAmount, self.profile());

            if new_value != current_value {
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::GenderAmount]))?;
                let message = format!("Gender {}", new_value);
                let _ = self.global_events.send(TTSMessage(message)).await;
            }
        }

        if encoders[2] != self.profile.get_reverb_value() {
            debug!(
                "Updating REVERB value from {} to {} as human moved the dial",
                self.profile.get_reverb_value(),
                encoders[2]
            );

            value_changed = true;
            self.profile.set_reverb_value(encoders[2])?;

            let new_value = self
                .mic_profile
                .get_effect_value(EffectKey::ReverbAmount, self.profile());

            self.apply_effects(LinkedHashSet::from_iter([EffectKey::ReverbAmount]))?;

            let percent = 100 - ((new_value as f32 / -36.) * 100.) as i32;
            let message = format!("Reverb {} percent", percent);
            let _ = self.global_events.send(TTSMessage(message)).await;
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

            let mut user_value = self
                .mic_profile
                .get_effect_value(EffectKey::EchoAmount, self.profile());
            user_value = 100 - ((user_value as f32 / -36.) * 100.) as i32;
            let message = format!("Echo {} percent", user_value);
            let _ = self.global_events.send(TTSMessage(message)).await;
        }

        Ok(value_changed)
    }

    pub async fn get_mic_level(&mut self) -> Result<f64> {
        let level = self.goxlr.get_microphone_level()?;

        let db = ((f64::log(level.into(), 10.) * 20.) - 72.2).clamp(-72.2, 0.);
        Ok(db)
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
                if duration > 30000 {
                    bail!("Buffer must be below 30seconds");
                }

                self.settings
                    .set_device_sampler_pre_buffer(self.serial(), duration)
                    .await;
                self.settings.save().await;

                // Reload the Audio Handler..
                self.stop_all_samples().await?;

                // Drop the Audio Handler..
                let new_handler = AudioHandler::new(duration)?;
                self.audio_handler = Some(new_handler);
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

                // Update the Submix when volume changes via IPC
                self.update_submix_for(channel, volume)?;

                if let Some(fader) = self.profile.get_fader_from_channel(channel) {
                    self.fader_pause_until[fader].paused = true;
                    self.fader_pause_until[fader].until = volume;
                }
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
                self.profile.set_routing(input, output, enabled)?;

                // Apply the change..
                self.apply_routing(input).await?;
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
            GoXLRCommand::SetAnimationMode(mode) => {
                if !self.device_supports_animations() {
                    bail!("Animations not supported on this firmware.");
                }

                if mode == goxlr_types::AnimationMode::Ripple
                    && self.hardware.device_type == DeviceType::Mini
                {
                    bail!("Ripple Mode not supported on the GoXLR Mini");
                }

                self.profile.set_animation_mode(mode)?;
                self.load_animation(false).await?;
            }
            GoXLRCommand::SetAnimationMod1(value) => {
                if !self.device_supports_animations() {
                    bail!("Animations not supported on this firmware.");
                }

                self.profile.set_animation_mod1(value)?;
                self.load_animation(false).await?;
            }
            GoXLRCommand::SetAnimationMod2(value) => {
                if !self.device_supports_animations() {
                    bail!("Animations not supported on this firmware.");
                }

                self.profile.set_animation_mod2(value)?;
                self.load_animation(false).await?;
            }
            GoXLRCommand::SetAnimationWaterfall(direction) => {
                if !self.device_supports_animations() {
                    bail!("Animations not supported on this firmware.");
                }

                self.profile.set_animation_waterfall(direction)?;
                self.load_animation(false).await?;
            }

            GoXLRCommand::SetGlobalColour(colour) => {
                self.profile.set_global_colour(colour)?;
                self.load_colour_map().await?;
                self.update_button_states()?;
                self.set_all_fader_display_from_profile()?;
            }
            GoXLRCommand::SetFaderDisplayStyle(fader, display) => {
                self.profile.set_fader_display(fader, display)?;
                self.set_fader_display_from_profile(fader)?;
            }
            GoXLRCommand::SetFaderColours(fader, top, bottom) => {
                // Need to get the fader colour map, and set values..
                self.profile.set_fader_colours(fader, top, bottom)?;
                self.load_colour_map().await?;
            }
            GoXLRCommand::SetAllFaderColours(top, bottom) => {
                // I considered this as part of SetFaderColours, but spamming a new colour map
                // for every fader change seemed excessive, this allows us to set them all before
                // reloading.
                for fader in FaderName::iter() {
                    self.profile
                        .set_fader_colours(fader, top.to_owned(), bottom.to_owned())?;
                }
                self.load_colour_map().await?;
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
                self.load_colour_map().await?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetButtonOffStyle(target, off_style) => {
                self.profile.set_button_off_style(target, off_style)?;

                self.load_colour_map().await?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetButtonGroupColours(target, colour, colour_2) => {
                self.profile
                    .set_group_button_colours(target, colour, colour_2)?;

                self.load_colour_map().await?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetButtonGroupOffStyle(target, off_style) => {
                self.profile.set_group_button_off_style(target, off_style)?;
                self.load_colour_map().await?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetSimpleColour(target, colour) => {
                self.profile.set_simple_colours(target, colour)?;
                self.load_colour_map().await?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetEncoderColour(target, colour, colour_2, colour_3) => {
                self.profile
                    .set_encoder_colours(target, colour, colour_2, colour_3)?;
                self.load_colour_map().await?;
            }
            GoXLRCommand::SetSampleColour(target, colour, colour_2, colour_3) => {
                self.profile
                    .set_sampler_colours(target, colour, colour_2, colour_3)?;
                self.profile.sync_sample_if_active(target)?;
                self.load_colour_map().await?;
            }
            GoXLRCommand::SetSampleOffStyle(target, style) => {
                self.profile.set_sampler_off_style(target, style)?;
                self.load_colour_map().await?;
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

                // Update the pitch 'Threshold' value which may have changed..
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::PitchThreshold]))?;
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
                    self.apply_routing(BasicInputDevice::Music).await?;
                    self.apply_routing(BasicInputDevice::Game).await?;
                    self.apply_routing(BasicInputDevice::LineIn).await?;
                    self.apply_routing(BasicInputDevice::System).await?;
                } else {
                    let current = self.profile.get_active_hardtune_source();
                    self.profile.set_hardtune_source(value)?;
                    let new = self.profile.get_active_hardtune_source();

                    // Remove from current, add to New.
                    self.apply_routing(current).await?;
                    self.apply_routing(new).await?;
                }

                // TODO: Check this..
                self.apply_effects(LinkedHashSet::from_iter([EffectKey::HardTuneKeySource]))?;
            }

            // Sampler..
            GoXLRCommand::ClearSampleProcessError() => {
                self.last_sample_error = None;
            }
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

                // If we have an audio handler, try to calcuate the Gain..
                if let Some(audio_handler) = &mut self.audio_handler {
                    if audio_handler.is_calculating() {
                        bail!("Gain Calculation already in progress..");
                    }

                    // V2 Here, this technically still blocks in it's current state, however, it
                    // doesn't have to anymore.
                    audio_handler.calculate_gain_thread(path, bank, button)?;
                }

                // Update the lighting..
                self.load_colour_map().await?;
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
                    .remove_sample_file_by_index(bank, button, index)?;

                if remaining == 0 {
                    self.load_colour_map().await?;
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
                self.apply_scribble(fader).await?;
            }
            GoXLRCommand::SetScribbleText(fader, text) => {
                self.profile.set_scribble_text(fader, text);
                self.apply_scribble(fader).await?;
            }
            GoXLRCommand::SetScribbleNumber(fader, number) => {
                self.profile.set_scribble_number(fader, number);
                self.apply_scribble(fader).await?;
            }
            GoXLRCommand::SetScribbleInvert(fader, inverted) => {
                self.profile.set_scribble_inverted(fader, inverted);
                self.apply_scribble(fader).await?;
            }

            // Profiles
            GoXLRCommand::NewProfile(profile_name) => {
                self.stop_all_samples().await?;
                let profile_directory = self.settings.get_profile_directory().await;
                let volumes = self.profile.get_current_state();

                // Do a new file verification check..
                ProfileAdapter::can_create_new_file(profile_name.clone(), &profile_directory)?;

                // Force load the default embedded profile..
                self.profile = ProfileAdapter::default();
                self.apply_profile(Some(volumes)).await?;

                // Save the profile under a new name (although, don't overwrite if exists!)
                self.profile
                    .save_as(profile_name.clone(), &profile_directory, false)?;

                // Save the profile in the settings
                self.settings
                    .set_device_profile_name(self.serial(), profile_name.as_str())
                    .await;
                self.settings.save().await;
            }
            GoXLRCommand::LoadProfile(profile_name, save_change) => {
                self.stop_all_samples().await?;
                let volumes = self.profile.get_current_state();

                let profile_directory = self.settings.get_profile_directory().await;
                self.profile = ProfileAdapter::from_named(profile_name, &profile_directory)?;

                self.apply_profile(Some(volumes)).await?;
                if save_change {
                    self.settings
                        .set_device_profile_name(self.serial(), self.profile.name())
                        .await;
                    self.settings.save().await;
                }
            }
            GoXLRCommand::LoadProfileColours(profile_name) => {
                debug!("Loading Colours For Profile: {}", profile_name);
                let profile_directory = self.settings.get_profile_directory().await;
                let profile = ProfileAdapter::from_named(profile_name, &profile_directory)?;
                debug!("Profile Loaded, Applying Colours..");
                self.profile.load_colour_profile(profile);

                if self.device_supports_animations() {
                    self.load_animation(false).await?;
                } else {
                    self.load_colour_map().await?;
                }
                self.update_button_states()?;
            }
            GoXLRCommand::SaveProfile() => {
                let profile_directory = self.settings.get_profile_directory().await;
                self.profile.save(&profile_directory, true)?;
            }
            GoXLRCommand::SaveProfileAs(profile_name) => {
                let profile_directory = self.settings.get_profile_directory().await;

                // Do a new file verification check..
                ProfileAdapter::can_create_new_file(profile_name.clone(), &profile_directory)?;

                self.profile
                    .save_as(profile_name.clone(), &profile_directory, false)?;

                // Save the new name in the settings
                self.settings
                    .set_device_profile_name(self.serial(), profile_name.as_str())
                    .await;

                self.settings.save().await;
            }
            GoXLRCommand::DeleteProfile(profile_name) => {
                if self.profile.name() == profile_name {
                    bail!("Unable to Remove Active Profile!");
                }

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
                self.mic_profile.save_as(
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
            GoXLRCommand::LoadMicProfile(mic_profile_name, save_change) => {
                let mic_profile_directory = self.settings.get_mic_profile_directory().await;
                self.mic_profile =
                    MicProfileAdapter::from_named(mic_profile_name, &mic_profile_directory)?;
                self.apply_mic_profile().await?;

                if save_change {
                    self.settings
                        .set_device_mic_profile_name(self.serial(), self.mic_profile.name())
                        .await;
                    self.settings.save().await;
                }
            }
            GoXLRCommand::SaveMicProfile() => {
                let mic_profile_directory = self.settings.get_mic_profile_directory().await;
                self.mic_profile.save(&mic_profile_directory, true)?;
            }
            GoXLRCommand::SaveMicProfileAs(profile_name) => {
                let profile_directory = self.settings.get_mic_profile_directory().await;
                MicProfileAdapter::can_create_new_file(profile_name.clone(), &profile_directory)?;

                self.mic_profile
                    .save_as(profile_name.clone(), &profile_directory, false)?;

                // Save the new name in the settings
                self.settings
                    .set_device_mic_profile_name(self.serial(), profile_name.as_str())
                    .await;

                self.settings.save().await;
            }
            GoXLRCommand::DeleteMicProfile(profile_name) => {
                if self.mic_profile.name() == profile_name {
                    bail!("Unable to Remove Active Profile!");
                }

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

            GoXLRCommand::SetMonitorWithFx(value) => {
                self.settings
                    .set_enable_monitor_with_fx(self.serial(), value)
                    .await;
                self.settings.save().await;
                self.apply_routing(BasicInputDevice::Microphone).await?;
            }

            GoXLRCommand::SetLockFaders(value) => {
                let current = self.settings.get_device_lock_faders(self.serial()).await;

                if current != value {
                    self.settings
                        .set_device_lock_faders(self.serial(), value)
                        .await;

                    self.settings.save().await;

                    if value {
                        self.lock_faders()?;
                    } else {
                        self.unlock_faders()?;
                    }
                    self.load_colour_map().await?;
                }
            }
            GoXLRCommand::SetActiveEffectPreset(preset) => {
                self.load_effect_bank(preset).await?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetActiveSamplerBank(bank) => {
                self.load_sample_bank(bank).await?;
                self.load_colour_map().await?;
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
            GoXLRCommand::SetCoughMuteState(state) => {
                // This is more complicated because the 'state' of the mute can come from
                // various different locations, so what we're going to do is simply update
                // the profile, and re-apply the Mute settings from there.
                if !self.profile.is_mute_chat_button_toggle() {
                    bail!("Cannot Set state when Mute button is in 'Hold' Mode");
                }
                match state {
                    MuteState::Unmuted => {
                        self.profile.set_mute_chat_button_on(false);
                        self.profile.set_mute_chat_button_blink(false);
                    }
                    MuteState::MutedToX => {
                        self.profile.set_mute_chat_button_on(true);
                        self.profile.set_mute_chat_button_blink(false);
                    }
                    MuteState::MutedToAll => {
                        self.profile.set_mute_chat_button_on(false);
                        self.profile.set_mute_chat_button_blink(true);
                    }
                }
                self.apply_cough_from_profile()?;
                self.apply_routing(BasicInputDevice::Microphone).await?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetSubMixEnabled(enabled) => {
                let headphones = goxlr_types::OutputDevice::Headphones;
                if self.profile.is_submix_enabled() != enabled {
                    if !enabled {
                        // Submixes are being disabled, we need to revert the monitor..
                        self.profile.set_monitor_mix(headphones)?;
                        for device in BasicInputDevice::iter() {
                            self.apply_routing(device).await?;
                        }
                    }

                    self.profile.set_submix_enabled(enabled)?;
                    self.load_submix_settings(true)?;
                }
            }
            GoXLRCommand::SetSubMixVolume(channel, volume) => {
                self.apply_submix_volume(channel, volume)?;
            }
            GoXLRCommand::SetSubMixLinked(channel, linked) => {
                self.link_submix_channel(channel, linked)?;
            }
            GoXLRCommand::SetSubMixOutputMix(device, mix) => {
                self.profile.set_mix_output(device, mix)?;
                self.load_submix_settings(false)?;
            }
            GoXLRCommand::SetMonitorMix(device) => {
                self.profile.set_monitor_mix(device)?;

                // Might be a cleaner way to do this, we only need to handle 1 output..
                for device in BasicInputDevice::iter() {
                    self.apply_routing(device).await?;
                }

                // Make sure to switch Headphones from A to B if needed.
                self.load_submix_settings(false)?;
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
            self.apply_transient_cough_routing(router)?;
        }

        Ok(())
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
        router: &mut EnumMap<BasicOutputDevice, bool>,
    ) -> Result<()> {
        // Same deal, pull out the current state, make needed changes.
        let (_mute_toggle, muted_to_x, muted_to_all, mute_function) =
            self.profile.get_mute_chat_button_state();

        self.apply_transient_channel_routing(
            ChannelName::Mic,
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
                for output in BasicOutputDevice::iter() {
                    router[output] = false;
                }
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

    async fn apply_routing(&mut self, input: BasicInputDevice) -> Result<()> {
        // Load the routing for this channel from the profile..
        let mut router = self.profile.get_router(input);

        // Before we apply transient routing (especially because mic), check whether we should
        // be forcing Mic -> Headphones to 'On' due to settings..
        if input == BasicInputDevice::Microphone {
            // If the mic is muted, transient routing will forcefully disable this, so we should
            // be safe to simply set it true here, and hope for the best :D
            let serial = self.hardware.serial_number.as_str();
            if self.settings.get_enable_monitor_with_fx(serial).await {
                // We need to adjust this based on the FX state..
                if self.profile.is_fx_enabled() {
                    router[BasicOutputDevice::Headphones] = true;
                }
            }
        }

        self.apply_transient_routing(input, &mut router)?;
        debug!("Applying Routing to {:?}:", input);
        debug!("{:?}", router);

        let monitor = self.profile.get_monitoring_mix();
        if monitor != BasicOutputDevice::Headphones {
            router[BasicOutputDevice::Headphones] = router[monitor];
        }

        self.apply_channel_routing(input, router)?;

        Ok(())
    }

    fn apply_mute_from_profile(
        &mut self,
        fader: FaderName,
        current: Option<ChannelState>,
    ) -> Result<()> {
        // Basically stripped down behaviour from handle_fader_mute which simply applies stuff.
        let channel = self.profile.get_fader_assignment(fader);

        let (muted_to_x, muted_to_all, mute_function) = self.profile.get_mute_button_state(fader);
        if muted_to_all || (muted_to_x && mute_function == MuteFunction::All) {
            if let Some(current) = current {
                if current != Muted {
                    // This channel should be fully muted
                    debug!(
                        "Setting Channel {} to Muted (change from previous)",
                        channel
                    );
                    self.goxlr.set_channel_state(channel, Muted)?;
                } else {
                    debug!("Fader {} is Already Muted, doing nothing.", fader);
                }
            } else {
                debug!("Setting Channel {} to Muted (no previous)", channel);
                self.goxlr.set_channel_state(channel, Muted)?;
            }

            return Ok(());
        }

        // This channel isn't supposed to be muted (The Router will handle anything else).
        if let Some(current) = current {
            if current != Unmuted {
                debug!("Channel {} set to Unmuted (change from previous)", channel);
                self.goxlr.set_channel_state(channel, Unmuted)?;
            } else {
                debug!("Channel {} already Unmuted, doing nothing.", fader);
            }
        } else {
            debug!("Channel {} set to Unmuted (no previous)", channel);
            self.goxlr.set_channel_state(channel, Unmuted)?;
        }

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
            debug!("Setting Mic to Muted");
            self.goxlr.set_channel_state(ChannelName::Mic, Muted)?;
        } else {
            debug!("Setting Mic to Unmuted");
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

            // Submix firmware bug mitigation:
            if new_channel == ChannelName::Headphones || new_channel == ChannelName::LineOut {
                return Ok(());
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

            // Make sure the new channel comes in with the correct volume..
            if new_channel == ChannelName::Headphones || new_channel == ChannelName::LineOut {
                let volume = self.profile.get_channel_volume(new_channel);
                self.goxlr.set_volume(new_channel, volume)?;
            }

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

        // If the channel being moved is either Headphone or Line Out, reset the volume..
        if new_channel == ChannelName::Headphones || new_channel == ChannelName::LineOut {
            let volume = self.profile.get_channel_volume(new_channel);
            self.goxlr.set_volume(new_channel, volume)?;
        }
        if existing_channel == ChannelName::Headphones || existing_channel == ChannelName::LineOut {
            let volume = self.profile.get_channel_volume(existing_channel);
            self.goxlr.set_volume(existing_channel, volume)?;
        }

        self.apply_scribble(fader).await?;
        self.apply_scribble(fader_to_switch).await?;

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

    fn set_all_fader_display_from_profile(&mut self) -> Result<()> {
        for fader in FaderName::iter() {
            self.set_fader_display_from_profile(fader)?;
        }
        Ok(())
    }

    fn set_fader_display_from_profile(&mut self, fader: FaderName) -> Result<()> {
        self.goxlr.set_fader_display_mode(
            fader,
            self.profile.is_fader_gradient(fader),
            self.profile.is_fader_meter(fader),
        )?;
        Ok(())
    }

    async fn load_colour_map(&mut self) -> Result<()> {
        // The new colour format occurred on different firmware versions depending on device,
        // so do the check here.

        let device_mini = self.hardware.device_type == DeviceType::Mini;
        let lock_faders = self.settings.get_device_lock_faders(self.serial()).await;

        let blank_mute = device_mini || lock_faders;

        let use_1_3_40_format = self.device_supports_animations();
        let colour_map = self.profile.get_colour_map(use_1_3_40_format, blank_mute);

        if use_1_3_40_format {
            self.goxlr.set_button_colours_1_3_40(colour_map)?;
        } else {
            let mut map: [u8; 328] = [0; 328];
            map.copy_from_slice(&colour_map[0..328]);
            self.goxlr.set_button_colours(map)?;
        }

        Ok(())
    }

    async fn load_animation(&mut self, map_set: bool) -> Result<()> {
        let enabled = self.profile.get_animation_mode() != goxlr_types::AnimationMode::None;

        // This one is kinda weird, we go from profile -> types -> usb..
        let mode = match self.profile.get_animation_mode() {
            goxlr_types::AnimationMode::RetroRainbow => AnimationMode::RetroRainbow,
            goxlr_types::AnimationMode::RainbowDark => AnimationMode::RainbowDark,
            goxlr_types::AnimationMode::RainbowBright => AnimationMode::RainbowBright,
            goxlr_types::AnimationMode::Simple => AnimationMode::Simple,
            goxlr_types::AnimationMode::Ripple => AnimationMode::Ripple,
            goxlr_types::AnimationMode::None => AnimationMode::None,
        };

        let mod1 = self.profile.get_animation_mod1();
        let mod2 = self.profile.get_animation_mod2();
        let waterfall = match self.profile.get_animation_waterfall() {
            WaterfallDirection::Down => WaterFallDir::Down,
            WaterfallDirection::Up => WaterFallDir::Up,
            WaterfallDirection::Off => WaterFallDir::Off,
        };

        self.goxlr
            .set_animation_mode(enabled, mode, mod1, mod2, waterfall)?;

        if !map_set
            && (mode == AnimationMode::None
                || mode == AnimationMode::Ripple
                || mode == AnimationMode::Simple)
        {
            self.load_colour_map().await?;
        }

        Ok(())
    }

    async fn apply_profile(&mut self, current: Option<CurrentState>) -> Result<()> {
        // Set volumes first, applying mute may modify stuff..
        debug!("Applying Profile..");

        debug!("Setting Faders..");
        let mut mic_assigned_to_fader = false;
        //
        // // Prepare the faders, and configure channel mute states
        for fader in FaderName::iter() {
            let assignment = self.profile.get_fader_assignment(fader);

            if let Some(current) = &current {
                if current.faders[fader] != assignment {
                    debug!("Setting Fader {} to {:?}", fader, assignment);
                    self.goxlr.set_fader(fader, assignment)?;
                } else {
                    debug!("Fader Already Assigned, ignoring");
                }
            } else {
                debug!("Setting Fader {} to {:?}", fader, assignment);
                self.goxlr.set_fader(fader, assignment)?;
            }

            // Force Mic Fader Assignment
            if assignment == ChannelName::Mic {
                mic_assigned_to_fader = true;
                self.profile.set_mic_fader(fader)?;
            }
        }
        if !mic_assigned_to_fader {
            self.profile.clear_mic_fader();
        }

        debug!("Setting Mute States..");
        for channel in ChannelName::iter() {
            if channel == ChannelName::Mic {
                debug!("Applying Microphone Mute State");
                self.apply_cough_from_profile()?;
            } else if let Some(fader) = self.profile.get_fader_from_channel(channel) {
                debug!("Channel {} on Fader, Loading State from Profile", channel);
                if let Some(current) = &current {
                    self.apply_mute_from_profile(fader, Some(current.mute_state[channel]))?;
                } else {
                    self.apply_mute_from_profile(fader, None)?;
                }
            } else if let Some(current) = &current {
                if current.mute_state[channel] != Unmuted {
                    debug!("Channel {} not on Fader, but muted. Unmuting..", channel);
                    self.goxlr.set_channel_state(channel, Unmuted)?;
                }
            } else {
                debug!("Unknown Channel state for {}, Unmuting.", channel);
                self.goxlr.set_channel_state(channel, Unmuted)?;
            }
        }

        debug!("Setting Channel Volumes..");
        let volumes = if let Some(current) = &current {
            self.get_load_volume_order(Some(current.volumes))
        } else {
            self.get_load_volume_order(None)
        };

        for channel in volumes {
            let channel_volume = self.profile.get_channel_volume(channel);

            debug!("Setting volume for {} to {}", channel, channel_volume);
            self.goxlr.set_volume(channel, channel_volume)?;
        }

        debug!("Applying Submixing Settings..");
        self.load_submix_settings(true)?;

        debug!("Loading Colour Map..");
        self.load_colour_map().await?;

        if self.device_supports_animations() {
            // Load any animation settings..
            self.load_animation(true).await?;
        }

        debug!("Setting Fader display modes..");
        for fader in FaderName::iter() {
            debug!("Setting display for {}", fader);
            self.set_fader_display_from_profile(fader)?;
        }

        if self.hardware.device_type == DeviceType::Full {
            for fader in FaderName::iter() {
                self.apply_scribble(fader).await?;
            }
        }

        debug!("Updating button states..");
        self.update_button_states()?;

        debug!("Applying Routing..");
        // For profile load, we should configure all the input channels from the profile,
        // this is split so we can do tweaks in places where needed.
        for input in BasicInputDevice::iter() {
            self.apply_routing(input).await?;
        }

        debug!("Applying Voice FX");
        self.apply_voice_fx()?;

        // Drop this to the end so it doesn't directly interfere with profile loading..
        debug!("Validating Sampler Configuration..");
        self.validate_sampler().await?;

        Ok(())
    }

    fn get_load_volume_order(&self, volumes: Option<EnumMap<ChannelName, u8>>) -> Vec<ChannelName> {
        // This method exists primarily to 'smooth' the loading of new volumes, in situations
        // where you're starting with a Headphone volume of 100 and a System volume of 20 and are
        // finishing at Headphone 20, System 100 there's an (albeit) brief period during load where
        // both headphones and system will be at 100% which can result in brief, sudden, loud noises

        // The goal is to check whether this load is making the headphones / line out quieter,
        // and pushing their volume change to the head of the queue, making the System channel
        // in the above example briefly QUIETER, instead of louder.

        let mut order = vec![];

        if let Some(volumes) = volumes {
            let headphone_volume = self.profile.get_channel_volume(ChannelName::Headphones);
            let lineout_volume = self.profile.get_channel_volume(ChannelName::LineOut);

            if volumes[ChannelName::Headphones] > headphone_volume {
                order.push(ChannelName::Headphones);
            }
            if volumes[ChannelName::LineOut] > lineout_volume {
                order.push(ChannelName::LineOut);
            }

            // Grab all the other channels in order, and push them..
            ChannelName::iter().for_each(|channel| {
                if channel != ChannelName::Headphones && channel != ChannelName::LineOut {
                    order.push(channel);
                }
            });

            // Headphones and Line out are technically the last in the list, so we could, in theory
            // handle them in the above iter, however, they're placed here separately just in case
            // the ChannelName enum list needs to change in the future which could break this.
            if volumes[ChannelName::Headphones] <= headphone_volume {
                order.push(ChannelName::Headphones);
            }
            if volumes[ChannelName::LineOut] <= lineout_volume {
                order.push(ChannelName::LineOut);
            }
        } else {
            // We don't have reference volumes, just send all the channel names..
            debug!("No reference volumes, sending channels in order");
            return ChannelName::iter().collect();
        }

        order
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

    fn apply_voice_fx(&mut self) -> Result<()> {
        if self.hardware.device_type == DeviceType::Mini {
            // Voice FX aren't present on the mini.
            return Ok(());
        }

        // Grab all keys that aren't common between devices
        let fx_keys = self.mic_profile.get_fx_keys();

        // Setup to send these keys..
        let mut send_keys = LinkedHashSet::new();
        send_keys.extend(fx_keys);

        // Apply these settings..
        self.apply_effects(send_keys)?;

        // Apply any Pitch / Encoder related Effects
        self.set_pitch_mode()?;
        self.load_encoder_effects()?;

        Ok(())
    }

    fn apply_mic_gain(&mut self) -> Result<()> {
        let mic_type = self.mic_profile.mic_type();
        let gain = self.mic_profile.mic_gains()[mic_type];
        self.goxlr.set_microphone_gain(mic_type, gain)?;

        Ok(())
    }

    async fn apply_mic_profile(&mut self) -> Result<()> {
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
        keys.extend(self.mic_profile.get_mic_keys());

        self.apply_effects(keys)?;

        Ok(())
    }

    fn load_encoder_effects(&mut self) -> Result<()> {
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

    async fn apply_scribble(&mut self, fader: FaderName) -> Result<()> {
        let icon_path = self.settings.get_icons_directory().await;

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

    fn load_submix_settings(&mut self, apply_volumes: bool) -> Result<()> {
        if !self.device_supports_submixes() {
            // Submixes not supported, do nothing.
            return Ok(());
        }

        let mut mix_a: [u8; 4] = [0x0c; 4];
        let mut mix_b: [u8; 4] = [0x0c; 4];

        let mut index = 0;
        let submix_enabled = self.profile.is_submix_enabled();

        // This is kinda awkward, but we'll run with it..
        for device in BasicOutputDevice::iter() {
            if device == BasicOutputDevice::Headphones {
                // We need to make sure the monitor is on the right side..
                if submix_enabled {
                    let mix = self.profile.get_submix_channel(device);
                    self.goxlr.set_monitored_mix(mix)?;
                } else {
                    self.goxlr.set_monitored_mix(Mix::A)?;
                }

                // Monitor Mix handled, move to the next channel
                continue;
            }
            if submix_enabled {
                // We need to place this on the correct mix..
                match self.profile.get_submix_channel(device) {
                    Mix::A => mix_a[index] = (device as u8) * 2,
                    Mix::B => mix_b[index] = (device as u8) * 2,
                }
            } else {
                // Force this channel to A..
                mix_a[index] = (device as u8) * 2;
            }
            index += 1;
        }

        let submix = [mix_a, mix_b].concat();

        // This should always be successful, in theory :D
        self.goxlr.set_channel_mixes(submix.try_into().unwrap())?;

        if submix_enabled && apply_volumes {
            for channel in ChannelName::iter() {
                self.sync_submix_volume(channel)?;
            }
        }

        // If submixes are enabled, the Mic Monitor should be at 100% as monitoring
        // is supposed to be handled by the mix.
        if submix_enabled {
            self.goxlr.set_volume(ChannelName::MicMonitor, 255)?;
        } else {
            let volume = self.profile.get_channel_volume(ChannelName::MicMonitor);
            self.goxlr.set_volume(ChannelName::MicMonitor, volume)?;
        }

        Ok(())
    }

    fn sync_submix_volume(&mut self, channel: ChannelName) -> Result<()> {
        if let Some(mix) = self.profile.get_submix_from_channel(channel) {
            if self.profile.is_channel_linked(mix) {
                // Get the channels volume..
                let volume = self.profile.get_channel_volume(channel);
                self.update_submix_for(channel, volume)?;
            } else {
                let sub_volume = self.profile.get_submix_volume(mix);
                self.goxlr.set_sub_volume(mix, sub_volume)?;
            }
        }
        Ok(())
    }

    fn apply_submix_volume(&mut self, channel: ChannelName, volume: u8) -> Result<()> {
        if let Some(mix) = self.profile.get_submix_from_channel(channel) {
            if self.profile.is_channel_linked(mix) {
                // We need to calculate the new value for the main channel..
                let ratio = self.profile.get_submix_ratio(mix);

                let linked_volume = (volume as f64 / ratio) as u8;
                if self.profile.get_channel_volume(channel) != linked_volume {
                    // Setup the latch..
                    if let Some(fader) = self.profile.get_fader_from_channel(channel) {
                        self.fader_pause_until[fader].paused = true;
                        self.fader_pause_until[fader].until = linked_volume;
                    }
                    self.profile.set_channel_volume(channel, linked_volume)?;
                    self.goxlr.set_volume(channel, linked_volume)?;
                }
            }

            // Apply the submix volume..
            self.profile.set_submix_volume(mix, volume)?;
            self.goxlr.set_sub_volume(mix, volume)?;
        }
        Ok(())
    }

    fn link_submix_channel(&mut self, channel: ChannelName, linked: bool) -> Result<()> {
        if let Some(mix) = self.profile.get_submix_from_channel(channel) {
            if !linked {
                // We don't need to do anything special here..
                self.profile.set_submix_linked(mix, linked)?;
                return Ok(());
            } else {
                // We need to work out the current ratio between the channel, and it's mix..
                let channel_volume = self.profile.get_channel_volume(channel);
                let mix_volume = self.profile.get_submix_volume(mix);
                let ratio = mix_volume as f64 / channel_volume as f64;

                // Enable the link, and set the ratio..
                self.profile.set_submix_linked(mix, linked)?;
                self.profile.set_submix_link_ratio(mix, ratio)?;
            }
        }
        Ok(())
    }

    fn needs_submix_correction(&self, channel: ChannelName) -> bool {
        self.device_supports_submixes()
            && (channel == ChannelName::Headphones || channel == ChannelName::LineOut)
    }

    fn device_supports_submixes(&self) -> bool {
        match self.hardware.device_type {
            DeviceType::Unknown => false,
            DeviceType::Full => version_newer_or_equal_to(
                &self.hardware.versions.firmware,
                VersionNumber(1, 4, 2, 107),
            ),
            DeviceType::Mini => version_newer_or_equal_to(
                &self.hardware.versions.firmware,
                VersionNumber(1, 2, 0, 46),
            ),
        }
    }

    fn device_supports_animations(&self) -> bool {
        match self.hardware.device_type {
            DeviceType::Unknown => true,
            DeviceType::Full => version_newer_or_equal_to(
                &self.hardware.versions.firmware,
                VersionNumber(1, 3, 40, 0),
            ),
            DeviceType::Mini => version_newer_or_equal_to(
                &self.hardware.versions.firmware,
                VersionNumber(1, 1, 8, 0),
            ),
        }
    }

    // Get the current time in millis..
    fn get_epoch_ms(&self) -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }
}

fn tts_bool_to_state(bool: bool) -> String {
    match bool {
        true => "On".to_string(),
        false => "Off".to_string(),
    }
}

fn tts_target(target: MuteFunction) -> String {
    match target {
        MuteFunction::All => "".to_string(),
        MuteFunction::ToStream => " to Stream".to_string(),
        MuteFunction::ToVoiceChat => " to Voice Chat".to_string(),
        MuteFunction::ToPhones => " to Headphones".to_string(),
        MuteFunction::ToLineOut => " to Line Out".to_string(),
    }
}
