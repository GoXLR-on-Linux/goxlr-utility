use crate::profile::{version_newer_or_equal_to, ProfileAdapter};
use anyhow::Result;
use enumset::EnumSet;
use goxlr_ipc::{GoXLRCommand, HardwareStatus, MixerStatus};
use goxlr_types::{
    ChannelName, FaderName, InputDevice as BasicInputDevice, MicrophoneType,
    OutputDevice as BasicOutputDevice, VersionNumber,
};
use goxlr_usb::buttonstate::{ButtonStates, Buttons};
use goxlr_usb::channelstate::ChannelState;
use goxlr_usb::goxlr::GoXLR;
use goxlr_usb::routing::{InputDevice, OutputDevice};
use goxlr_usb::rusb::UsbContext;
use log::debug;
use std::path::Path;
use strum::{EnumCount, IntoEnumIterator};

const MIN_VOLUME_THRESHOLD: u8 = 6;

#[derive(Debug)]
pub struct Device<T: UsbContext> {
    goxlr: GoXLR<T>,
    volumes_before_muted: [u8; ChannelName::COUNT],
    status: MixerStatus,
    last_buttons: EnumSet<Buttons>,
    profile: ProfileAdapter,
}

impl<T: UsbContext> Device<T> {
    pub fn new(
        mut goxlr: GoXLR<T>,
        hardware: HardwareStatus,
        profile_name: Option<String>,
        profile_directory: &Path,
    ) -> Result<Self> {
        let profile = ProfileAdapter::from_named_or_default(profile_name, profile_directory);

        let router = profile.create_router();
        let status = MixerStatus {
            hardware,
            fader_a_assignment: profile.get_fader_assignment(FaderName::A),
            fader_b_assignment: profile.get_fader_assignment(FaderName::B),
            fader_c_assignment: profile.get_fader_assignment(FaderName::C),
            fader_d_assignment: profile.get_fader_assignment(FaderName::D),
            volumes: [255; ChannelName::COUNT],
            muted: [false; ChannelName::COUNT],
            mic_gains: [0; MicrophoneType::COUNT],
            mic_type: MicrophoneType::Jack,
            router,
            profile_name: profile.name().to_owned(),
        };
        goxlr.set_fader(FaderName::A, profile.get_fader_assignment(FaderName::A))?;
        goxlr.set_fader(FaderName::B, profile.get_fader_assignment(FaderName::B))?;
        goxlr.set_fader(FaderName::C, profile.get_fader_assignment(FaderName::C))?;
        goxlr.set_fader(FaderName::D, profile.get_fader_assignment(FaderName::D))?;
        for channel in ChannelName::iter() {
            goxlr.set_volume(channel, profile.get_channel_volume(channel))?;
            goxlr.set_channel_state(channel, ChannelState::Unmuted)?;
        }
        goxlr.set_microphone_gain(MicrophoneType::Jack, 72)?;

        // Load the colour Map..
        let use_1_3_40_format = version_newer_or_equal_to(
            &status.hardware.versions.firmware,
            VersionNumber(1, 3, 40, 0),
        );
        let colour_map = profile.get_colour_map(use_1_3_40_format);

        if use_1_3_40_format {
            goxlr.set_button_colours_1_3_40(colour_map)?;
        } else {
            let mut map: [u8; 328] = [0; 328];
            map.copy_from_slice(&colour_map[0..328]);
            goxlr.set_button_colours(map)?;
        }

        let mut device = Self {
            profile,
            goxlr,
            status,
            volumes_before_muted: [255; ChannelName::COUNT],
            last_buttons: EnumSet::empty(),
        };

        device
            .goxlr
            .set_button_states(device.create_button_states())?;
        device.apply_router(&device.status.router.to_owned())?;

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

    pub fn monitor_inputs(&mut self) -> Result<()> {
        self.status.hardware.usb_device.has_kernel_driver_attached =
            self.goxlr.usb_device_has_kernel_driver_active()?;

        if let Ok((buttons, volumes)) = self.goxlr.get_button_states() {
            self.update_volumes_to(volumes);
            let released_buttons = self.last_buttons.difference(buttons);
            for button in released_buttons {
                self.on_button_press(button)?;
            }
            self.last_buttons = buttons;
        }

        Ok(())
    }

    fn on_button_press(&mut self, button: Buttons) -> Result<()> {
        debug!("Handling button press: {:?}", button);
        match button {
            Buttons::Fader1Mute => {
                self.toggle_fader_mute(FaderName::A)?;
            }
            Buttons::Fader2Mute => {
                self.toggle_fader_mute(FaderName::B)?;
            }
            Buttons::Fader3Mute => {
                self.toggle_fader_mute(FaderName::C)?;
            }
            Buttons::Fader4Mute => {
                self.toggle_fader_mute(FaderName::D)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn toggle_fader_mute(&mut self, fader: FaderName) -> Result<()> {
        let channel = self.status.get_fader_assignment(fader);
        let muted = self.status.get_channel_muted(channel);

        self.perform_command(GoXLRCommand::SetChannelMuted(channel, !muted))?;

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

    pub fn perform_command(&mut self, command: GoXLRCommand) -> Result<()> {
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
            GoXLRCommand::SetChannelMuted(channel, muted) => {
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
                if muted {
                    self.volumes_before_muted[channel as usize] =
                        self.status.get_channel_volume(channel);
                    self.goxlr.set_volume(channel, 0)?;
                } else if self.status.get_channel_volume(channel) <= MIN_VOLUME_THRESHOLD {
                    // Don't restore the old volume if the new volume is above minimum.
                    // This seems to match the official GoXLR software behaviour.
                    self.goxlr
                        .set_volume(channel, self.volumes_before_muted[channel as usize])?;
                }
                self.goxlr.set_button_states(self.create_button_states())?;
            }
            GoXLRCommand::SetMicrophoneGain(mic_type, gain) => {
                self.goxlr.set_microphone_gain(mic_type, gain)?;
                self.status.mic_type = mic_type;
                self.status.mic_gains[mic_type as usize] = gain;
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
}
