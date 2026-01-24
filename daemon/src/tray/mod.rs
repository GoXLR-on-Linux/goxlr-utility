use crate::DaemonState;
use crate::events::EventTriggers;
use anyhow::Result;
use tokio::sync::mpsc;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
mod windows;

pub fn handle_tray(state: DaemonState, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::sync::atomic::Ordering;
        use tokio::task;
        if state.show_tray.load(Ordering::Relaxed) {
            // We'll just spawn the tray and return.
            task::spawn(linux::handle_tray(state.shutdown, tx));
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        macos::handle_tray(state, tx)
    }
    #[cfg(target_os = "windows")]
    {
        windows::handle_tray(state, tx)
    }

    // For all other platforms, don't attempt to spawn a Tray Icon
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        // For now, don't spawn a tray icon.
        Ok(())
    }
}
