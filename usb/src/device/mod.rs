use crate::device::base::FullGoXLRDevice;
use crate::device::base::GoXLRDevice;
use crate::device::base::{AttachGoXLR, PnPMessage};
use anyhow::Result;
use tokio::sync::mpsc::{Receiver, Sender};

pub mod base;

cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        // Under Windows, we need to utilise the official GoXLR Driver to communicate..
        mod tusb;
        use crate::device::tusb::device;

        pub async fn start_pnp_handle(receiver: Receiver<PnPMessage>) {
            device::start_pnp_handler(receiver).await
        }

        pub fn find_devices() -> Vec<GoXLRDevice> {
            device::find_devices()
        }

        pub fn from_device(
            device: GoXLRDevice,
            disconnect_sender: Sender<String>,
            event_sender: Sender<String>,
        ) -> Result<Box<dyn FullGoXLRDevice>> {
            device::TUSBAudioGoXLR::from_device(device, disconnect_sender, event_sender)
        }
    } else {
        // If we're using Linux / MacOS / etc, utilise libUSB for control.
        mod libusb;
        use crate::device::libusb::device;

        pub fn find_devices() -> Vec<GoXLRDevice> {
            device::find_devices()
        }

        pub fn from_device(
            device: GoXLRDevice,
            disconnect_sender: Sender<String>,
            event_sender: Sender<String>,
        ) -> Result<Box<dyn FullGoXLRDevice>> {
            device::GoXLRUSB::from_device(device, disconnect_sender, event_sender)
        }
    }
}
