//! Voicemeeter integration module.
//!
//! Links GoXLR faders / buttons with Voicemeeter strips, buses and macro buttons (both ways).
//! Each GoXLR profile gets a matching Voicemeeter profile in `<profiles>/Voicemeeter/<name>.json`,
//! which is saved / loaded / deleted alongside the GoXLR profile.
//!
//! The module is self contained, it hooks into the daemon in three places:
//!  - `main.rs`: `mod voicemeeter;`
//!  - `main.rs`: `let usb_tx = voicemeeter::spawn(...)` (wraps the device command channel)
//!  - `servers/http_server.rs`: `.configure(crate::voicemeeter::http::configure)`
//!
//! See `README.md` in this directory for details.

pub mod http;

mod config;
mod links;
mod remote;

use crate::PatchEvent;
use crate::primary_worker::{DeviceCommand, DeviceReceiver, DeviceSender};
use crate::settings::SettingsHandle;
use crate::shutdown::Shutdown;
use anyhow::{Result, anyhow};
use config::{VmProfile, load_profile, profile_path, save_profile};
use goxlr_ipc::{DaemonStatus, GoXLRCommand, MixerStatus};
use links::{LinkStates, read_button, read_volume};
use log::{debug, warn};
use remote::{RemoteStatus, VmEvent, VmInfo, VmRequest, spawn_remote};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;
use tokio::sync::broadcast::Sender as BroadcastSender;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{mpsc, oneshot};

const STATUS_REFRESH: Duration = Duration::from_millis(30);

static API: OnceLock<mpsc::Sender<ApiRequest>> = OnceLock::new();

/// Requests from the HTTP API
pub(crate) enum ApiRequest {
    GetState(oneshot::Sender<Value>),
    SetConfig(String, VmProfile, oneshot::Sender<Result<()>>),
    Save(String, oneshot::Sender<Result<()>>),
    Reload(String, oneshot::Sender<Result<()>>),
}

/// GoXLR profile operations spotted by the command proxy
enum ProfileEvent {
    /// Sent *before* the GoXLR saves, so the profile name change can't race us.
    SaveAs(String, String, oneshot::Sender<()>),
    New(String, String, oneshot::Sender<()>),

    /// Sent after a successful GoXLR load / delete
    Loaded(String, String),
    Deleted(String),
}

/// Starts the Voicemeeter module, returns the command channel the servers should use from now on.
pub fn spawn(
    usb_tx: DeviceSender,
    broadcast_tx: BroadcastSender<PatchEvent>,
    settings: SettingsHandle,
    shutdown: Shutdown,
) -> DeviceSender {
    let (proxy_tx, proxy_rx) = mpsc::channel(32);
    let (profile_tx, profile_rx) = mpsc::channel(8);
    let (api_tx, api_rx) = mpsc::channel(8);
    let _ = API.set(api_tx);

    tokio::spawn(run_proxy(proxy_rx, usb_tx.clone(), profile_tx));

    let (vm_tx, vm_rx) = mpsc::unbounded_channel();
    let worker = Worker {
        usb_tx,
        settings,
        remote: spawn_remote(vm_tx),
        remote_status: RemoteStatus::NotRunning,
        info: None,
        values: HashMap::new(),
        watched: vec![],
        status: None,
        devices: HashMap::new(),
    };
    tokio::spawn(worker.run(
        profile_rx,
        api_rx,
        vm_rx,
        broadcast_tx.subscribe(),
        shutdown,
    ));

    proxy_tx
}

pub(crate) async fn request<T>(build: impl FnOnce(oneshot::Sender<T>) -> ApiRequest) -> Result<T> {
    let api = API
        .get()
        .ok_or_else(|| anyhow!("Voicemeeter module not running"))?;
    let (tx, rx) = oneshot::channel();
    api.send(build(tx)).await?;
    Ok(rx.await?)
}

