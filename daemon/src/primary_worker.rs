use crate::device::Device;
use crate::Shutdown;
use anyhow::{anyhow, Result};
use goxlr_ipc::{DaemonStatus, DeviceType, GoXLRCommand, HardwareStatus, UsbProductInformation};
use goxlr_usb::goxlr;
use goxlr_usb::goxlr::GoXLR;
use goxlr_usb::rusb::GlobalContext;
use log::{error, info};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;

pub enum DeviceCommand {
    SendDaemonStatus(oneshot::Sender<DaemonStatus>),
    RunDeviceCommand(String, GoXLRCommand, oneshot::Sender<Result<()>>),
}

pub type DeviceSender = mpsc::Sender<DeviceCommand>;
pub type DeviceReceiver = mpsc::Receiver<DeviceCommand>;

pub async fn handle_changes(mut rx: DeviceReceiver, mut shutdown: Shutdown) {
    let sleep_duration = Duration::from_millis(100);
    let mut devices = HashMap::new();

    loop {
        tokio::select! {
            () = sleep(sleep_duration) => {
                // TODO: Make GoXLR::open() take a specific hardware device
                if devices.is_empty() {
                    match load_new_device() {
                        Ok(device) => {
                            devices.insert(device.serial().to_owned(), device);
                        },
                        Err(e) => {
                            error!("Error initializing device: {}", e);
                        },
                    }
                }
                for device in devices.values_mut() {
                    if let Err(e) = device.monitor_inputs() {
                        error!("Couldn't monitor device for inputs: {}", e);
                    }
                }
            },
            () = shutdown.recv() => {
                info!("Shutting down device worker");
                return;
            },
            Some(command) = rx.recv() => {
                match command {
                    DeviceCommand::SendDaemonStatus(sender) => {
                        let mut status = DaemonStatus::default();
                        for (serial, device) in &devices {
                            status.mixers.insert(serial.to_owned(), device.status().clone());
                        }
                        let _ = sender.send(status);
                    },
                    DeviceCommand::RunDeviceCommand(serial, command, sender) => {
                        if let Some(device) = devices.get_mut(&serial) {
                            let _ = sender.send(device.perform_command(command));
                        } else {
                            let _ = sender.send(Err(anyhow!("Device {} is not connected", serial)));
                        }
                    },
                }
            },
        };
    }
}

fn load_new_device() -> Result<Device<GlobalContext>> {
    let mut device = GoXLR::open()?;
    let descriptor = device.usb_device_descriptor();
    let device_type = match descriptor.product_id() {
        goxlr::PID_GOXLR_FULL => DeviceType::Full,
        goxlr::PID_GOXLR_MINI => DeviceType::Mini,
        _ => DeviceType::Unknown,
    };
    let device_version = descriptor.device_version();
    let version = (device_version.0, device_version.1, device_version.2);
    let usb_device = UsbProductInformation {
        manufacturer_name: device.usb_device_manufacturer()?,
        product_name: device.usb_device_product_name()?,
        is_claimed: device.usb_device_is_claimed(),
        has_kernel_driver_attached: device.usb_device_has_kernel_driver_active()?,
        bus_number: device.usb_bus_number(),
        address: device.usb_address(),
        version,
    };
    let (serial_number, manufactured_date) = device.get_serial_number()?;
    let hardware = HardwareStatus {
        versions: device.get_firmware_version()?,
        serial_number,
        manufactured_date,
        device_type,
        usb_device,
    };
    Device::new(device, hardware)
}
