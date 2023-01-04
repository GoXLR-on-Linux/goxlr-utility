use crate::events::EventTriggers;
use crate::DaemonState;
use anyhow::Result;
use tokio::sync::mpsc;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod tao;

pub fn handle_tray(state: DaemonState, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        linux::handle_tray(state, tx)
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        tao::handle_tray(state, tx)
    }

    // For all other platforms, don't attempt to spawn a Tray Icon
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        // For now, don't spawn a tray icon.
        Ok(())
    }
}
