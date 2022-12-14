use anyhow::Result;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[cfg(target_os = "linux")]
mod ksni;

#[cfg(not(target_os = "linux"))]
pub mod tray_icon;

pub fn handle_tray(blocking_shutdown: Arc<AtomicBool>) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        ksni::handle_tray(blocking_shutdown)
    }
    #[cfg(not(target_os = "linux"))]
    {
        tray_icon::handle_tray(blocking_shutdown)
    }
}
