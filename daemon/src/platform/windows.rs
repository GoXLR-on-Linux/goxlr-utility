use crate::events::EventTriggers;
use crate::DaemonState;
use anyhow::{bail, Result};
use futures::executor::block_on;
use log::{debug, error};
use sysinfo::{ProcessRefreshKind, RefreshKind, System, SystemExt};
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio::{select, time};
use winrt_notification::{Sound, Toast};

const GOXLR_APP_NAME: &str = "GoXLR App.exe";

pub fn perform_platform_preflight() -> Result<()> {
    let system = System::new_all();
    let processes = system.processes_by_exact_name(GOXLR_APP_NAME);
    if processes.count() > 0 {
        throw_notification();
        error!("Detected Official GoXLR Application Running, Failing Preflight.");
        bail!("Official GoXLR App Running, Please terminate it before running the Daemon");
    }

    Ok(())
}

pub async fn spawn_platform_runtime(
    state: DaemonState,
    tx: mpsc::Sender<EventTriggers>,
) -> Result<()> {
    // Grab an async shutdown event..
    let mut shutdown = state.shutdown.clone();
    let mut duration = time::interval(Duration::from_millis(1000));

    let refresh_kind = RefreshKind::new().with_processes(ProcessRefreshKind::new().with_user());
    let mut system = System::new_with_specifics(refresh_kind);
    loop {
        select! {
            _ = duration.tick() => {
                system.refresh_processes();
                let processes = system.processes_by_exact_name(GOXLR_APP_NAME);
                if processes.count() > 0 {
                    throw_notification();

                    // The processes list isn't Sendable, so this can't be triggered asynchronously.
                    block_on(tx.send(EventTriggers::Stop))?;
                    break;
                }
            },
            () = shutdown.recv() => {
                debug!("Shutting down Platform Runtime..");
                break;
            }
        };
    }

    Ok(())
}

fn throw_notification() {
    Toast::new(Toast::POWERSHELL_APP_ID)
        .title("GoXLR Utility Daemon Terminated")
        .text1("Please stop the official app before using the utility")
        .sound(Some(Sound::SMS))
        .duration(winrt_notification::Duration::Short)
        .show()
        .expect("Unable to Launch Toast");
}
