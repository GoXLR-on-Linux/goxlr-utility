use std::collections::HashSet;
use crate::profile::{version_newer_or_equal_to, MicProfileAdapter, ProfileAdapter, SampleBank};
use crate::SettingsHandle;
use anyhow::{anyhow, Result};
use enumset::EnumSet;
use goxlr_ipc::{DeviceType, FaderStatus, GoXLRCommand, HardwareStatus, MicSettings, MixerStatus};
use goxlr_types::{ChannelName, EffectBankPresets, EffectKey, EncoderName, FaderName, InputDevice as BasicInputDevice, MicrophoneParamKey, OutputDevice as BasicOutputDevice, VersionNumber};
use goxlr_usb::buttonstate::{ButtonStates, Buttons};
use goxlr_usb::channelstate::ChannelState;
use goxlr_usb::goxlr::GoXLR;
use goxlr_usb::routing::{InputDevice, OutputDevice};
use goxlr_usb::rusb::UsbContext;
use log::{debug, error, info, warn};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use enum_map::EnumMap;
use futures::executor::block_on;
use strum::{IntoEnumIterator};
use goxlr_profile_loader::components::mute::{MuteFunction};
use goxlr_usb::channelstate::ChannelState::{Muted, Unmuted};

#[derive(Debug)]
pub struct Device<'a, T: UsbContext> {
    goxlr: GoXLR<T>,
    hardware: HardwareStatus,
    last_buttons: EnumSet<Buttons>,
    button_states: EnumMap<Buttons, ButtonState>,
    profile: ProfileAdapter,
    mic_profile: MicProfileAdapter,
    settings: &'a SettingsHandle,
}

// Experimental code:
#[derive(Debug, Default, Copy, Clone)]
struct ButtonState {
    press_time: u128,
    hold_handled: bool
}

impl<'a, T: UsbContext> Device<'a, T> {
    pub fn new(
        goxlr: GoXLR<T>,
        hardware: HardwareStatus,
        profile_name: Option<String>,
        mic_profile_name: Option<String>,
        profile_directory: &Path,
        mic_profile_directory: &Path,
        settings_handle: &'a SettingsHandle
    ) -> Result<Self> {
        info!("Loading Profile: {}", profile_name.clone().unwrap_or("Not Defined".to_string()));
        info!("Loading Mic Profile: {}", mic_profile_name.clone().unwrap_or("Not Defined".to_string()));
        let profile = ProfileAdapter::from_named_or_default(profile_name, profile_directory);
        let mic_profile =
            MicProfileAdapter::from_named_or_default(mic_profile_name, mic_profile_directory);

        let mut device = Self {
            profile,
            mic_profile,
            goxlr,
            hardware,
            last_buttons: EnumSet::empty(),
            button_states: EnumMap::default(),
            settings: settings_handle,
        };

        device.apply_profile()?;
        device.apply_mic_profile()?;

        device.get_sample_device();

        Ok(device)
    }

    pub fn serial(&self) -> &str {
        &self.hardware.serial_number
    }

    pub fn status(&self) -> MixerStatus {
        let mut fader_map = [Default::default(); 4];
        fader_map[FaderName::A as usize] = self.get_fader_state(FaderName::A);
        fader_map[FaderName::B as usize] = self.get_fader_state(FaderName::B);
        fader_map[FaderName::C as usize] = self.get_fader_state(FaderName::C);
        fader_map[FaderName::D as usize] = self.get_fader_state(FaderName::D);

        MixerStatus {
            hardware: self.hardware.clone(),
            fader_status: fader_map,
            volumes: self.profile.get_volumes(),
            router: self.profile.create_router(),
            router_table: self.profile.create_router_table(),
            mic_status: MicSettings {
                mic_type: self.mic_profile.mic_type(),
                mic_gains: self.mic_profile.mic_gains(),
                noise_gate: self.mic_profile.noise_gate_ipc(),
                equaliser: self.mic_profile.equalizer_ipc(),
                equaliser_mini: self.mic_profile.equalizer_mini_ipc(),
                compressor: self.mic_profile.compressor_ipc()
            },
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

    pub async fn monitor_inputs(&mut self) -> Result<()> {
        self.hardware.usb_device.has_kernel_driver_attached =
            self.goxlr.usb_device_has_kernel_driver_active()?;

        if let Ok((buttons, volumes, encoders)) = self.goxlr.get_button_states() {
            self.update_volumes_to(volumes);
            self.update_encoders_to(encoders)?;

            let pressed_buttons = buttons.difference(self.last_buttons);
            for button in pressed_buttons {
                // This is a new press, store it in the states..
                self.button_states[button] = ButtonState {
                    press_time: self.get_epoch_ms(),
                    hold_handled: false
                };

                self.on_button_down(button).await?;
            }

            let released_buttons = self.last_buttons.difference(buttons);
            for button in released_buttons {
                let button_state = self.button_states[button];
                self.on_button_up(button, &button_state).await?;

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
                        self.on_button_hold(button).await?;
                        self.button_states[button].hold_handled = true;
                    }
                }
            }

            self.last_buttons = buttons;
        }

        Ok(())
    }

