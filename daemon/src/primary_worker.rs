use crate::device::Device;
use crate::events::EventTriggers;
use crate::files::extract_defaults;
use crate::platform::{has_autostart, set_autostart};
use crate::{FileManager, PatchEvent, SettingsHandle, Shutdown, VERSION};
use anyhow::{anyhow, Result};
use goxlr_ipc::{
    DaemonCommand, DaemonConfig, DaemonStatus, DeviceType, Files, GoXLRCommand, HardwareStatus,
    HttpSettings, PathTypes, Paths, UsbProductInformation,
};
use goxlr_usb::device::base::GoXLRDevice;
use goxlr_usb::device::{find_devices, from_device};
use goxlr_usb::{PID_GOXLR_FULL, PID_GOXLR_MINI};
use json_patch::diff;
use log::{error, info, warn};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::Sender as BroadcastSender;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;

// Adding a third entry has tripped enum_variant_names, I'll probably need to rename
// RunDeviceCommand, but that'll need to be in a separate commit, for now, suppress.
#[allow(clippy::enum_variant_names)]
pub enum DeviceCommand {
    SendDaemonStatus(oneshot::Sender<DaemonStatus>),
    RunDaemonCommand(DaemonCommand, oneshot::Sender<Result<()>>),
    RunDeviceCommand(String, GoXLRCommand, oneshot::Sender<Result<()>>),
}

pub type DeviceSender = Sender<DeviceCommand>;
pub type DeviceReceiver = Receiver<DeviceCommand>;

