use crate::device::Device;
use crate::{FileManager, SettingsHandle, Shutdown};
use anyhow::{anyhow, Result};
use goxlr_ipc::{
    DaemonResponse, DaemonStatus, DeviceType, Files, GoXLRCommand, HardwareStatus, Paths,
    UsbProductInformation,
};
use goxlr_usb::goxlr::{GoXLR, PID_GOXLR_FULL, PID_GOXLR_MINI, VID_GOXLR};
use goxlr_usb::rusb;
use goxlr_usb::rusb::{DeviceDescriptor, GlobalContext};
use log::{error, info};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;

// Adding a third entry has tripped enum_variant_names, I'll probably need to rename
// RunDeviceCommand, but that'll need to be in a separate commit, for now, suppress.
#[allow(clippy::enum_variant_names)]
pub enum DeviceCommand {
    SendDaemonStatus(oneshot::Sender<DaemonStatus>),
    InvalidateCaches(oneshot::Sender<DaemonResponse>),
    RunDeviceCommand(String, GoXLRCommand, oneshot::Sender<Result<()>>),
}

pub type DeviceSender = mpsc::Sender<DeviceCommand>;
pub type DeviceReceiver = mpsc::Receiver<DeviceCommand>;

pub async fn handle_changes(
    mut rx: DeviceReceiver,
    mut shutdown: Shutdown,
    settings: SettingsHandle,
    mut file_manager: FileManager,
) {
    let detect_count = 10;
    let mut loop_count = 10;

    let sleep_duration = Duration::from_millis(100);
    let mut devices = HashMap::new();
    let mut ignore_list = HashMap::new();

    loop {
        tokio::select! {
            () = sleep(sleep_duration) => {
                if loop_count == detect_count {
                    if let Some((device, descriptor)) = find_new_device(&devices, &ignore_list) {
                    let bus_number = device.bus_number();
                    let address = device.address();
                        match load_device(device, descriptor, &settings).await {
                            Ok(device) => {
                                devices.insert(device.serial().to_owned(), device);
                            }
                            Err(e) => {
                                error!(
                                    "Couldn't load potential GoXLR on bus {} address {}: {}",
                                    bus_number, address, e
                                );
                                ignore_list
                                    .insert((bus_number, address), Instant::now() + Duration::from_secs(10));
                            }
                        };
                    }
                    loop_count = -1;
                }
                loop_count += 1;
                let mut found_error = false;
                for device in devices.values_mut() {
                    if let Err(e) = device.monitor_inputs().await {
                        error!("Couldn't monitor device for inputs: {}", e);
                        found_error = true;
                    }
                }
                if found_error {
                    devices.retain(|_, d| d.is_connected());
                }
            },
            () = shutdown.recv() => {
                info!("Shutting down device worker");
                return;
            },
            Some(command) = rx.recv() => {
                match command {
                    DeviceCommand::SendDaemonStatus(sender) => {
                        let mut status = DaemonStatus {
                            paths: Paths {
                                profile_directory: settings.get_profile_directory().await,
                                mic_profile_directory: settings.get_mic_profile_directory().await,
                                samples_directory: settings.get_samples_directory().await,
                            },
                            files: Files {
                                profiles: file_manager.get_profiles(&settings),
                                mic_profiles: file_manager.get_mic_profiles(&settings),
                            },
                            ..Default::default()
                        };
                        for (serial, device) in &devices {
                            status.mixers.insert(serial.to_owned(), device.status().clone());
                        }
                        let _ = sender.send(status);
                    },
                    DeviceCommand::InvalidateCaches(sender) => {
                        file_manager.invalidate_caches();
                        let _ = sender.send(DaemonResponse::Ok);
                    }
                    DeviceCommand::RunDeviceCommand(serial, command, sender) => {
                        if let Some(device) = devices.get_mut(&serial) {
                            let _ = sender.send(device.perform_command(command).await);
                        } else {
                            let _ = sender.send(Err(anyhow!("Device {} is not connected", serial)));
                        }
                    },
                }
            },
        };
    }
}

fn find_new_device(
    existing_devices: &HashMap<String, Device<GlobalContext>>,
    devices_to_ignore: &HashMap<(u8, u8), Instant>,
) -> Option<(rusb::Device<GlobalContext>, DeviceDescriptor)> {
    let now = Instant::now();
    if let Ok(devices) = rusb::devices() {
        for device in devices.iter() {
            if let Ok(descriptor) = device.device_descriptor() {
                let bus_number = device.bus_number();
                let address = device.address();
                if descriptor.vendor_id() == VID_GOXLR
                    && (descriptor.product_id() == PID_GOXLR_FULL
                        || descriptor.product_id() == PID_GOXLR_MINI)
                    && !existing_devices.values().any(|d| {
                        d.status().hardware.usb_device.bus_number == bus_number
                            && d.status().hardware.usb_device.address == address
                    })
                    && !devices_to_ignore
                        .iter()
                        .any(|((bus_number, address), expires)| {
                            *bus_number == device.bus_number()
                                && *address == device.address()
                                && expires > &now
                        })
                {
                    return Some((device, descriptor));
                }
            }
        }
    }
    None
}

async fn load_device(
    device: rusb::Device<GlobalContext>,
    descriptor: DeviceDescriptor,
    settings: &SettingsHandle,
) -> Result<Device<'_, GlobalContext>> {
    let mut device = GoXLR::from_device(device.open()?, descriptor)?;
    let descriptor = device.usb_device_descriptor();
    let device_type = match descriptor.product_id() {
        PID_GOXLR_FULL => DeviceType::Full,
        PID_GOXLR_MINI => DeviceType::Mini,
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
        serial_number: serial_number.clone(),
        manufactured_date,
        device_type,
        usb_device,
    };
    let profile_directory = settings.get_profile_directory().await;
    let profile_name = settings.get_device_profile_name(&serial_number).await;
    let mic_profile_name = settings.get_device_mic_profile_name(&serial_number).await;
    let mic_profile_directory = settings.get_mic_profile_directory().await;
    let device = Device::new(
        device,
        hardware,
        profile_name,
        mic_profile_name,
        &profile_directory,
        &mic_profile_directory,
        settings,
    )?;
    settings
        .set_device_profile_name(&serial_number, device.profile().name())
        .await;
    settings
        .set_device_mic_profile_name(&serial_number, device.mic_profile().name())
        .await;
    settings.save().await;
    Ok(device)
}