/// Sits between the IPC / HTTP servers and the device worker, passing everything through while
/// keeping an eye out for profile management commands.
async fn run_proxy(
    mut rx: DeviceReceiver,
    usb_tx: DeviceSender,
    profile_tx: mpsc::Sender<ProfileEvent>,
) {
    while let Some(command) = rx.recv().await {
        let DeviceCommand::RunDeviceCommand(serial, command, sender) = command else {
            let _ = usb_tx.send(command).await;
            continue;
        };

        match &command {
            GoXLRCommand::SaveProfile() => {
                let (tx, rx) = oneshot::channel();
                let _ = profile_tx
                    .send(ProfileEvent::SaveAs(serial.clone(), String::new(), tx))
                    .await;
                let _ = rx.await;
            }
            GoXLRCommand::SaveProfileAs(name) => {
                let (tx, rx) = oneshot::channel();
                let event = ProfileEvent::SaveAs(serial.clone(), name.clone(), tx);
                let _ = profile_tx.send(event).await;
                let _ = rx.await;
            }
            GoXLRCommand::NewProfile(name) => {
                let (tx, rx) = oneshot::channel();
                let event = ProfileEvent::New(serial.clone(), name.clone(), tx);
                let _ = profile_tx.send(event).await;
                let _ = rx.await;
            }
            GoXLRCommand::LoadProfile(name, _) | GoXLRCommand::DeleteProfile(name) => {
                let event = match &command {
                    GoXLRCommand::LoadProfile(..) => {
                        ProfileEvent::Loaded(serial.clone(), name.clone())
                    }
                    _ => ProfileEvent::Deleted(name.clone()),
                };

                // Wait for the result without holding up other commands
                let (tx, rx) = oneshot::channel();
                let forward = DeviceCommand::RunDeviceCommand(serial, command, tx);
                let _ = usb_tx.send(forward).await;
                let profile_tx = profile_tx.clone();
                tokio::spawn(async move {
                    let result = rx.await.unwrap_or_else(|e| Err(anyhow!(e)));
                    if result.is_ok() {
                        let _ = profile_tx.send(event).await;
                    }
                    let _ = sender.send(result);
                });
                continue;
            }
            _ => {}
        }

        let _ = usb_tx
            .send(DeviceCommand::RunDeviceCommand(serial, command, sender))
            .await;
    }
}

struct DeviceLinks {
    profile_name: String,
    config: VmProfile,
    dirty: bool,
    states: LinkStates,
}

struct Worker {
    usb_tx: DeviceSender,
    settings: SettingsHandle,
    remote: std_mpsc::Sender<VmRequest>,

    remote_status: RemoteStatus,
    info: Option<VmInfo>,
    values: HashMap<String, f32>,
    watched: Vec<String>,

    status: Option<DaemonStatus>,
    devices: HashMap<String, DeviceLinks>,
}

impl Worker {
    async fn run(
        mut self,
        mut profile_rx: mpsc::Receiver<ProfileEvent>,
        mut api_rx: mpsc::Receiver<ApiRequest>,
        mut vm_rx: mpsc::UnboundedReceiver<VmEvent>,
        mut patch_rx: tokio::sync::broadcast::Receiver<PatchEvent>,
        mut shutdown: Shutdown,
    ) {
        let mut refresh = tokio::time::interval(STATUS_REFRESH);
        let mut stale = true;

        loop {
            tokio::select! {
                // Profile events must be handled before any status update that follows them.
                biased;

                Some(event) = profile_rx.recv() => self.handle_profile_event(event).await,
                Some(request) = api_rx.recv() => self.handle_api(request).await,
                Some(event) = vm_rx.recv() => {
                    self.handle_vm_event(event);
                    self.sync();
                }
                result = patch_rx.recv() => match result {
                    Ok(_) | Err(RecvError::Lagged(_)) => stale = true,
                    Err(RecvError::Closed) => break,
                },
                _ = refresh.tick() => {
                    if stale {
                        stale = false;
                        self.refresh_status().await;
                        self.sync();
                    }
                }
                () = shutdown.recv() => break,
            }
        }

        debug!("Stopping Voicemeeter module");
        let _ = self.remote.send(VmRequest::Shutdown);
    }

