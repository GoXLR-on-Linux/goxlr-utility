use crate::events::EventTriggers;
use crate::DaemonState;
use anyhow::Result;
use tokio::sync::mpsc;

mod tao;

pub fn handle_tray(state: DaemonState, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
    tao::handle_tray(state, tx)
}
