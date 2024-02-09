use crate::events::EventTriggers;
use crate::DaemonState;
use anyhow::Result;
use log::debug;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;

pub async fn spawn_platform_runtime(
    state: DaemonState,
    tx: mpsc::Sender<EventTriggers>,
) -> Result<()> {
    // This one's a little odd, because Windows doesn't directly support SIGTERM, we're going
    // to monitor for it here, and trigger a shutdown if one is received.
    let mut stream = signal(SignalKind::terminate())?;
    let mut shutdown = state.shutdown.clone();

    select! {
        Some(_) = stream.recv() => {
            // Trigger a Shutdown
            debug!("TERM Signal Received, Triggering STOP");
            let _ = tx.send(EventTriggers::Stop).await;
        },
        () = shutdown.recv() => {}
    }
    debug!("Platform Runtime Ended");
    Ok(())
}

// This is only used on Linux, but is available on MacOS
#[allow(unused)]
pub(crate) fn display_error(message: String) {
    use std::process::Command;
    // We have two choices here, kdialog, or zenity. We'll try both.
    if let Err(e) = Command::new("kdialog")
        .arg("--title")
        .arg("GoXLR Utility")
        .arg("--error")
        .arg(message.clone())
        .output()
    {
        println!("Error Running kdialog: {}, falling back to zenity..", e);
        let _ = Command::new("zenity")
            .arg("--title")
            .arg("GoXLR Utility")
            .arg("--error")
            .arg("--text")
            .arg(message)
            .output();
    }
}
