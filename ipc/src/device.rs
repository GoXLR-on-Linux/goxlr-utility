use goxlr_types::{ChannelName, FaderName};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviceStatus {
    pub device_type: DeviceType,
    pub usb_device: Option<UsbProductInformation>,
    pub mixer: Option<MixerStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerStatus {
    pub fader_a_assignment: ChannelName,
    pub fader_b_assignment: ChannelName,
    pub fader_c_assignment: ChannelName,
    pub fader_d_assignment: ChannelName,
}

impl MixerStatus {
    pub fn get_fader_assignment(&self, fader: FaderName) -> ChannelName {
        match fader {
            FaderName::A => self.fader_a_assignment,
            FaderName::B => self.fader_b_assignment,
            FaderName::C => self.fader_c_assignment,
            FaderName::D => self.fader_d_assignment,
        }
    }

    pub fn set_fader_assignment(&mut self, fader: FaderName, channel: ChannelName) {
        match fader {
            FaderName::A => self.fader_a_assignment = channel,
            FaderName::B => self.fader_b_assignment = channel,
            FaderName::C => self.fader_c_assignment = channel,
            FaderName::D => self.fader_d_assignment = channel,
        }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
