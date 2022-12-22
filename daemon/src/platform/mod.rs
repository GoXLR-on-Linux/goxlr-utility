use crate::events::EventTriggers;
use crate::DaemonState;
use anyhow::Result;
use tokio::sync::mpsc;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(target_os = "windows"))]
mod default;

#[cfg(target_os = "windows")]
pub fn perform_preflight() -> Result<()> {
    windows::perform_platform_preflight()
}

#[cfg(target_os = "windows")]
pub async fn spawn_runtime(state: DaemonState, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
    windows::spawn_platform_runtime(state, tx).await
}

#[cfg(not(target_os = "windows"))]
pub fn perform_reflight() -> Result<()> {
    default::perform_platform_preflight()
}

#[cfg(not(target_os = "windows"))]
pub async fn perform_runtime(state: DaemonState, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
    default::spawn_platform_runtime(state, tx)
}