// Fix this later..
#[allow(clippy::too_many_arguments)]
pub async fn spawn_usb_handler(
    mut command_rx: DeviceReceiver,
    mut file_rx: Receiver<PathTypes>,
    mut device_stop_rx: Receiver<()>,
    broadcast_tx: BroadcastSender<PatchEvent>,
    global_tx: Sender<EventTriggers>,
    mut shutdown: Shutdown,
    settings: SettingsHandle,
    http_settings: HttpSettings,
    mut file_manager: FileManager,
) {
    // We can probably either merge these, or struct them..
    let (disconnect_sender, mut disconnect_receiver) = mpsc::channel(16);
    let (event_sender, mut event_receiver) = mpsc::channel(16);

    // Create the device detection Sleep Timer..
    let detection_duration = Duration::from_millis(1000);
    let detection_sleep = sleep(Duration::from_millis(0));
    tokio::pin!(detection_sleep);

    // Create the State update Sleep Timer..
    let update_duration = Duration::from_millis(50);
    let update_sleep = sleep(update_duration);
    tokio::pin!(update_sleep);

    // Create the Primary Device List, and 'Ignore' list..
    let mut devices: HashMap<String, Device> = HashMap::new();
    let mut ignore_list = HashMap::new();

    let mut files = get_files(&mut file_manager).await;
    let mut daemon_status =
        get_daemon_status(&devices, &settings, &http_settings, files.clone()).await;

    let mut shutdown_triggered = false;

    loop {
        let mut change_found = false;
        tokio::select! {
            () = &mut detection_sleep => {
                if let Some(device) = find_new_device(&daemon_status, &ignore_list) {
                    let existing_serials: Vec<String> = get_all_serials(&devices);
                    let bus_number = device.bus_number();
                    let address = device.address();

                    let mut device_identifier = None;
                    if let Some(identifier) = device.identifier() {
                        device_identifier = Some(identifier.clone());
                    }

                    match load_device(device, existing_serials, disconnect_sender.clone(), event_sender.clone(), global_tx.clone(), &settings).await {
                        Ok(device) => {
                            devices.insert(device.serial().to_owned(), device);
                            change_found = true;
                        }
                        Err(e) => {
                            error!(
                                "Couldn't load potential GoXLR on bus {} address {}: {}",
                                bus_number, address, e
                            );
                            ignore_list
                                .insert((bus_number, address, device_identifier), Instant::now() + Duration::from_secs(10));
                        }
                    };
                }
                detection_sleep.as_mut().reset(tokio::time::Instant::now() + detection_duration);
            },
            () = &mut update_sleep => {
                for device in devices.values_mut() {
                    let updated = device.update_state().await;

                    if let Ok(result) = updated {
                        change_found = result;
                    }

                    if let Err(error) = updated {
                        warn!("Error Received from {} while updating state: {}", device.serial(), error);
                    }
                }
                update_sleep.as_mut().reset(tokio::time::Instant::now() + update_duration);
            }
            Some(serial) = disconnect_receiver.recv() => {
                info!("[{}] Device Disconnected", serial);
                devices.remove(&serial);
                change_found = true;
            },
            Some(serial) = event_receiver.recv() => {
                if let Some(device) = devices.get_mut(&serial) {
                    let result = device.monitor_inputs().await;
                    if let Ok(changed) = result {
                        change_found = changed;
                    }

                    if let Err(error) = result {
                        warn!("Error Received from {}: {}", device.serial(), error);
                    }
                } else {
                    warn!("Cannot find registered device with serial: {}", &serial);
                }
            }
            _ = device_stop_rx.recv() => {
                // Make sure this only happens once..
                if shutdown_triggered {
                    continue;
                }
                shutdown_triggered = true;

                // Flip through all the devices, send a shutdown signal..
                for device in devices.values_mut() {
                    device.shutdown().await;
                }

                // Send a notification that we're done here..
                let _ = global_tx.send(EventTriggers::DevicesStopped).await;
            }
            () = shutdown.recv() => {
                info!("Shutting down device worker");
                return;
            },
            Some(command) = command_rx.recv() => {
                match command {
                    DeviceCommand::SendDaemonStatus(sender) => {
                        let _ = sender.send(daemon_status.clone());
                    }

                    DeviceCommand::RunDaemonCommand(command, sender) => {
                        match command {
                            DaemonCommand::StopDaemon => {
                                // These should probably be moved upstream somewhere, they're not
                                // device specific!
                                let _ = global_tx.send(EventTriggers::Stop).await;
                                let _ = sender.send(Ok(()));
                            }
                            DaemonCommand::OpenUi => {
                                let _ = global_tx.send(EventTriggers::OpenUi).await;
                                let _ = sender.send(Ok(()));
                            }
                            DaemonCommand::RecoverDefaults(path_type) => {
                                let path = match path_type {
                                    PathTypes::Profiles => settings.get_profile_directory().await,
                                    PathTypes::Presets => settings.get_presets_directory().await,
                                    PathTypes::Icons => settings.get_icons_directory().await,
                                    PathTypes::MicProfiles => settings.get_mic_profile_directory().await,
                                    _ => {
                                        let _ = sender.send(Err(anyhow!("Invalid Path type Sent")));
                                        return;
                                    }
                                };
                                let _ = sender.send(extract_defaults(path_type, &path));
                            }
                            DaemonCommand::SetAutoStartEnabled(enabled) => {
                                let _ = sender.send(set_autostart(enabled));
                            }
                            DaemonCommand::SetLogLevel(level) => {
                                settings.set_log_level(level).await;
                                settings.save().await;
                                change_found = true;
                                let _ = sender.send(Ok(()));
                            }
                            DaemonCommand::SetShowTrayIcon(enabled) => {
                                settings.set_show_tray_icon(enabled).await;
                                settings.save().await;
                                change_found = true;
                                let _ = sender.send(Ok(()));
                            }
                            DaemonCommand::SetTTSEnabled(enabled) => {
                                settings.set_tts_enabled(enabled).await;
                                settings.save().await;
                                change_found = true;
                                let _ = sender.send(Ok(()));
                            }
                            DaemonCommand::SetAllowNetworkAccess(enabled) => {
                                settings.set_allow_network_access(enabled).await;
                                settings.save().await;
                                change_found = true;
                                let _ = sender.send(Ok(()));
                            }
                            DaemonCommand::OpenPath(path_type) => {
                                // There's nothing we can really do if this errors..
                                let _ = global_tx.send(EventTriggers::Open(path_type)).await;
                                let _ = sender.send(Ok(()));
                            }
                        }
                    },

                    DeviceCommand::RunDeviceCommand(serial, command, sender) => {
                        if let Some(device) = devices.get_mut(&serial) {
                            let _ = sender.send(device.perform_command(command).await);
                            change_found = true;
                        } else {
                            let _ = sender.send(Err(anyhow!("Device {} is not connected", serial)));
                        }
                    },
                }
            },
            Some(path) = file_rx.recv() => {
                // Notify devices if Samples have changed..
                if path == PathTypes::Samples {
                    for device in devices.values_mut() {
                        let _ = device.validate_sampler().await;
                    }
                }

                files = update_files(files, path, &mut file_manager).await;
                change_found = true;
            }
        }

        if change_found {
            let new_status =
                get_daemon_status(&devices, &settings, &http_settings, files.clone()).await;

            // Convert them to JSON..
            let json_old = serde_json::to_value(&daemon_status).unwrap();
            let json_new = serde_json::to_value(&new_status).unwrap();

            let patch = diff(&json_old, &json_new);

            // Only send a patch if something has changed..
            if !patch.0.is_empty() {
                let _ = broadcast_tx.send(PatchEvent { data: patch });
            }

            // Send the patch to the tokio broadcaster, for handling by clients..
            daemon_status = new_status;
        }
    }
}

