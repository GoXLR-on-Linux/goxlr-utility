use anyhow::Result;
use enumset::{enum_set, EnumSet};
use goxlr_ipc::{
    DeviceStatus, DeviceType, GoXLRCommand, HardwareStatus, MixerStatus, UsbProductInformation,
};
use goxlr_types::{
    ChannelName, FaderName, InputDevice as BasicInputDevice, OutputDevice as BasicOutputDevice,
};
use goxlr_usb::buttonstate::{ButtonStates, Buttons};
use goxlr_usb::channelstate::ChannelState;
use goxlr_usb::goxlr;
use goxlr_usb::goxlr::GoXLR;
use goxlr_usb::routing::{InputDevice, OutputDevice};
use goxlr_usb::rusb::UsbContext;
use log::debug;
use strum::{EnumCount, IntoEnumIterator};

const MIN_VOLUME_THRESHOLD: u8 = 6;

#[derive(Debug)]
pub struct Device<T: UsbContext> {
    goxlr: GoXLR<T>,
    volumes_before_muted: [u8; ChannelName::COUNT],
    status: DeviceStatus,
}

impl<T: UsbContext> Device<T> {
    pub fn new(goxlr: GoXLR<T>) -> Self {
        Self {
            goxlr,
            status: DeviceStatus::default(),
            volumes_before_muted: [255; ChannelName::COUNT],
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        let descriptor = self.goxlr.usb_device_descriptor();
        self.status.device_type = match descriptor.product_id() {
            goxlr::PID_GOXLR_FULL => DeviceType::Full,
            goxlr::PID_GOXLR_MINI => DeviceType::Mini,
            _ => DeviceType::Unknown,
        };
        self.fill_usb_information()?;
        self.initialize_mixer()?;

        Ok(())
    }

    fn fill_usb_information(&mut self) -> Result<()> {
        let descriptor = self.goxlr.usb_device_descriptor();
        let device_version = descriptor.device_version();
        let version = (device_version.0, device_version.1, device_version.2);

        self.status.usb_device = Some(UsbProductInformation {
            manufacturer_name: self.goxlr.usb_device_manufacturer()?,
            product_name: self.goxlr.usb_device_product_name()?,
            is_claimed: self.goxlr.usb_device_is_claimed(),
            has_kernel_driver_attached: self.goxlr.usb_device_has_kernel_driver_active()?,
            bus_number: self.goxlr.usb_bus_number(),
            address: self.goxlr.usb_address(),
            version,
        });

        Ok(())
    }

    fn initialize_mixer(&mut self) -> Result<()> {
        self.goxlr.set_fader(FaderName::A, ChannelName::Mic)?;
        self.goxlr.set_fader(FaderName::B, ChannelName::Music)?;
        self.goxlr.set_fader(FaderName::C, ChannelName::Chat)?;
        self.goxlr.set_fader(FaderName::D, ChannelName::System)?;
        for channel in ChannelName::iter() {
            self.goxlr.set_volume(channel, 255)?;
            self.goxlr
                .set_channel_state(channel, ChannelState::Unmuted)?;
        }
        self.goxlr.set_button_states(self.create_button_states())?;

        let mut router = [EnumSet::empty(); BasicInputDevice::COUNT];
        router[BasicInputDevice::Microphone as usize] = enum_set!(
            BasicOutputDevice::Headphones
                | BasicOutputDevice::BroadcastMix
                | BasicOutputDevice::LineOut
                | BasicOutputDevice::ChatMic
                | BasicOutputDevice::Sampler
        );
        router[BasicInputDevice::Chat as usize] = enum_set!(
            BasicOutputDevice::Headphones
                | BasicOutputDevice::BroadcastMix
                | BasicOutputDevice::LineOut
        );
        router[BasicInputDevice::Music as usize] = enum_set!(
            BasicOutputDevice::Headphones
                | BasicOutputDevice::BroadcastMix
                | BasicOutputDevice::LineOut
        );
        router[BasicInputDevice::Game as usize] = enum_set!(
            BasicOutputDevice::Headphones
                | BasicOutputDevice::BroadcastMix
                | BasicOutputDevice::LineOut
        );
        router[BasicInputDevice::Console as usize] = enum_set!(
            BasicOutputDevice::Headphones
                | BasicOutputDevice::BroadcastMix
                | BasicOutputDevice::LineOut
        );
        router[BasicInputDevice::LineIn as usize] =
            enum_set!(BasicOutputDevice::Headphones | BasicOutputDevice::BroadcastMix);
        router[BasicInputDevice::System as usize] = enum_set!(
            BasicOutputDevice::Headphones
                | BasicOutputDevice::BroadcastMix
                | BasicOutputDevice::LineOut
        );
        router[BasicInputDevice::Samples as usize] = enum_set!(
            BasicOutputDevice::Headphones
                | BasicOutputDevice::BroadcastMix
                | BasicOutputDevice::LineOut
                | BasicOutputDevice::ChatMic
        );

        self.apply_router(&router)?;

        let (serial_number, manufactured_date) = self.goxlr.get_serial_number()?;
        self.status.mixer = Some(MixerStatus {
            hardware: HardwareStatus {
                versions: self.goxlr.get_firmware_version()?,
                serial_number,
                manufactured_date,
            },
            fader_a_assignment: ChannelName::Mic,
            fader_b_assignment: ChannelName::Music,
            fader_c_assignment: ChannelName::Chat,
            fader_d_assignment: ChannelName::System,
            volumes: [255; ChannelName::COUNT],
            muted: [false; ChannelName::COUNT],
            router,
        });

        Ok(())
    }

    pub fn monitor_inputs(&mut self) -> Result<()> {
        if let Some(usb_device) = &mut self.status.usb_device {
            usb_device.has_kernel_driver_attached =
                self.goxlr.usb_device_has_kernel_driver_active()?;
        }

        self.update_volumes_from_device();

        Ok(())
    }

    fn update_volumes_from_device(&mut self) {
        if let Ok((_buttons, volumes)) = self.goxlr.get_button_states() {
            if let Some(mixer) = &mut self.status.mixer {
                for fader in FaderName::iter() {
                    let channel = mixer.get_fader_assignment(fader);
                    let old_volume = mixer.get_channel_volume(channel);
                    let new_volume = volumes[fader as usize];
                    if new_volume != old_volume {
                        debug!(
                            "Updating {} volume from {} to {} as a human moved the fader",
                            channel, old_volume, new_volume
                        );
                        mixer.set_channel_volume(channel, new_volume);
                    }
                }
            }
        }
    }

    pub fn perform_command(&mut self, command: GoXLRCommand) -> Result<Option<DeviceStatus>> {
        match command {
            GoXLRCommand::GetStatus => Ok(Some(self.status.clone())),
            GoXLRCommand::AssignFader(fader, channel) => {
                self.goxlr.set_fader(fader, channel)?;
                if let Some(mixer) = &mut self.status.mixer {
                    mixer.set_fader_assignment(fader, channel);
                }
                self.goxlr.set_button_states(self.create_button_states())?;
                Ok(None)
            }
            GoXLRCommand::SetVolume(channel, volume) => {
                self.goxlr.set_volume(channel, volume)?;
                if let Some(mixer) = &mut self.status.mixer {
                    mixer.set_channel_volume(channel, volume);
                }
                Ok(None)
            }
            GoXLRCommand::SetChannelMuted(channel, muted) => {
                self.update_volumes_from_device();
                self.goxlr.set_channel_state(
                    channel,
                    if muted {
                        ChannelState::Muted
                    } else {
                        ChannelState::Unmuted
                    },
                )?;
                if let Some(mixer) = &mut self.status.mixer {
                    mixer.set_channel_muted(channel, muted);
                    if muted {
                        self.volumes_before_muted[channel as usize] =
                            mixer.get_channel_volume(channel);
                        self.goxlr.set_volume(channel, 0)?;
                    } else if mixer.get_channel_volume(channel) <= MIN_VOLUME_THRESHOLD {
                        // Don't restore the old volume if the new volume is above minimum.
                        // This seems to match the official GoXLR software behaviour.
                        self.goxlr
                            .set_volume(channel, self.volumes_before_muted[0])?;
                    }
                }
                self.goxlr.set_button_states(self.create_button_states())?;
                Ok(None)
            }
        }
    }

    fn create_button_states(&self) -> [ButtonStates; 24] {
        let mut result = [ButtonStates::DimmedColour1; 24];
        if let Some(mixer) = &self.status.mixer {
            if mixer.get_channel_muted(mixer.get_fader_assignment(FaderName::A)) {
                result[Buttons::Fader1Mute as usize] = ButtonStates::Colour1;
            }
            if mixer.get_channel_muted(mixer.get_fader_assignment(FaderName::B)) {
                result[Buttons::Fader2Mute as usize] = ButtonStates::Colour1;
            }
            if mixer.get_channel_muted(mixer.get_fader_assignment(FaderName::C)) {
                result[Buttons::Fader3Mute as usize] = ButtonStates::Colour1;
            }
            if mixer.get_channel_muted(mixer.get_fader_assignment(FaderName::D)) {
                result[Buttons::Fader4Mute as usize] = ButtonStates::Colour1;
            }
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
