use crate::events::EventTriggers;
use crate::DaemonState;
use anyhow::Result;
use tokio::sync::mpsc;

pub fn perform_platform_preflight() -> Result<()> {
    Ok(())
}

pub async fn spawn_platform_runtime(
    _state: DaemonState,
    _tx: mpsc::Sender<EventTriggers>,
) -> Result<()> {
    Ok(())
}
