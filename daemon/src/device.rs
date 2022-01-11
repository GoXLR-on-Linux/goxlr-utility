use anyhow::Result;
use goxlr_ipc::{DeviceStatus, DeviceType, GoXLRCommand, MixerStatus, UsbProductInformation};
use goxlr_types::{ChannelName, FaderName};
use goxlr_usb::channelstate::ChannelState;
use goxlr_usb::goxlr;
use goxlr_usb::goxlr::GoXLR;
use goxlr_usb::rusb::UsbContext;
use std::time::Duration;
use strum::IntoEnumIterator;

#[derive(Debug)]
pub struct Device<T: UsbContext> {
    goxlr: GoXLR<T>,
    status: DeviceStatus,
}

impl<T: UsbContext> Device<T> {
    pub fn new(goxlr: GoXLR<T>) -> Self {
        Self {
            goxlr,
            status: DeviceStatus::default(),
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
        self.goxlr.set_fader(FaderName::B, ChannelName::Chat)?;
        self.goxlr.set_fader(FaderName::C, ChannelName::Music)?;
        self.goxlr.set_fader(FaderName::D, ChannelName::System)?;
        for channel in ChannelName::iter() {
            self.goxlr.set_volume(channel, 255)?;
            self.goxlr
                .set_channel_state(channel, ChannelState::Unmuted)?;
        }
        self.status.mixer = Some(MixerStatus {
            fader_a_assignment: ChannelName::Mic,
            fader_b_assignment: ChannelName::Chat,
            fader_c_assignment: ChannelName::Music,
            fader_d_assignment: ChannelName::System,
            mic_volume: 255,
            line_in_volume: 255,
            console_volume: 255,
            system_volume: 255,
            game_volume: 255,
            chat_volume: 255,
            sample_volume: 255,
            music_volume: 255,
            headphones_volume: 255,
            mic_monitor_volume: 255,
            line_out_volume: 255,
            mic_muted: false,
            line_in_muted: false,
            console_muted: false,
            system_muted: false,
            game_muted: false,
            chat_muted: false,
            sample_muted: false,
            music_muted: false,
            headphones_muted: false,
            mic_monitor_muted: false,
            line_out_muted: false,
        });

        Ok(())
    }

    pub fn monitor_inputs(&mut self) -> Result<()> {
        if let Some(usb_device) = &mut self.status.usb_device {
            usb_device.has_kernel_driver_attached =
                self.goxlr.usb_device_has_kernel_driver_active()?;
        }

        let interrupt_duration = Duration::from_secs(1);
        if self.goxlr.await_interrupt(interrupt_duration) {
            if let Ok(buttons) = self.goxlr.get_button_states() {
                dbg!(buttons);
            }
        }

        Ok(())
    }

    pub fn perform_command(&mut self, command: GoXLRCommand) -> Result<Option<DeviceStatus>> {
        match command {
            GoXLRCommand::GetStatus => Ok(Some(self.status.clone())),
            GoXLRCommand::AssignFader(fader, channel) => {
                self.goxlr.set_fader(fader, channel)?;
                if let Some(mixer) = &mut self.status.mixer {
                    mixer.set_fader_assignment(fader, channel);
                }
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
                }
                Ok(None)
            }
        }
    }
}
