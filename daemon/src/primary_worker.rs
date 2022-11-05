use crate::device::Device;
use crate::files::create_path;
use crate::{FileManager, SettingsHandle, Shutdown};
use anyhow::{anyhow, Result};
use goxlr_ipc::{
    DaemonResponse, DaemonStatus, DeviceType, Files, GoXLRCommand, HardwareStatus, PathTypes,
    Paths, UsbProductInformation,
};
use goxlr_usb::device::base::{find_devices, from_device, GoXLRDevice};
use goxlr_usb::goxlr::{PID_GOXLR_FULL, PID_GOXLR_MINI};
use log::{error, info, warn};
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
    OpenPath(PathTypes, oneshot::Sender<DaemonResponse>),
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

    // Attempt to create the needed paths..
    if let Err(error) = create_path(&settings.get_profile_directory().await) {
        warn!("Unable to create profile directory: {}", error);
    }

    if let Err(error) = create_path(&settings.get_mic_profile_directory().await) {
        warn!("Unable to create mic profile directory: {}", error);
    }

    let samples_path = &settings.get_samples_directory().await;
    if let Err(error) = create_path(samples_path) {
        warn!("Unable to create samples directory: {}", error);
    }

    let recorded_path = samples_path.join("Recorded/");
    if let Err(error) = create_path(&recorded_path) {
        warn!("Unable to create samples directory: {}", error);
    }

    loop {
        tokio::select! {
            () = sleep(sleep_duration) => {
                if loop_count == detect_count {
                    if let Some(device) = find_new_device(&devices, &ignore_list) {
                    let existing_serials: Vec<String> = get_all_serials(&devices);
                    let bus_number = device.bus_number();
                    let address = device.address();
                        match load_device(device, existing_serials, &settings).await {
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
                                presets_directory: settings.get_presets_directory().await,
                            },
                            files: Files {
                                profiles: file_manager.get_profiles(&settings),
                                mic_profiles: file_manager.get_mic_profiles(&settings),
                                presets: file_manager.get_presets(&settings),
                                samples: file_manager.get_samples(&settings),
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
                    DeviceCommand::OpenPath(path_type, sender) => {
                        let result = opener::open(match path_type {
                            PathTypes::Profiles => settings.get_profile_directory().await,
                            PathTypes::MicProfiles => settings.get_mic_profile_directory().await,
                            PathTypes::Presets => settings.get_presets_directory().await,
                            PathTypes::Samples => settings.get_samples_directory().await,
                        });
                        if result.is_err() {
                            let _ = sender.send(DaemonResponse::Error("Unable to Open".to_string()));
                        } else {
                            let _ = sender.send(DaemonResponse::Ok);
                        }
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
    existing_devices: &HashMap<String, Device>,
    devices_to_ignore: &HashMap<(u8, u8), Instant>,
) -> Option<GoXLRDevice> {
    let now = Instant::now();

    let goxlr_devices = find_devices();
    goxlr_devices.into_iter().find(|&device| {
        !existing_devices.values().any(|d| {
            d.status().hardware.usb_device.bus_number == device.bus_number()
                && d.status().hardware.usb_device.address == device.address()
        }) && !devices_to_ignore
            .iter()
            .any(|((bus_number, address), expires)| {
                *bus_number == device.bus_number() && *address == device.address() && expires > &now
            })
    })
}

fn get_all_serials(existing_devices: &HashMap<String, Device>) -> Vec<String> {
    let mut serials: Vec<String> = vec![];

    for device in existing_devices {
        serials.push(device.0.clone());
    }

    serials
}

async fn load_device(
    device: GoXLRDevice,
    existing_serials: Vec<String>,
    settings: &SettingsHandle,
) -> Result<Device<'_>> {
    let mut handled_device = from_device(device)?;
    let descriptor = handled_device.get_descriptor()?;

    //let mut device = GoXLR::from_device(device.open()?, descriptor)?;

    let device_type = match descriptor.product_id() {
        PID_GOXLR_FULL => DeviceType::Full,
        PID_GOXLR_MINI => DeviceType::Mini,
        _ => DeviceType::Unknown,
    };
    let device_version = descriptor.device_version();
    let version = (device_version.0, device_version.1, device_version.2);
    let usb_device = UsbProductInformation {
        manufacturer_name: descriptor.device_manufacturer(),
        product_name: descriptor.product_name(),
        bus_number: device.bus_number(),
        address: device.address(),
        version,
    };
    let (mut serial_number, manufactured_date) = handled_device.get_serial_number()?;
    if serial_number.is_empty() {
        let mut serial = String::from("");
        for i in 0..=24 {
            serial = format!("UNKNOWN-SN-{}", i);
            if !existing_serials.contains(&serial) {
                break;
            }
        }

        warn!("This GoXLR isn't reporting a serial number, this may cause issues if you're running with multiple devices.");
        serial_number = serial;
        warn!("Generated Internal Serial Number: {}", serial_number);
    }

    let hardware = HardwareStatus {
        versions: handled_device.get_firmware_version()?,
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
        handled_device,
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
