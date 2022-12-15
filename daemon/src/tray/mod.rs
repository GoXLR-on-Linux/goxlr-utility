use crate::events::EventTriggers;
use crate::DaemonState;
use anyhow::Result;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::mpsc;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod tray_icon;

pub fn handle_tray(
    blocking_shutdown: Arc<AtomicBool>,
    tx: mpsc::Sender<EventTriggers>,
    state: DaemonState,
) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        linux::handle_tray(blocking_shutdown, tx, state)
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        #[cfg(target_os = "macos")]
        {
            use cocoa::appkit::NSApp;

            // Before we spawn the tray, we need to initialise the app (this doesn't appear to
            // be done by tray-icon)
            unsafe {
                let _app = NSApp();
            }
        }
        tray_icon::handle_tray(blocking_shutdown, tx, state)
    }

    // For all other platforms, don't attempt to spawn a Tray Icon
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        // For now, don't spawn a tray icon.
        Ok(())
    }
}
