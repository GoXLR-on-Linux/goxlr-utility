use enumset::EnumSet;
use goxlr_types::{ChannelName, FaderName, FirmwareVersions, InputDevice, MicrophoneType, MuteFunction, OutputDevice};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use strum::EnumCount;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub mixers: HashMap<String, MixerStatus>,
    pub profile_directory: PathBuf,
    pub mic_profile_directory: PathBuf,
    pub samples_directory: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareStatus {
    pub versions: FirmwareVersions,
    pub serial_number: String,
    pub manufactured_date: String,
    pub device_type: DeviceType,
    pub usb_device: UsbProductInformation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaderStatus {
    pub channel: ChannelName,
    pub mute_type: MuteFunction
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerStatus {
    pub hardware: HardwareStatus,
    pub fader_a_assignment: FaderStatus,
    pub fader_b_assignment: FaderStatus,
    pub fader_c_assignment: FaderStatus,
    pub fader_d_assignment: FaderStatus,
    pub volumes: [u8; ChannelName::COUNT],
    pub router: [EnumSet<OutputDevice>; InputDevice::COUNT],
    pub mic_gains: [u16; MicrophoneType::COUNT],
    pub mic_type: MicrophoneType,
    pub profile_name: String,
    pub mic_profile_name: String,
}

impl MixerStatus {
    pub fn get_fader_assignment(&self, fader: FaderName) -> &FaderStatus {
        match fader {
            FaderName::A => &self.fader_a_assignment,
            FaderName::B => &self.fader_b_assignment,
            FaderName::C => &self.fader_c_assignment,
            FaderName::D => &self.fader_d_assignment,
        }
    }

    pub fn set_fader_assignment(&mut self, fader: FaderName, status: FaderStatus) {
        match fader {
            FaderName::A => self.fader_a_assignment = status,
            FaderName::B => self.fader_b_assignment = status,
            FaderName::C => self.fader_c_assignment = status,
            FaderName::D => self.fader_d_assignment = status,
        }
    }

    pub fn get_channel_volume(&self, channel: ChannelName) -> u8 {
        return self.volumes[channel as usize];
    }

    pub fn set_channel_volume(&mut self, channel: ChannelName, volume: u8) {
        self.volumes[channel as usize] = volume;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbProductInformation {
    pub manufacturer_name: String,
    pub product_name: String,
    pub version: (u8, u8, u8),
    pub is_claimed: bool,
    pub has_kernel_driver_attached: bool,
    pub bus_number: u8,
    pub address: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeviceType {
    Unknown,
    Full,
    Mini,
}

impl Default for DeviceType {
    fn default() -> Self {
        DeviceType::Unknown
    }
}
