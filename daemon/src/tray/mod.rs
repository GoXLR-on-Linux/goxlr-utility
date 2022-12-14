use anyhow::Result;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

pub fn handle_tray(blocking_shutdown: Arc<AtomicBool>) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        ksni::handle_tray(blocking_shutdown)
    }

    #[cfg(target_os = "windows")]
    {
        windows::handle_tray(blocking_shutdown)
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        // For now, don't spawn a tray icon.
        Ok(())
    }
}
