// This file primarily handles 'global' events which may occur inside the daemon from a potential
// variety of sources, which affect other parts of the daemon.

use crate::{SettingsHandle, Shutdown};
use goxlr_ipc::{HttpSettings, PathTypes};
use log::{debug, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::{select, signal};

#[derive(Debug)]
pub enum EventTriggers {
    Stop,
    Open(PathTypes),
    OpenUi,
    DevicesStopped,
}

#[derive(Clone)]
pub struct DaemonState {
    pub show_tray: Arc<AtomicBool>,
    pub http_settings: HttpSettings,

    // Shutdown Handlers
    pub shutdown: Shutdown,
    pub shutdown_blocking: Arc<AtomicBool>,

    // Settings Handle..
    pub settings_handle: SettingsHandle,
}

pub async fn spawn_event_handler(
    state: DaemonState,
    mut rx: Receiver<EventTriggers>,
    device_stop_tx: Sender<()>,
) {
    let mut triggered_device_stop = false;
    debug!("Starting Event Loop..");
    loop {
        select! {
            Ok(()) = signal::ctrl_c() => {
                debug!("Shutdown Phase 1 Triggered..");

                // Ctrl+C is a generic capture, although we should also check for SIGTERM under Linux..
                if !triggered_device_stop {
                    triggered_device_stop = true;
                    let _ = device_stop_tx.send(()).await;
                }
            },
            Some(event) = rx.recv() => {
                match event {
                    EventTriggers::Stop => {
                        debug!("Shutdown Phase 1 Triggered..");
                        if !triggered_device_stop {
                            triggered_device_stop = true;
                            let _ = device_stop_tx.send(()).await;
                        }
                    }
                    EventTriggers::DevicesStopped => {
                        debug!("Shutdown Phase 2 Triggered..");

                        // This hits after devices have been stopped..
                        state.shutdown.trigger();
                        state.shutdown_blocking.store(true, Ordering::Relaxed);
                        break;
                    }
                    EventTriggers::Open(path_type) => {
                        if let Err(error) = opener::open(match path_type {
                            PathTypes::Profiles => state.settings_handle.get_profile_directory().await,
                            PathTypes::MicProfiles => state.settings_handle.get_mic_profile_directory().await,
                            PathTypes::Presets => state.settings_handle.get_presets_directory().await,
                            PathTypes::Samples => state.settings_handle.get_samples_directory().await,
                            PathTypes::Icons => state.settings_handle.get_icons_directory().await,
                        }) {
                            warn!("Error Opening Path: {}", error);
                        };
                    },
                    EventTriggers::OpenUi => {
                        let mut host = String::from("localhost");
                        if &state.http_settings.bind_address != "localhost" && &state.http_settings.bind_address != "0.0.0.0" {
                            host = state.http_settings.bind_address.clone();
                        }

                        let url = format!("http://{}:{}/", host, state.http_settings.port);
                        if let Err(error) = opener::open(url) {
                            warn!("Error Opening URL: {}", error);
                        }
                    }
                }
            },
        }
    }
}
