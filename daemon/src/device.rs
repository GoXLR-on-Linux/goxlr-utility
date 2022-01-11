use anyhow::Result;
use goxlr_ipc::{DeviceStatus, DeviceType, GoXLRCommand, UsbProductInformation};
use goxlr_usb::goxlr;
use goxlr_usb::goxlr::GoXLR;
use goxlr_usb::rusb::UsbContext;
use std::time::Duration;

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
                Ok(None)
            }
        }
    }
}
