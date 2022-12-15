// This file primarily handles 'global' events which may occur inside the daemon from a potential
// variety of sources, which affect other parts of the daemon.

use crate::{SettingsHandle, Shutdown};
use goxlr_ipc::{HttpSettings, PathTypes};
use log::{debug, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::{select, signal};

#[derive(Debug)]
pub enum EventTriggers {
    Stop,
    Open(PathTypes),
    OpenUi,
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

pub async fn spawn_event_handler(state: DaemonState, mut rx: Receiver<EventTriggers>) {
    debug!("Starting Event Loop..");
    loop {
        select! {
            Ok(()) = signal::ctrl_c() => {
                // Ctrl+C is a generic capture, although we should also check for SIGTERM under Linux..
                state.shutdown.trigger();
                state.shutdown_blocking.store(true, Ordering::Relaxed);
                break;
            },
            Some(event) = rx.recv() => {
                debug!("{:?}", event);

                match event {
                    EventTriggers::Stop => {
                        // This is essentially the same as Ctrl+C..
                        state.shutdown.trigger();
                        state.shutdown_blocking.store(true, Ordering::Relaxed);
                        break;
                    },
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
