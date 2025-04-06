use crate::device::base::AttachGoXLR;
use crate::device::base::FullGoXLRDevice;
use crate::device::base::GoXLRDevice;
use anyhow::Result;
use goxlr_types::{DriverInterface, VersionNumber};
use tokio::sync::mpsc::Sender;

pub mod base;

cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        // Under Windows, we need to utilise the official GoXLR Driver to communicate..
        mod tusb;
        use crate::device::tusb::device;

        pub fn get_version() -> (DriverInterface, Option<VersionNumber>) {
            device::get_interface_version()
        }

        pub fn find_devices() -> Vec<GoXLRDevice> {
            device::find_devices()
        }

        pub fn from_device(
            device: GoXLRDevice,
            disconnect_sender: Sender<String>,
            event_sender: Sender<String>,
            skip_pause: bool,
        ) -> Result<Box<dyn FullGoXLRDevice>> {
            device::TUSBAudioGoXLR::from_device(device, disconnect_sender, event_sender, skip_pause)
        }
    } else {
        // If we're using Linux / MacOS / etc, utilise libUSB for control.
        mod libusb;
        use crate::device::libusb::device;

        pub fn get_version() -> (DriverInterface, Option<VersionNumber>) {
            device::get_interface_version()
        }

        pub fn find_devices() -> Vec<GoXLRDevice> {
            device::find_devices()
        }

        pub fn from_device(
            device: GoXLRDevice,
            disconnect_sender: Sender<String>,
            event_sender: Sender<String>,
            skip_pause: bool,
        ) -> Result<Box<dyn FullGoXLRDevice>> {
            device::GoXLRUSB::from_device(device, disconnect_sender, event_sender, skip_pause)
        }
    }
}
