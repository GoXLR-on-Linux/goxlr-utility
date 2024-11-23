use crate::device::Device;
use crate::events::EventTriggers;
use crate::files::extract_defaults;
use crate::firmware::firmware_update::{
    do_firmware_update, FirmwareMessages, FirmwareRequest, FirmwareUpdateDevice,
    FirmwareUpdateSettings,
};
use crate::platform::{get_ui_app_path, has_autostart, set_autostart};
use crate::{
    FileManager, PatchEvent, SettingsHandle, Shutdown, FIRMWARE_BASE, SYSTEM_LOCALE, VERSION,
};
use anyhow::{anyhow, Result};
use enum_map::EnumMap;
use goxlr_ipc::{
    Activation, ColourWay, DaemonCommand, DaemonConfig, DaemonStatus, DriverDetails, Files,
    FirmwareStatus, GoXLRCommand, HardwareStatus, HttpSettings, Locale, PathTypes, Paths,
    SampleFile, UpdateState, UsbProductInformation,
};
use goxlr_types::{DeviceType, VersionNumber};
use goxlr_usb::device::base::GoXLRDevice;
use goxlr_usb::device::{find_devices, from_device, get_version};
use goxlr_usb::{PID_GOXLR_FULL, PID_GOXLR_MINI};
use json_patch::diff;
use log::{debug, error, info, warn};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::Sender as BroadcastSender;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;
use xmltree::Element;

const IGNORE_DEVICE_DURATION: Duration = Duration::from_secs(10);
const APP_CHECK_INTERVAL: Duration = Duration::from_secs(30);

// Adding a third entry has tripped enum_variant_names, I'll probably need to rename
// RunDeviceCommand, but that'll need to be in a separate commit, for now, suppress.
#[allow(clippy::enum_variant_names)]
pub enum DeviceCommand {
    SendDaemonStatus(oneshot::Sender<DaemonStatus>),
    RunDaemonCommand(DaemonCommand, oneshot::Sender<Result<()>>),
    RunDeviceCommand(String, GoXLRCommand, oneshot::Sender<Result<()>>),
    GetDeviceMicLevel(String, oneshot::Sender<Result<f64>>),
    RunFirmwareUpdate(String, Option<PathBuf>, oneshot::Sender<Result<()>>),
    ClearFirmwareState(String, oneshot::Sender<Result<()>>),
}

#[allow(dead_code)]
pub enum DeviceStateChange {
    Shutdown(bool),
    Sleep(oneshot::Sender<()>),
    Wake(oneshot::Sender<()>),
}

pub type DeviceSender = Sender<DeviceCommand>;
pub type DeviceReceiver = Receiver<DeviceCommand>;