    async fn refresh_status(&mut self) {
        let (tx, rx) = oneshot::channel();
        if self
            .usb_tx
            .send(DeviceCommand::SendDaemonStatus(tx))
            .await
            .is_err()
        {
            return;
        }
        let Ok(status) = rx.await else { return };

        // Track device arrivals, removals and profile changes.
        self.devices
            .retain(|serial, _| status.mixers.contains_key(serial));
        for (serial, mixer) in &status.mixers {
            let changed = self
                .devices
                .get(serial)
                .is_none_or(|d| d.profile_name != mixer.profile_name);
            if changed {
                self.load_device(serial, &mixer.profile_name).await;
            }
        }

        self.status = Some(status);
    }

    async fn load_device(&mut self, serial: &str, profile_name: &str) {
        let dir = self.settings.get_profile_directory().await;
        let config = load_profile(&dir, profile_name);

        // Every GoXLR profile should have a Voicemeeter counterpart on disk.
        if !profile_path(&dir, profile_name).exists()
            && let Err(e) = save_profile(&dir, profile_name, &config)
        {
            warn!("{}", e);
        }

        debug!("[{}] Loaded Voicemeeter profile '{}'", serial, profile_name);
        let states = LinkStates::new(&config);
        self.devices.insert(
            serial.to_string(),
            DeviceLinks {
                profile_name: profile_name.to_string(),
                config,
                dirty: false,
                states,
            },
        );
        self.update_watched();
    }

    async fn save_device(&mut self, serial: &str) -> Result<()> {
        let dir = self.settings.get_profile_directory().await;
        let device = self
            .devices
            .get_mut(serial)
            .ok_or_else(|| anyhow!("Device {} not found", serial))?;
        save_profile(&dir, &device.profile_name, &device.config)?;
        device.dirty = false;
        Ok(())
    }

    async fn handle_profile_event(&mut self, event: ProfileEvent) {
        match event {
            ProfileEvent::SaveAs(serial, name, done) => {
                if let Some(device) = self.devices.get_mut(&serial)
                    && !name.is_empty()
                {
                    device.profile_name = name;
                }
                if let Err(e) = self.save_device(&serial).await {
                    warn!("Unable to save Voicemeeter profile: {}", e);
                }
                let _ = done.send(());
            }
            ProfileEvent::New(serial, name, done) => {
                // If the GoXLR refuses the new profile, the next status refresh will spot the
                // name mismatch and reload the real one.
                let dir = self.settings.get_profile_directory().await;
                let config = VmProfile::default();
                if !profile_path(&dir, &name).exists()
                    && let Err(e) = save_profile(&dir, &name, &config)
                {
                    warn!("{}", e);
                }
                self.devices.insert(
                    serial,
                    DeviceLinks {
                        profile_name: name,
                        states: LinkStates::new(&config),
                        config,
                        dirty: false,
                    },
                );
                self.update_watched();
                let _ = done.send(());
            }
            ProfileEvent::Loaded(serial, name) => {
                // Also covers re-loading the current profile, which discards unsaved changes.
                self.load_device(&serial, &name).await;
            }
            ProfileEvent::Deleted(name) => {
                let dir = self.settings.get_profile_directory().await;
                if let Err(e) = config::delete_profile(&dir, &name) {
                    warn!("{}", e);
                }
            }
        }
    }

    async fn handle_api(&mut self, request: ApiRequest) {
        match request {
            ApiRequest::GetState(sender) => {
                let _ = sender.send(self.state_json().await);
            }
            ApiRequest::SetConfig(serial, config, sender) => {
                let result = match self.devices.get_mut(&serial) {
                    Some(device) => {
                        if device.config != config {
                            device.states = if device.config.enabled != config.enabled {
                                LinkStates::new(&config)
                            } else {
                                device.states.rebuild(&device.config, &config)
                            };
                            device.config = config;
                            device.dirty = true;
                        }
                        Ok(())
                    }
                    None => Err(anyhow!("Device {} not found", serial)),
                };
                self.update_watched();
                self.sync();
                let _ = sender.send(result);
            }
            ApiRequest::Save(serial, sender) => {
                let _ = sender.send(self.save_device(&serial).await);
            }
            ApiRequest::Reload(serial, sender) => {
                let result = match self.devices.get(&serial) {
                    Some(device) => {
                        let name = device.profile_name.clone();
                        self.load_device(&serial, &name).await;
                        self.sync();
                        Ok(())
                    }
                    None => Err(anyhow!("Device {} not found", serial)),
                };
                let _ = sender.send(result);
            }
        }
    }

