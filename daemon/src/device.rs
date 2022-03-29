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
use goxlr_types::ChannelName::Mic;

const MIN_VOLUME_THRESHOLD: u8 = 6;

#[derive(Debug)]
pub struct Device<T: UsbContext> {
    goxlr: GoXLR<T>,
    volumes_before_muted: [u8; ChannelName::COUNT],
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
            muted: [false; ChannelName::COUNT],
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
            volumes_before_muted: [255; ChannelName::COUNT],
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
                        self.button_states[button] = ButtonState {
                            press_time: self.button_states[button].press_time,
                            hold_handled: true,
                        };
                    }
                }
            }

            self.last_buttons = buttons;
        }

        Ok(())
    }

    async fn on_button_down(&mut self, button: Buttons, settings: &SettingsHandle) -> Result<()> {
        debug!("Handling Button Down: {:?}", button);

        match button {
            Buttons::MicrophoneMute => {
                if !self.profile.is_cough_toggle() {
                    self.perform_command(GoXLRCommand::SetChannelMuted(Mic, true, false), settings).await?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn on_button_hold(&mut self, button: Buttons, settings: &SettingsHandle) -> Result<()> {
        debug!("Handling Button Hold: {:?}", button);
        Ok(())
    }

    async fn on_button_up(&mut self, button: Buttons, state: &ButtonState, settings: &SettingsHandle) -> Result<()> {
        debug!("Handling Button Release: {:?}, Has Long Press Handled: {:?}", button, state.hold_handled);
        match button {
            Buttons::Fader1Mute => {
                self.toggle_fader_mute(FaderName::A, settings).await?;
            }
            Buttons::Fader2Mute => {
                self.toggle_fader_mute(FaderName::B, settings).await?;
            }
            Buttons::Fader3Mute => {
                self.toggle_fader_mute(FaderName::C, settings).await?;
            }
            Buttons::Fader4Mute => {
                self.toggle_fader_mute(FaderName::D, settings).await?;
            }
            Buttons::MicrophoneMute => {
                if self.profile.is_cough_toggle() {
                    let muted = self.status.get_channel_muted(Mic);
                    self.perform_command(GoXLRCommand::SetChannelMuted(Mic, !muted, true), settings).await?;
                } else {
                    self.perform_command(GoXLRCommand::SetChannelMuted(Mic, false, false), settings).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn toggle_fader_mute(
        &mut self,
        fader: FaderName,
        settings: &SettingsHandle,
    ) -> Result<()> {
        let channel = self.status.get_fader_assignment(fader);
        let muted = self.status.get_channel_muted(channel);

        self.perform_command(GoXLRCommand::SetChannelMuted(channel, !muted, true), settings)
            .await?;

        Ok(())
    }

    fn update_volumes_to(&mut self, volumes: [u8; 4]) {
        for fader in FaderName::iter() {
            let channel = self.status.get_fader_assignment(fader);
            let old_volume = self.status.get_channel_volume(channel);
            let new_volume = volumes[fader as usize];
            if new_volume != old_volume {
                debug!(
                    "Updating {} volume from {} to {} as a human moved the fader",
                    channel, old_volume, new_volume
                );
                self.status.set_channel_volume(channel, new_volume);
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
                self.goxlr.set_fader(fader, channel)?;
                self.status.set_fader_assignment(fader, channel);
                self.goxlr.set_button_states(self.create_button_states())?;
            }
            GoXLRCommand::SetVolume(channel, volume) => {
                self.goxlr.set_volume(channel, volume)?;
                self.status.set_channel_volume(channel, volume);
            }
            GoXLRCommand::SetChannelMuted(channel, muted, update_volume) => {
                let (_, device_volumes) = self.goxlr.get_button_states()?;
                self.update_volumes_to(device_volumes);
                self.goxlr.set_channel_state(
                    channel,
                    if muted {
                        ChannelState::Muted
                    } else {
                        ChannelState::Unmuted
                    },
                )?;
                self.status.set_channel_muted(channel, muted);

                // This may seem unusual, however for things like the cough button slapping the
                // mic fader down and up for a brief tap is probably bad for the motors :p
                if update_volume {
                    if muted {
                        // Store the pre-mute volume so it can be restored later..
                        self.volumes_before_muted[channel as usize] =
                            self.status.get_channel_volume(channel);

                        // Send the new channel volume to the device
                        self.goxlr.set_volume(channel, 0)?;

                        // In the case where a mute is happening that's not on a slider (eg,
                        // cough button), we need to update the new internal volume.
                        self.status.volumes[channel as usize] = 0;

                    } else if self.status.get_channel_volume(channel) <= MIN_VOLUME_THRESHOLD {
                        // Don't restore the old volume if the new volume is above minimum.
                        // This seems to match the official GoXLR software behaviour.
                        self.goxlr
                            .set_volume(channel, self.volumes_before_muted[channel as usize])?;

                        // As above, restore the internal volume on channels that aren't on a slider.
                        self.status.volumes[channel as usize] =
                            self.volumes_before_muted[channel as usize];
                    }
                }
                self.goxlr.set_button_states(self.create_button_states())?;
            }
            GoXLRCommand::SetMicrophoneGain(mic_type, gain) => {
                self.goxlr.set_microphone_gain(mic_type, gain)?;
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

    fn create_button_states(&self) -> [ButtonStates; 24] {
        let mut result = [ButtonStates::DimmedColour1; 24];
        if self
            .status
            .get_channel_muted(self.status.get_fader_assignment(FaderName::A))
        {
            result[Buttons::Fader1Mute as usize] = ButtonStates::Colour1;
        }
        if self
            .status
            .get_channel_muted(self.status.get_fader_assignment(FaderName::B))
        {
            result[Buttons::Fader2Mute as usize] = ButtonStates::Colour1;
        }
        if self
            .status
            .get_channel_muted(self.status.get_fader_assignment(FaderName::C))
        {
            result[Buttons::Fader3Mute as usize] = ButtonStates::Colour1;
        }
        if self
            .status
            .get_channel_muted(self.status.get_fader_assignment(FaderName::D))
        {
            result[Buttons::Fader4Mute as usize] = ButtonStates::Colour1;
        }

        if self.status.get_channel_muted(Mic) {
            result[Buttons::MicrophoneMute as usize] = ButtonStates::Colour1;
        }

        result
    }

    fn apply_router(
        &mut self,
        router: &[EnumSet<BasicOutputDevice>; BasicInputDevice::COUNT],
    ) -> Result<()> {
        for simple_input in BasicInputDevice::iter() {
            let outputs = &router[simple_input as usize];
            let (left_input, right_input) = InputDevice::from_basic(&simple_input);
            let mut left = [0; 22];
            let mut right = [0; 22];

            for simple_output in outputs.iter() {
                let (left_output, right_output) = OutputDevice::from_basic(&simple_output);
                // 0x20 is 100% volume. This is adjustable. 100% isn't the maximum, either! :D
                left[left_output.position()] = 0x20;
                right[right_output.position()] = 0x20;
            }

            self.goxlr.set_routing(left_input, left)?;
            self.goxlr.set_routing(right_input, right)?;
        }

        Ok(())
    }

    fn apply_profile(&mut self) -> Result<()> {
        self.status.profile_name = self.profile.name().to_owned();

        self.status.fader_a_assignment = self.profile.get_fader_assignment(FaderName::A);
        self.goxlr.set_fader(
            FaderName::A,
            self.profile.get_fader_assignment(FaderName::A),
        )?;

        self.status.fader_b_assignment = self.profile.get_fader_assignment(FaderName::B);
        self.goxlr.set_fader(
            FaderName::B,
            self.profile.get_fader_assignment(FaderName::B),
        )?;

        self.status.fader_c_assignment = self.profile.get_fader_assignment(FaderName::C);
        self.goxlr.set_fader(
            FaderName::C,
            self.profile.get_fader_assignment(FaderName::C),
        )?;

        self.status.fader_d_assignment = self.profile.get_fader_assignment(FaderName::D);
        self.goxlr.set_fader(
            FaderName::D,
            self.profile.get_fader_assignment(FaderName::D),
        )?;

        for channel in ChannelName::iter() {
            self.status
                .set_channel_volume(channel, self.profile.get_channel_volume(channel));
            self.goxlr
                .set_volume(channel, self.profile.get_channel_volume(channel))?;

            self.status.set_channel_muted(channel, false);
            self.goxlr
                .set_channel_state(channel, ChannelState::Unmuted)?;
        }

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

        self.goxlr.set_fader_display_mode(
            FaderName::A,
            self.profile.is_fader_gradient(FaderName::A),
            self.profile.is_fader_meter(FaderName::A)
        )?;

        self.goxlr.set_fader_display_mode(
            FaderName::B,
            self.profile.is_fader_gradient(FaderName::B),
            self.profile.is_fader_meter(FaderName::B)
        )?;

        self.goxlr.set_fader_display_mode(
            FaderName::C,
            self.profile.is_fader_gradient(FaderName::C),
            self.profile.is_fader_meter(FaderName::C)
        )?;

        self.goxlr.set_fader_display_mode(
            FaderName::D,
            self.profile.is_fader_gradient(FaderName::D),
            self.profile.is_fader_meter(FaderName::D)
        )?;

        self.goxlr.set_button_states(self.create_button_states())?;

        let router = self.profile.create_router();
        self.apply_router(&router)?;
        self.status.router = router;

        Ok(())
    }

    fn apply_mic_profile(&mut self) -> Result<()> {
        self.status.mic_profile_name = self.mic_profile.name().to_owned();

        self.status.mic_gains = self.mic_profile.mic_gains();
        self.status.mic_type = self.mic_profile.mic_type();
        self.goxlr.set_microphone_gain(
            self.status.mic_type,
            self.status.mic_gains[self.status.mic_type as usize],
        )?;

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