    async fn on_button_down(&mut self, button: Buttons) -> Result<()> {
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

    async fn on_button_hold(&mut self, button: Buttons) -> Result<()> {
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

    async fn on_button_up(&mut self, button: Buttons, state: &ButtonState) -> Result<()> {
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
                self.toggle_megaphone().await?;
            },
            Buttons::EffectRobot => {
                self.toggle_robot().await?;
            }
            Buttons::EffectHardTune => {
                self.toggle_hardtune().await?;
            }
            Buttons::EffectFx => {
                self.toggle_effects().await?;
            }

            // This is mostly experimental..
            Buttons::SamplerBottomLeft => {
                if self.profile.get_active_sample_bank() == &SampleBank::C {
                    debug!("Playing.. ");
                    self.play_audio_sample("40minutes.wav".to_string());
                }
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

    async fn unmute_if_muted(&mut self, fader: FaderName) -> Result<()> {
        let (muted_to_x, muted_to_all, _mute_function) = self.profile.get_mute_button_state(fader);

        if muted_to_x || muted_to_all {
            self.handle_fader_mute(fader, false).await?;
        }

        Ok(())
    }

    async fn unmute_chat_if_muted(&mut self) -> Result<()> {
        let (_mute_toggle, muted_to_x, muted_to_all, _mute_function) = self.profile.get_mute_chat_button_state();

        if muted_to_x || muted_to_all {
            self.handle_cough_mute(true, false, false, false).await?;
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

    async fn load_effect_bank(&mut self, preset: EffectBankPresets) -> Result<()> {
        self.profile.load_effect_bank(preset);
        self.load_effects()?;
        self.set_pitch_mode()?;

        // Configure the various parts..
        let mut keyset = HashSet::new();
        keyset.extend(self.mic_profile.get_reverb_keyset());
        keyset.extend(self.mic_profile.get_echo_keyset());
        keyset.extend(self.mic_profile.get_pitch_keyset());
        keyset.extend(self.mic_profile.get_gender_keyset());
        keyset.extend(self.mic_profile.get_megaphone_keyset());
        keyset.extend(self.mic_profile.get_robot_keyset());
        keyset.extend(self.mic_profile.get_hardtune_keyset());

        self.apply_effects(keyset)?;

        Ok(())
    }

    async fn toggle_megaphone(&mut self) -> Result<()> {
        self.profile.toggle_megaphone();
        self.apply_effects(HashSet::from([EffectKey::MegaphoneEnabled]))?;
        Ok(())
    }

    async fn toggle_robot(&mut self) -> Result<()> {
        self.profile.toggle_robot();
        self.apply_effects(HashSet::from([EffectKey::RobotEnabled]))?;
        Ok(())
    }

    async fn toggle_hardtune(&mut self) -> Result<()> {
        self.profile.toggle_hardtune();
        self.apply_effects(HashSet::from([EffectKey::HardTuneEnabled]))?;
        self.set_pitch_mode()?;
        Ok(())
    }

    async fn toggle_effects(&mut self) -> Result<()> {
        self.profile.toggle_effects();

        // When this changes, we need to update all the 'Enabled' keys..
        let mut key_updates = HashSet::new();
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

    fn update_encoders_to(&mut self, encoders: [i8; 4]) -> Result<()> {
        // Ok, this is funky, due to the way pitch works, the encoder 'value' doesn't match
        // the profile value if hardtune is enabled, so we'll pre-emptively calculate pitch here..
        let mut pitch_value = encoders[0];
        if self.profile.is_hardtune_pitch_enabled() {
            pitch_value = pitch_value * 12;
        }

        if pitch_value != self.profile.get_pitch_value() {
            debug!(
                "Updating PITCH value from {} to {} as human moved the dial",
                self.profile.get_pitch_value(),
                pitch_value
            );

            // Ok, if hard tune is enabled, multiply this value by 12..
            self.profile.set_pitch_value(pitch_value);
            self.apply_effects(HashSet::from([EffectKey::PitchAmount]))?;
        }

        if encoders[1] != self.profile.get_gender_value() {
            debug!(
                "Updating GENDER value from {} to {} as human moved the dial",
                self.profile.get_gender_value(),
                encoders[1]
            );
            self.profile.set_gender_value(encoders[1]);
            self.apply_effects(HashSet::from([EffectKey::GenderAmount]))?;
        }

        if encoders[2] != self.profile.get_reverb_value() {
            debug!(
                "Updating REVERB value from {} to {} as human moved the dial",
                self.profile.get_reverb_value(),
                encoders[2]
            );
            self.profile.set_reverb_value(encoders[2]);
            self.apply_effects(HashSet::from([EffectKey::ReverbAmount]))?;
        }

        if encoders[3] != self.profile.get_echo_value() {
            debug!(
                "Updating ECHO value from {} to {} as human moved the dial",
                self.profile.get_echo_value(),
                encoders[3]
            );
            self.profile.set_echo_value(encoders[3]);
            self.apply_effects(HashSet::from([EffectKey::EchoAmount]))?;
        }

        Ok(())
    }

    pub async fn perform_command(
        &mut self,
        command: GoXLRCommand,
    ) -> Result<()> {
        match command {
            GoXLRCommand::SetFader(fader, channel) => {
                self.set_fader(fader, channel).await?;
            }
            GoXLRCommand::SetFaderMuteFunction(fader, behaviour) => {
                if self.profile.get_mute_button_behaviour(fader) == behaviour {
                    // Settings are the same..
                    return Ok(());
                }

                // Unmute the channel to prevent weirdness, then set new behaviour
                self.unmute_if_muted(fader).await?;
                self.profile.set_mute_button_behaviour(fader, behaviour);
            }
            GoXLRCommand::SetFaderDisplay(fader, display) => {
                self.profile.set_fader_display(fader, display);
                self.set_fader_display_from_profile(fader)?;
            }
            GoXLRCommand::SetFaderColours(fader, top, bottom) => {
                // Need to get the fader colour map, and set values..
                self.profile.set_fader_colours(fader, top, bottom)?;
                self.load_colour_map()?;
            },
            GoXLRCommand::SetFaderButtonColours(fader, one, style, two) => {
                self.profile.set_mute_button_off_style(fader, style);
                self.profile.set_mute_button_colours(fader, one, two)?;

                self.load_colour_map()?;
                self.update_button_states()?;
            },
            GoXLRCommand::SetAllFaderColours(top, bottom) => {
                // I considered this as part of SetFaderColours, but spamming a new colour map
                // for every fader change seemed excessive, this allows us to set them all before
                // reloading.
                for fader in FaderName::iter() {
                    self.profile.set_fader_colours(fader, top.to_owned(), bottom.to_owned())?;
                }
                self.load_colour_map()?;
            }
            GoXLRCommand::SetAllFaderButtonColours(one, style, two) => {
                for fader in FaderName::iter() {
                    self.profile.set_mute_button_off_style(fader, style);
                    self.profile.set_mute_button_colours(fader,one.to_owned(), two.to_owned())?;
                }
                self.load_colour_map()?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetVolume(channel, volume) => {
                self.profile.set_channel_volume(channel, volume);
                self.goxlr.set_volume(channel, volume)?;
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
            GoXLRCommand::SetCoughColourConfiguration(colour_one, off_style, colour_two) => {
                self.profile.set_mute_chat_off_style(off_style);
                self.profile.set_mute_chat_colours(colour_one, colour_two)?;
                self.load_colour_map()?;
                self.update_button_states()?;
            }
            GoXLRCommand::SetSwearButtonVolume(volume) => {
                if volume < -34 || volume > 0 {
                    return Err(anyhow!("Mute volume must be between -34 and 0"));
                }
                self.settings
                    .set_device_bleep_volume(self.serial(), volume)
                    .await;
                self.settings.save().await;

                self.goxlr.set_effect_values(&[
                    (EffectKey::BleepLevel, volume as i32),
                ])?;
            }
            GoXLRCommand::SetSwearButtonColourConfiguration(colour_one, off_style, colour_two) => {
                self.profile.set_swear_off_style(off_style);
                self.profile.set_swear_colours(colour_one, colour_two)?;
                self.load_colour_map()?;
                self.update_button_states()?;
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
            GoXLRCommand::LoadProfile(profile_name) => {
                let profile_directory = self.settings.get_profile_directory().await;
                self.profile = ProfileAdapter::from_named(profile_name, &profile_directory)?;
                self.apply_profile()?;
                self.settings
                    .set_device_profile_name(self.serial(), self.profile.name())
                    .await;
                self.settings.save().await;
            }
            GoXLRCommand::SaveProfile() => {
                let profile_directory = self.settings.get_profile_directory().await;
                let profile_name = self.settings.get_device_profile_name(self.serial()).await;

                if let Some(profile_name) = profile_name {
                    self.profile.write_profile(profile_name, &profile_directory, true)?;
                }
            }
            GoXLRCommand::SaveProfileAs(profile_name) => {
                let profile_directory = self.settings.get_profile_directory().await;
                self.profile.write_profile(profile_name.clone(), &profile_directory, false)?;

                // Save the new name in the settings
                self.settings.set_device_profile_name(
                    self.serial(),
                    profile_name.as_str()
                ).await;

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
                let mic_profile_name = self.settings.get_device_mic_profile_name(self.serial()).await;

                if let Some(profile_name) = mic_profile_name {
                    self.mic_profile.write_profile(profile_name, &mic_profile_directory, true)?;
                }
            }
            GoXLRCommand::SaveMicProfileAs(profile_name) => {
                let profile_directory = self.settings.get_mic_profile_directory().await;
                self.mic_profile.write_profile(profile_name.clone(), &profile_directory, false)?;

                // Save the new name in the settings
                self.settings.set_device_mic_profile_name(
                    self.serial(),
                    profile_name.as_str()
                ).await;

                self.settings.save().await;
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

        // We need to handle hardtune configuration here as well..
        let hardtune_position = OutputDevice::HardTune.position();
        if self.profile.is_active_hardtune_source_all() {
            match input {
                BasicInputDevice::Music | BasicInputDevice::Game | BasicInputDevice::LineIn | BasicInputDevice::System  => {
                    left[hardtune_position] = 0x04;
                    right[hardtune_position] = 0x04;
                }
                _ => {}
            }
        } else {
            // We need to match only against a specific target..
            if input == self.profile.get_active_hardtune_source() {
                left[hardtune_position] = 0x10;
                right[hardtune_position] = 0x10;
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

    fn set_fader_display_from_profile(&mut self, fader: FaderName) -> Result<()> {
        self.goxlr.set_fader_display_mode(
            fader,
            self.profile.is_fader_gradient(fader),
            self.profile.is_fader_meter(fader)
        )?;
        Ok(())
    }

    fn load_colour_map(&mut self) -> Result<()> {
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

        Ok(())
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
        self.load_colour_map()?;

        for fader in FaderName::iter() {
            self.set_fader_display_from_profile(fader)?;
        }

        self.update_button_states()?;

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
            vec.push((param, self.mic_profile.get_param_value(param, self.serial(), self.settings)));
        }
        self.goxlr.set_mic_param(vec.as_slice())?;
        Ok(())
    }

    fn apply_effects(&mut self, params: HashSet<EffectKey>) -> Result<()> {
        let mut vec = Vec::new();
        for effect in params {
            vec.push((effect, self.mic_profile.get_effect_value(effect, self.serial(), self.settings, self.profile())));
        }

        for effect in &vec {
            let (key, value) = effect;
            debug!("Setting {:?} to {}", key, value);
        }
        self.goxlr.set_effect_values(vec.as_slice())?;
        Ok(())
    }

    fn apply_mic_profile(&mut self) -> Result<()> {
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

        let mut keys = HashSet::new();
        keys.extend(self.mic_profile.get_common_keys());

        if self.hardware.device_type == DeviceType::Full {
            keys.extend(self.mic_profile.get_full_keys());
        }

        self.apply_effects(keys)?;

        if self.hardware.device_type == DeviceType::Full {
            self.load_effects()?;
            self.set_pitch_mode()?;
        }
        Ok(())
    }

    fn load_effects(&mut self) -> Result<()> {
        // For now, we'll simply set the knob positions, more to come!
        let mut value = self.profile.get_pitch_value();
        self.goxlr.set_encoder_value(EncoderName::Pitch, value as u8)?;

        value = self.profile.get_echo_value();
        self.goxlr.set_encoder_value(EncoderName::Echo, value as u8)?;

        value = self.profile.get_gender_value();
        self.goxlr.set_encoder_value(EncoderName::Gender, value as u8)?;

        value = self.profile.get_reverb_value();
        self.goxlr.set_encoder_value(EncoderName::Reverb, value as u8)?;

        Ok(())
    }

    fn set_pitch_mode(&mut self) -> Result<()> {
        if self.hardware.device_type != DeviceType::Full {
            // Not a Full GoXLR, nothing to do.
            return Ok(())
        }

        if self.profile.is_hardtune_pitch_enabled() {
            if self.profile.is_pitch_narrow() {
                self.goxlr.set_encoder_mode(EncoderName::Pitch, 03, 01)?;
            } else {
                self.goxlr.set_encoder_mode(EncoderName::Pitch, 03, 02)?;
            }
        } else {
            self.goxlr.set_encoder_mode(EncoderName::Pitch, 01, 04)?;
        }

        Ok(())
    }

    // EXPERIMENTAL, JUST TESTING :)
    fn get_sample_device(&self) -> Option<String> {
        // This will probably change at some point to let an external script
        // handle it, just to better support cross platform behaviour..

        // Linux:
        // pactl list short sinks | grep goxlr_sample | awk '{print $2}'

        let output = Command::new("/usr/bin/pactl")
            .arg("list")
            .arg("short")
            .arg("sinks")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();

        if !output.status.success() {
            error!("Failed to List Sinks: ");
            error!("{}", String::from_utf8(output.stderr).unwrap());
            return None
        }

        let output = String::from_utf8(output.stdout).unwrap();
        let lines = output.lines();
        for line in lines {
            let sections = line.split_whitespace();
            let device = sections.skip(1).next().unwrap();
            if device.contains("goxlr_sample") {
                return Some(device.to_string());
            }
            if device.contains("GoXLR_0_8_9") {
                return Some(device.to_string());
            }
        }
        warn!("Could not find GoXLR Sampler Channel");
        None
    }

    fn play_audio_sample(&self, filename: String) {
        // Plays the specific file, again, should be sent to external script..

        // Linux
        // paplay -d <device> <file>

        // This should probably be cached somewhere..
        let device = self.get_sample_device();
        let file_path = format!("{}/{}", block_on(self.settings.get_samples_directory()).to_string_lossy(), filename);

        if let Some(device_name) = device {
            // Execute paplay to play audio
            let _command = Command::new("/usr/bin/paplay")
                .arg("-d")
                .arg(device_name)
                .arg(file_path)
                .spawn();

        }
    }

    // Get the current time in millis..
    fn get_epoch_ms(&self) -> u128 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
    }
}