    fn handle_vm_event(&mut self, event: VmEvent) {
        match event {
            VmEvent::Status(status, info) => {
                if status != self.remote_status {
                    // Fresh connection, let every link re-sync from scratch
                    for device in self.devices.values_mut() {
                        device.states = LinkStates::new(&device.config);
                    }
                }
                if status != RemoteStatus::Connected {
                    self.values.clear();
                }
                self.remote_status = status;
                if info.is_some() {
                    self.info = info;
                }
            }
            VmEvent::Values(values) => self.values = values,
        }
    }

    fn update_watched(&mut self) {
        let mut watched: Vec<String> = self
            .devices
            .values()
            .flat_map(|d| links::targets(&d.config))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        watched.sort();
        watched.dedup();

        if watched != self.watched {
            self.watched = watched.clone();
            let _ = self.remote.send(VmRequest::Watch(watched));
        }
    }

    fn sync(&mut self) {
        if self.remote_status != RemoteStatus::Connected {
            return;
        }
        let Some(status) = &self.status else { return };

        for (serial, device) in self.devices.iter_mut() {
            if !device.config.enabled {
                continue;
            }
            let Some(mixer) = status.mixers.get(serial) else {
                continue;
            };

            let actions =
                links::evaluate(&device.config, &mut device.states, mixer, &mut self.values);
            for (param, value) in actions.vm {
                debug!("Voicemeeter: {} = {}", param, value);
                let _ = self.remote.send(VmRequest::Set(param, value));
            }
            for command in actions.goxlr {
                debug!("[{}] From Voicemeeter: {:?}", serial, command);
                let usb_tx = self.usb_tx.clone();
                let serial = serial.clone();
                tokio::spawn(async move {
                    let (tx, rx) = oneshot::channel();
                    let command = DeviceCommand::RunDeviceCommand(serial, command, tx);
                    if usb_tx.send(command).await.is_ok()
                        && let Ok(Err(e)) = rx.await
                    {
                        warn!("Voicemeeter: GoXLR command failed: {}", e);
                    }
                });
            }
        }
    }

    async fn state_json(&self) -> Value {
        let dir = self.settings.get_profile_directory().await;
        let mixers = self.status.as_ref().map(|s| &s.mixers);

        let devices: serde_json::Map<String, Value> = self
            .devices
            .iter()
            .map(|(serial, device)| {
                let mixer = mixers.and_then(|m| m.get(serial));
                (serial.clone(), self.device_json(device, mixer, &dir))
            })
            .collect();

        json!({
            "remote": self.remote_status,
            "info": self.info,
            "devices": devices,
        })
    }

    fn device_json(
        &self,
        device: &DeviceLinks,
        mixer: Option<&MixerStatus>,
        dir: &std::path::Path,
    ) -> Value {
        let vm = |target: &String| self.values.get(target.trim()).copied();

        // Live values, so the UI can show what each link is currently doing
        let faders: Vec<Value> = device
            .config
            .faders
            .iter()
            .map(|link| {
                let goxlr = mixer.and_then(|m| read_volume(link.source, m));
                json!({ "goxlr": goxlr, "vm": vm(&link.target) })
            })
            .collect();
        let buttons: Vec<Value> = device
            .config
            .buttons
            .iter()
            .map(|link| {
                let goxlr = mixer.and_then(|m| read_button(link.source, m));
                json!({ "goxlr": goxlr, "vm": vm(&link.target) })
            })
            .collect();

        json!({
            "profile_name": device.profile_name,
            "file": profile_path(dir, &device.profile_name),
            "dirty": device.dirty,
            "config": device.config,
            "device_type": mixer.map(|m| m.hardware.device_type),
            "fader_channels": mixer.map(|m| {
                m.fader_status.iter().map(|(fader, status)| (fader.to_string(), json!(status.channel))).collect::<serde_json::Map<_, _>>()
            }),
            "live": { "faders": faders, "buttons": buttons },
        })
    }
}
