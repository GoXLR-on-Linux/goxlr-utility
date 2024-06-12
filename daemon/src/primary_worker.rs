use crate::device::Device;
use crate::events::EventTriggers;
use crate::files::extract_defaults;
use crate::platform::{get_ui_app_path, has_autostart, set_autostart};
use crate::{FileManager, PatchEvent, SettingsHandle, Shutdown, SYSTEM_LOCALE, VERSION};
use anyhow::{anyhow, Result};
use enum_map::EnumMap;
use goxlr_ipc::{
    Activation, ColourWay, DaemonCommand, DaemonConfig, DaemonStatus, DriverDetails, Files,
    GoXLRCommand, HardwareStatus, HttpSettings, Locale, PathTypes, Paths, SampleFile,
    UsbProductInformation,
};
use goxlr_types::{DeviceType, VersionNumber};
use goxlr_usb::device::base::GoXLRDevice;
use goxlr_usb::device::{find_devices, from_device, get_version};
use goxlr_usb::{PID_GOXLR_FULL, PID_GOXLR_MINI};
use json_patch::diff;
use log::{debug, error, info, warn};
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::broadcast::Sender as BroadcastSender;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;
use xmltree::Element;

// Adding a third entry has tripped enum_variant_names, I'll probably need to rename
// RunDeviceCommand, but that'll need to be in a separate commit, for now, suppress.
#[allow(clippy::enum_variant_names)]
pub enum DeviceCommand {
    SendDaemonStatus(oneshot::Sender<DaemonStatus>),
    RunDaemonCommand(DaemonCommand, oneshot::Sender<Result<()>>),
    RunDeviceCommand(String, GoXLRCommand, oneshot::Sender<Result<()>>),
    GetDeviceMicLevel(String, oneshot::Sender<Result<f64>>),
}

#[allow(dead_code)]
pub enum DeviceStateChange {
    Shutdown,
    Sleep(oneshot::Sender<()>),
    Wake(oneshot::Sender<()>),
}

#[derive(Default)]
struct AppPathCheck {
    path: Option<String>,
    check: Option<SystemTime>,
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
    let mut app_check = AppPathCheck::default();
    let mut firmware_version = None;

    // We can probably either merge these, or struct them..
    let (disconnect_sender, mut disconnect_receiver) = mpsc::channel(16);
    let (event_sender, mut event_receiver) = mpsc::channel(16);
    let (firmware_sender, mut firmware_receiver) = mpsc::channel(1);

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

    // Get the Driver Type and Details..
    let (interface, version) = get_version();
    let driver_interface = DriverDetails { interface, version };

    // Create the Primary Device List, and 'Ignore' list..
    let mut devices: HashMap<String, Device> = HashMap::new();
    let mut ignore_list = HashMap::new();

    let mut files = get_files(&mut file_manager, &settings).await;
    let mut daemon_status = get_daemon_status(
        &devices,
        &settings,
        &http_settings,
        &driver_interface,
        &firmware_version,
        files.clone(),
        &mut app_check,
    )
    .await;

    let mut shutdown_triggered = false;

    loop {
        let mut change_found = false;
        tokio::select! {
            Some(version) = firmware_receiver.recv() => {
                firmware_version = Some(version);
                change_found = true;
            },
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
            Some(event) = device_state_rx.recv() => {
                match event {
                    DeviceStateChange::Shutdown => {
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
                                let _ = global_tx.send(EventTriggers::Stop).await;
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
                            let _ = sender.send(device.perform_command(command).await);
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
                files.clone(),
                &mut app_check,
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

async fn get_daemon_status(
    devices: &HashMap<String, Device<'_>>,
    settings: &SettingsHandle,
    http_settings: &HttpSettings,
    driver_details: &DriverDetails,
    firmware_versions: &Option<EnumMap<DeviceType, Option<VersionNumber>>>,
    files: Files,
    app_check: &mut AppPathCheck,
) -> DaemonStatus {
    // We need to limit this to every 30 seconds or so, simply because otherwise changing
    // any setting will start a probe on the drive, which is undesirable.
    if app_check.check.is_none() {
        get_app_path(app_check);
    }

    if let Some(check) = app_check.check {
        if let Ok(duration) = SystemTime::now().duration_since(check) {
            if duration.as_secs() > 30 {
                get_app_path(app_check);
            }
        }
    }

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
                app_path: app_check.path.clone(),
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

    for (serial, device) in devices {
        status
            .mixers
            .insert(serial.to_owned(), device.status().await.clone());
    }

    status
}

#[allow(const_item_mutation)]
fn get_app_path(app_check: &mut AppPathCheck) {
    debug!("Refreshing App Path..");
    if let Some(path) = get_ui_app_path() {
        // We need to escape the value..
        let wrap = if cfg!(windows) { "\"" } else { "'" };
        let path = format!("{}{}{}", wrap, path.to_string_lossy(), wrap);
        app_check.path.replace(path);
    } else {
        app_check.path.take();
    }
    app_check.check.replace(SystemTime::now());
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

async fn check_firmware_versions(x: Sender<EnumMap<DeviceType, Option<VersionNumber>>>) {
    let full_key = "version";
    let mini_key = "miniVersion";

    let mut map: EnumMap<DeviceType, Option<VersionNumber>> = EnumMap::default();

    debug!("Performing Firmware Version Check..");
    let url = "https://mediadl.musictribe.com/media/PLM/sftp/incoming/hybris/import/GOXLR/UpdateManifest_v3.xml";
    if let Ok(response) = reqwest::get(url).await {
        if let Ok(text) = response.text().await {
            // Parse this into an XML tree..
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