async fn get_daemon_status(
    devices: &HashMap<String, Device<'_>>,
    settings: &SettingsHandle,
    http_settings: &HttpSettings,
    files: Files,
) -> DaemonStatus {
    let mut status = DaemonStatus {
        config: DaemonConfig {
            http_settings: http_settings.clone(),
            daemon_version: String::from(VERSION),
            autostart_enabled: has_autostart(),
            show_tray_icon: settings.get_show_tray_icon().await,
            tts_enabled: settings.get_tts_enabled().await,
            allow_network_access: settings.get_allow_network_access().await,
            log_level: settings.get_log_level().await,
        },
        paths: Paths {
            profile_directory: settings.get_profile_directory().await,
            mic_profile_directory: settings.get_mic_profile_directory().await,
            samples_directory: settings.get_samples_directory().await,
            presets_directory: settings.get_presets_directory().await,
            icons_directory: settings.get_icons_directory().await,
            logs_directory: settings.get_log_directory().await,
        },
        files,
        ..Default::default()
    };

    for (serial, device) in devices {
        status
            .mixers
            .insert(serial.to_owned(), device.status().await.clone());
    }

    status
}

async fn get_files(file_manager: &mut FileManager) -> Files {
    Files {
        profiles: file_manager.get_profiles(),
        mic_profiles: file_manager.get_mic_profiles(),
        presets: file_manager.get_presets(),
        samples: file_manager.get_samples(),
        icons: file_manager.get_icons(),
    }
}

async fn update_files(files: Files, file_type: PathTypes, file_manager: &mut FileManager) -> Files {
    // Only re-poll for the changed type.
    Files {
        profiles: if file_type != PathTypes::Profiles {
            files.profiles
        } else {
            file_manager.get_profiles()
        },

        mic_profiles: if file_type != PathTypes::MicProfiles {
            files.mic_profiles
        } else {
            file_manager.get_mic_profiles()
        },

        presets: if file_type != PathTypes::Presets {
            files.presets
        } else {
            file_manager.get_presets()
        },

        samples: if file_type != PathTypes::Samples {
            files.samples
        } else {
            file_manager.get_samples()
        },

        icons: if file_type != PathTypes::Icons {
            files.icons
        } else {
            file_manager.get_icons()
        },
    }
}

fn find_new_device(
    current_status: &DaemonStatus,
    devices_to_ignore: &HashMap<(u8, u8, Option<String>), Instant>,
) -> Option<GoXLRDevice> {
    let now = Instant::now();

    let goxlr_devices = find_devices();
    goxlr_devices.into_iter().find(|device| {
        // Check the Mixers on the existing DaemonStatus..
        !current_status.mixers.values().any(|d| {
            if let Some(identifier) = device.identifier() {
                if let Some(device_identifier) = &d.hardware.usb_device.identifier {
                    return identifier.clone() == device_identifier.clone();
                }
            }
            d.hardware.usb_device.bus_number == device.bus_number()
                && d.hardware.usb_device.address == device.address()
        }) && !devices_to_ignore
            .iter()
            .any(|((bus_number, address, identifier), expires)| {
                if let Some(identifier) = identifier {
                    if let Some(device_identifier) = device.identifier() {
                        return identifier == device_identifier && expires > &now;
                    }
                }
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
    disconnect_sender: Sender<String>,
    event_sender: Sender<String>,
    global_events: Sender<EventTriggers>,
    settings: &SettingsHandle,
) -> Result<Device<'_>> {
    let device_copy = device.clone();

    let mut handled_device = from_device(device, disconnect_sender, event_sender)?;
    let descriptor = handled_device.get_descriptor()?;

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
        bus_number: device_copy.bus_number(),
        address: device_copy.address(),
        identifier: device_copy.identifier().clone(),
        version,
    };
    let (mut serial_number, manufactured_date) = handled_device.get_serial_number()?;
    if serial_number.is_empty() {
        let mut serial = String::from("");
        for i in 0..=24 {
            serial = format!("UNKNOWN-SN-{i}");
            if !existing_serials.contains(&serial) {
                break;
            }
        }

        warn!("This GoXLR isn't reporting a serial number, this may cause issues if you're running with multiple devices.");
        serial_number = serial;
        warn!("Generated Internal Serial Number: {}", serial_number);
    }
    handled_device.set_unique_identifier(serial_number.clone());

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
        global_events,
    )
    .await?;
    settings
        .set_device_profile_name(&serial_number, device.profile().name())
        .await;
    settings
        .set_device_mic_profile_name(&serial_number, device.mic_profile().name())
        .await;
    settings.save().await;
    Ok(device)
}
