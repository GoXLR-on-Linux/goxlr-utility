use crate::events::EventTriggers;
use crate::DaemonState;
use anyhow::{bail, Result};
use futures::executor::block_on;
use lazy_static::lazy_static;
use log::{debug, error};
use mslnk::ShellLink;
use std::path::PathBuf;
use std::{env, fs};
use sysinfo::{ProcessRefreshKind, RefreshKind, System, SystemExt};
use tokio::signal::windows::{ctrl_break, ctrl_close, ctrl_logoff, ctrl_shutdown};
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio::{select, time};
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;
use winrt_notification::{Sound, Toast};

const GOXLR_APP_NAME: &str = "GoXLR App.exe";
const AUTOSTART_FILENAME: &str = "GoXLR Utility.lnk";

lazy_static! {
    static ref STARTUP_PATH: Option<PathBuf> = get_startup_dir();
}

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

    let mut ctrl_break = ctrl_break()?;
    let mut ctrl_close = ctrl_close()?;
    let mut ctrl_shutdown = ctrl_shutdown()?;
    let mut ctrl_logoff = ctrl_logoff()?;

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
            Some(_) = ctrl_break.recv() => {
                block_on(tx.send(EventTriggers::Stop))?;
            },
            Some(_) = ctrl_close.recv() => {
                debug!("Hit Ctrl+Close");
                block_on(tx.send(EventTriggers::Stop))?;
            }
            Some(_) = ctrl_shutdown.recv() => {
                debug!("Hit Ctrl+Shutdown");
                block_on(tx.send(EventTriggers::Stop))?;
            }
            Some(_) = ctrl_logoff.recv() => {
                debug!("Hit Ctrl+Logoff");
                block_on(tx.send(EventTriggers::Stop))?;
            }
            //Some(_) = ctrl_
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

pub fn has_autostart() -> bool {
    if let Some(path) = &*STARTUP_PATH {
        return path.join(AUTOSTART_FILENAME).exists();
    }

    false
}

pub fn remove_startup_link() -> Result<()> {
    if let Some(path) = &*STARTUP_PATH {
        let file = path.join(AUTOSTART_FILENAME);
        if !file.exists() {
            debug!("Attempted to remove link on non-existent file");
            return Ok(());
        }

        // Remove the file.
        fs::remove_file(file)?;
    }
    Ok(())
}

pub fn create_startup_link() -> Result<()> {
    if let Some(path) = &*STARTUP_PATH {
        let file = path.join(AUTOSTART_FILENAME);
        if file.exists() {
            // File already exists, we're done?
            return Ok(());
        }

        // Get our executable filename..
        let executable = env::current_exe()?;

        // Remove any UNC Prefix from the executable path when safe..
        let executable = dunce::simplified(&executable);

        // Create the Symlink to our current path..
        let link = ShellLink::new(executable)?;
        link.create_lnk(file)?;
        return Ok(());
    }
    bail!("Error Finding Startup Path, unable to create link");
}

fn get_startup_dir() -> Option<PathBuf> {
    let reg_path = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\User Shell Folders";

    // Get %USERPROFILE% from the ENV..
    if let Ok(profile) = env::var("USERPROFILE") {
        let local_user = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(folders) = local_user.open_subkey(reg_path) {
            if let Ok(startup) = folders.get_value::<String, &str>("Startup") {
                let full_path = startup.replace("%USERPROFILE%", &profile);
                let path_buf = PathBuf::from(&full_path);

                if path_buf.exists() {
                    debug!("Setting Startup Path: {:?}", path_buf);
                    return Some(path_buf);
                }
            }
        }
    }
    None
}