// Fix this later..
#[allow(clippy::too_many_arguments)]
pub async fn spawn_usb_handler(
    mut command_rx: DeviceReceiver,
    mut file_rx: Receiver<PathTypes>,
    mut device_state_rx: Receiver<DeviceStateChange>,
    broadcast_tx: BroadcastSender<PatchEvent>,
    global_tx: Sender<EventTriggers>,
    mut shutdown: Shutdown,
    settings: SettingsHandle,
    http_settings: HttpSettings,
    mut file_manager: FileManager,
) {
    let mut firmware_version = None;

    // We can probably either merge these, or struct them..
    let (disconnect_sender, mut disconnect_receiver) = mpsc::channel(16);
    let (event_sender, mut event_receiver) = mpsc::channel(16);
    let (firmware_sender, mut firmware_receiver) = mpsc::channel(1);

    // The channel size depends on how many GoXLRs are simultaneously connected and performing
    // firmware updates.. Let's say.. 6?
    let (firmware_update_sender, mut firmware_update_receiver) = mpsc::channel(6);

    // Spawn a task in the background to check for the latest firmware versions.
    tokio::spawn(check_firmware_versions(firmware_sender));

    // Create the device detection Sleep Timer..
    let detection_duration = Duration::from_millis(1000);
    let detection_sleep = sleep(Duration::from_millis(0));
    tokio::pin!(detection_sleep);

    // Create the State update Sleep Timer..
    let update_duration = Duration::from_millis(50);
    let update_sleep = sleep(update_duration);
    tokio::pin!(update_sleep);

    // Timer for checking whether the UI App has appeared
    let mut app_check: Option<String> = None;
    get_app_path(&mut app_check);

    let app_duration = APP_CHECK_INTERVAL;
    let app_sleep = sleep(app_duration);
    tokio::pin!(app_sleep);

    // Get the Driver Type and Details..
    let (interface, version) = get_version();
    let driver_interface = DriverDetails { interface, version };

    // Create the Primary Device List, and 'Ignore' list..
    let mut devices: HashMap<String, Device> = HashMap::new();
    let mut devices_firmware: HashMap<String, Option<FirmwareStatus>> = HashMap::new();
    let mut ignore_list = HashMap::new();

    let mut files = get_files(&mut file_manager, &settings).await;
    let mut daemon_status = get_daemon_status(
        &devices,
        &settings,
        &http_settings,
        &driver_interface,
        &firmware_version,
        &devices_firmware,
        files.clone(),
        &app_check,
    )
    .await;

    let mut shutdown_triggered = false;

    loop {
        let mut change_found = false;
        tokio::select! {
            Some(version) = firmware_receiver.recv() => {
                // Uncomment this for testing purposes!
                // use enum_map::enum_map;
                // let version = enum_map! {
                //     DeviceType::Mini => {
                //         Some(VersionNumber::from(String::from("0.0.0.0")))
                //     },
                //     DeviceType::Full => {
                //         Some(VersionNumber::from(String::from("0.0.0.0")))
                //     },
                //     DeviceType::Unknown => {
                //         Some(VersionNumber::from(String::from("0.0.0.0")))
                //     }
                // };

                firmware_version = Some(version);
                change_found = true;
            },
            Some(received) = firmware_update_receiver.recv() => {
                match received {
                    FirmwareRequest::SetUpdateState(serial,state) => {
                        if let Some(Some(status)) = devices_firmware.get_mut(&serial) {
                            status.state = state;
                            status.progress = 0;
                            change_found = true;
                        } else {
                            // We don't have a state for this device, set one.
                            let state = FirmwareStatus {
                                state,
                                progress: 0,
                                error: None
                            };

                            // Only create this if the serial is present..
                            if devices.contains_key(&serial) {
                                devices_firmware.insert(serial, Some(state));
                                change_found = true;
                            }
                        }
                    }
                    FirmwareRequest::SetStateProgress(serial, progress) => {
                        if let Some(Some(status)) = devices_firmware.get_mut(&serial) {
                            status.progress = progress;
                            change_found = true;
                        } else {
                            error!("Update State does not exist! Ignoring.");
                        }
                    }
                    FirmwareRequest::SetError(serial, error) => {
                        debug!("Setting Error: {}", error);
                        if let Some(Some(status)) = devices_firmware.get_mut(&serial) {
                            status.error = Some(error);
                            change_found = true;
                        } else {
                            error!("Update State does not exist! Ignoring..");
                        }
                    }
                    FirmwareRequest::FirmwareMessage(serial,message) => {
                        if let Some(device) = devices.get_mut(&serial) {
                            // Pass this to the device to manage
                            device.handle_firmware_message(message).await;
                            change_found = true;
                        } else {
                            // We need to locate the sender, and inform it that the device is gone, these
                            // will pop back a SetError.
                            match message {
                                FirmwareMessages::EnterDFUMode(sender) => {
                                    let _ = sender.send(Err(anyhow!("Device Not Found!")));
                                }
                                FirmwareMessages::BeginEraseNVR(sender) => {
                                    let _ = sender.send(Err(anyhow!("Device Not Found!")));
                                }
                                FirmwareMessages::PollEraseNVR(sender) => {
                                    let _ = sender.send(Err(anyhow!("Device Not Found!")));
                                }
                                FirmwareMessages::UploadFirmwareChunk(_,sender) => {
                                    let _ = sender.send(Err(anyhow!("Device Not Found!")));
                                }
                                FirmwareMessages::ValidateUploadChunk(_,sender) => {
                                    let _ = sender.send(Err(anyhow!("Device Not Found!")));
                                }
                                FirmwareMessages::BeginHardwareVerify(sender) => {
                                    let _ = sender.send(Err(anyhow!("Device Not Found!")));
                                }
                                FirmwareMessages::PollHardwareVerify(sender) => {
                                    let _ = sender.send(Err(anyhow!("Device Not Found!")));
                                }
                                FirmwareMessages::BeginHardwareWrite(sender) => {
                                    let _ = sender.send(Err(anyhow!("Device Not Found!")));
                                }
                                FirmwareMessages::PollHardwareWrite(sender) => {
                                    let _ = sender.send(Err(anyhow!("Device Not Found!")));
                                }
                                FirmwareMessages::RebootGoXLR(sender) => {
                                    let _ = sender.send(Err(anyhow!("Device Not Found!")));
                                }
                            }
                        }
                    }
                }
            }
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
                            let serial = String::from(device.serial());

                            devices.insert(serial.clone(), device);
                            devices_firmware.insert(serial, None);
                            change_found = true;
                        }
                        Err(e) => {
                            error!(
                                "Couldn't load potential GoXLR on bus {} address {}: {}",
                                bus_number, address, e
                            );
                            ignore_list
                                .insert((bus_number, address, device_identifier), Instant::now() + IGNORE_DEVICE_DURATION);
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
            },
            () = &mut app_sleep => {
                if get_app_path(&mut app_check) {
                    change_found = true;
                }
                app_sleep.as_mut().reset(tokio::time::Instant::now() + APP_CHECK_INTERVAL);
            },
            Some(serial) = disconnect_receiver.recv() => {
                info!("[{}] Device Disconnected", serial);
                devices.remove(&serial);
                if devices_firmware.contains_key(&serial) && devices_firmware.get(&serial).unwrap().is_none() {
                    // Only remove if we have no status (prevents the reboot from clearing the state)
                    devices_firmware.remove(&serial);
                }
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
            Some(event) = device_state_rx.recv() => {
                match event {
                    DeviceStateChange::Shutdown(avoid_write) => {
                        // Make sure this only happens once..
                        if shutdown_triggered {
                            continue;
                        }
                        shutdown_triggered = true;

                        // Flip through all the devices, send a shutdown signal..
                        for device in devices.values_mut() {
                            device.shutdown(avoid_write).await;
                        }

                        // Send a notification that we're done here..
                        let _ = global_tx.send(EventTriggers::DevicesStopped).await;
                    },
                    DeviceStateChange::Sleep(sender) => {
                        debug!("Received Sleep Notification");
                        for device in devices.values_mut() {
                            device.sleep().await;
                        }
                        let _ = sender.send(());
                    },
                    DeviceStateChange::Wake(sender) => {
                        debug!("Received Wake Notification");
                        for device in devices.values_mut() {
                            device.wake().await;
                        }
                        let _ = sender.send(());
                    }
                }


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
                                let _ = global_tx.send(EventTriggers::Stop(false)).await;
                                let _ = sender.send(Ok(()));
                            }
                            DaemonCommand::OpenUi => {
                                let _ = global_tx.send(EventTriggers::OpenUi).await;
                                let _ = sender.send(Ok(()));
                            }
                            DaemonCommand::Activate => {
                                let _ = global_tx.send(EventTriggers::Activate).await;
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
                                change_found = true;
                            }
                            DaemonCommand::SetLogLevel(level) => {
                                settings.set_log_level(level).await;
                                settings.save().await;
                                change_found = true;
                                let _ = sender.send(Ok(()));
                            }
                            DaemonCommand::SetLocale(language) => {
                                settings.set_selected_locale(language).await;
                                settings.save().await;
                                change_found = true;
                                let _ = sender.send(Ok(()));
                            }
                            DaemonCommand::SetUiLaunchOnLoad(value) => {
                                settings.set_open_ui_on_launch(value).await;
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
                            DaemonCommand::SetSampleGainPct(sample, gain) => {
                                settings.set_sample_gain_percent(sample, gain).await;
                                let _ = sender.send(Ok(()));
                            }
                            DaemonCommand::ApplySampleChange => {
                                // Change is committed, save it..
                                settings.save().await;

                                // Resend the value.
                                files = update_files(files, PathTypes::Samples, &mut file_manager, &settings).await;
                                change_found = true;
                                let _ = sender.send(Ok(()));
                            }
                            DaemonCommand::SetActivatorPath(path) => {
                                if let Some(path) = path {
                                    settings.set_activate(Some(path.to_string_lossy().to_string())).await;
                                    settings.save().await;
                                } else {
                                    settings.set_activate(None).await;
                                    settings.save().await;
                                }
                                change_found = true;
                                let _ = sender.send(Ok(()));
                            }
                        }
                    },

                    DeviceCommand::RunDeviceCommand(serial, command, sender) => {
                        if let Some(device) = devices.get_mut(&serial) {
                            let result = match device.perform_command(command.clone()).await {
                                Ok(result) => {
                                    Ok(result)
                                }
                                Err(error) => {
                                    warn!("Error Executing: {:?}, {}", command, error);
                                    Err(error)
                                }
                            };
                            let _ = sender.send(result);
                            change_found = true;
                        } else {
                            let _ = sender.send(Err(anyhow!("Device {} is not connected", serial)));
                        }
                    },

                    DeviceCommand::GetDeviceMicLevel(serial, sender) => {
                        if let Some(device) = devices.get_mut(&serial) {
                            let _ = sender.send(device.get_mic_level().await);
                        } else {
                            let _ = sender.send(Err(anyhow!("Device {} is not connected", serial)));
                        }
                    },

                    DeviceCommand::RunFirmwareUpdate(serial, file, sender) => {
                        if let Some(device) = devices.get_mut(&serial) {
                            let device_type = device.get_hardware_type();
                            let current_firmware = device.get_firmware_version();

                            // Create and run a firmware updater for this device
                            let update_settings = FirmwareUpdateSettings {
                                sender: firmware_update_sender.clone(),
                                device: FirmwareUpdateDevice {
                                    serial,
                                    device_type,
                                    current_firmware,
                                },
                                file,
                            };
                            tokio::spawn(do_firmware_update(update_settings));
                            let _ = sender.send(Ok(()));
                        } else {
                            let _ = sender.send(Err(anyhow!("Device {} is not connected", serial)));
                        }
                    },

                    DeviceCommand::ClearFirmwareState(serial, sender) => {
                        if let Some(device) = devices_firmware.get_mut(&serial) {
                            if let Some(status) = device {
                                if status.state != UpdateState::Complete && status.state != UpdateState::Failed {
                                    let _ = sender.send(Err(anyhow!("Cannot Clear, update in progress")));
                                } else {
                                    device.take();

                                    if !devices.contains_key(&serial) {
                                        // If the device is no longer attached, remove it.
                                        devices.remove(&serial);
                                    }
                                }
                            } else {
                                let _ = sender.send(Err(anyhow!("Device not performing firmware update")));
                            }
                        } else {
                            let _ = sender.send(Err(anyhow!("Device not Present")));
                        }
                    }
                }
            },
            Some(path) = file_rx.recv() => {
                // Notify devices if Samples have changed..
                if path == PathTypes::Samples {
                    for device in devices.values_mut() {
                        let _ = device.validate_sampler().await;
                    }
                }

                files = update_files(files, path, &mut file_manager, &settings).await;
                change_found = true;
            }
        }

        if change_found {
            let new_status = get_daemon_status(
                &devices,
                &settings,
                &http_settings,
                &driver_interface,
                &firmware_version,
                &devices_firmware,
                files.clone(),
                &app_check,
            )
            .await;

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

// Dis getting big..
#[allow(clippy::too_many_arguments)]
async fn get_daemon_status(
    devices: &HashMap<String, Device<'_>>,
    settings: &SettingsHandle,
    http_settings: &HttpSettings,
    driver_details: &DriverDetails,
    firmware_versions: &Option<EnumMap<DeviceType, Option<VersionNumber>>>,
    firmware_state: &HashMap<String, Option<FirmwareStatus>>,
    files: Files,
    app_check: &Option<String>,
) -> DaemonStatus {
    let mut status = DaemonStatus {
        config: DaemonConfig {
            http_settings: http_settings.clone(),
            daemon_version: String::from(VERSION),
            driver_interface: driver_details.clone(),
            latest_firmware: firmware_versions.clone(),
            locale: Locale {
                user_locale: settings.get_selected_locale().await,
                system_locale: SYSTEM_LOCALE.clone(),
            },
            autostart_enabled: has_autostart(),
            show_tray_icon: settings.get_show_tray_icon().await,
            tts_enabled: settings.get_tts_enabled().await,
            allow_network_access: settings.get_allow_network_access().await,
            log_level: settings.get_log_level().await,
            open_ui_on_launch: settings.get_open_ui_on_launch().await,
            activation: Activation {
                active_path: settings.get_activate().await,
                app_path: app_check.clone(),
            },
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

    for (serial, state) in firmware_state {
        status.firmware.insert(serial.to_owned(), state.clone());
    }

    for (serial, device) in devices {
        status
            .mixers
            .insert(serial.to_owned(), device.status().await.clone());
    }

    status
}

#[allow(const_item_mutation)]
fn get_app_path(app_check: &mut Option<String>) -> bool {
    if let Some(path) = get_ui_app_path() {
        let mut changed = false;

        // We need to escape the value..
        let wrap = if cfg!(windows) { "\"" } else { "'" };
        let path = format!("{}{}{}", wrap, path.to_string_lossy(), wrap);

        if let Some(old_path) = app_check {
            if *old_path != path {
                app_check.replace(path);
                changed = true;
            }
        } else {
            app_check.replace(path);
            changed = true
        }

        changed
    } else {
        let mut changed = false;
        if app_check.is_some() {
            changed = true;
        }
        app_check.take();
        changed
    }
}

async fn get_sample_files(
    file_manager: &mut FileManager,
    settings: &SettingsHandle,
) -> BTreeMap<String, SampleFile> {
    let file_samples = file_manager.get_samples();
    let config_samples = settings.get_sample_gain_list().await;

    // We need to pair the two together, starting with the file samples..
    let mut samples: BTreeMap<String, SampleFile> = Default::default();
    for (key, value) in file_samples {
        let mut gain = 100;

        if let Some(config_gain) = config_samples.get(&*value) {
            gain = *config_gain;
        }

        samples.insert(
            key,
            SampleFile {
                name: value,
                gain_pct: gain,
            },
        );
    }
    samples
}

async fn get_files(file_manager: &mut FileManager, settings: &SettingsHandle) -> Files {
    Files {
        profiles: file_manager.get_profiles(),
        mic_profiles: file_manager.get_mic_profiles(),
        presets: file_manager.get_presets(),
        samples: get_sample_files(file_manager, settings).await,
        icons: file_manager.get_icons(),
    }
}

async fn update_files(
    files: Files,
    file_type: PathTypes,
    file_manager: &mut FileManager,
    settings: &SettingsHandle,
) -> Files {
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
            get_sample_files(file_manager, settings).await
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

    let mut handled_device = from_device(device, disconnect_sender, event_sender, false)?;
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

    let colour_way = if serial_number.ends_with("AAI") || serial_number.ends_with("3AA") {
        ColourWay::White
    } else {
        ColourWay::Black
    };

    let hardware = HardwareStatus {
        versions: handled_device.get_firmware_version()?,
        serial_number: serial_number.clone(),
        manufactured_date,
        device_type,
        colour_way,
        usb_device,
    };
    let device = Device::new(handled_device, hardware, settings, global_events).await?;
    settings
        .set_device_profile_name(&serial_number, device.profile().name())
        .await;
    settings
        .set_device_mic_profile_name(&serial_number, device.mic_profile().name())
        .await;
    settings.save().await;
    Ok(device)
}

async fn check_firmware_versions(x: Sender<EnumMap<DeviceType, Option<VersionNumber>>>) {
    let full_key = "version";
    let mini_key = "miniVersion";

    let mut map: EnumMap<DeviceType, Option<VersionNumber>> = EnumMap::default();

    debug!("Performing Firmware Version Check..");
    let url = format!("{}{}", FIRMWARE_BASE, "UpdateManifest_v3.xml");
    if let Ok(response) = reqwest::get(url).await {
        if let Ok(text) = response.text().await {
            // Parse this into an XML tree...
            if let Ok(root) = Element::parse(text.as_bytes()) {
                if root.attributes.contains_key(mini_key) {
                    map[DeviceType::Mini] =
                        Some(VersionNumber::from(root.attributes[mini_key].clone()));
                }
                if root.attributes.contains_key(full_key) {
                    map[DeviceType::Full] =
                        Some(VersionNumber::from(root.attributes[full_key].clone()));
                }
            } else {
                warn!("Unable to Parse the XML Response from the TC-Helicon Update Server");
            }
        } else {
            warn!("Failed to Fetch a Response from the TC-Helicon Update Server");
        }
    } else {
        warn!("Unable to connect to the TC-Helicon Update Server");
    }

    let _ = x.send(map).await;
}
