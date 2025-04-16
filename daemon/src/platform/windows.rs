use crate::events::EventTriggers;
use crate::DaemonState;
use anyhow::{bail, Result};
use lazy_static::lazy_static;
use log::{debug, error};
use mslnk::ShellLink;
use std::path::PathBuf;
use std::{env, fs};
use tasklist::tasklist;
use tokio::signal::windows::{ctrl_break, ctrl_close, ctrl_logoff, ctrl_shutdown};
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio::{select, time};
use windows::core::{w, HSTRING};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER};
use winreg::RegKey;
use winrt_notification::{Sound, Toast};

const GOXLR_APP_NAME: &str = "GoXLR App.exe";
const GOXLR_BETA_APP_NAME: &str = "GoXLR Beta App.exe";
const AUTOSTART_FILENAME: &str = "GoXLR Utility.lnk";

lazy_static! {
    static ref STARTUP_PATH: Option<PathBuf> = get_startup_dir();
}

pub fn perform_platform_preflight() -> Result<()> {
    if !locate_goxlr_driver() {
        error!("Driver not found, Failing Preflight.");
        bail!("The GoXLR Driver was not found, please install it and try again.");
    }

    if get_official_app_count() > 0 {
        error!("Detected Official GoXLR Application Running, Failing Preflight.");
        bail!("The official GoXLR Application is currently running, Please close it before running the Utility");
    }

    if get_utility_count() > 1 {
        error!("Daemon Process already running, Failing Preflight");
        bail!("The GoXLR Utility is already running, please stop it and try again.");
    }

    Ok(())
}

pub fn display_error(message: String) {
    let message = HSTRING::from(message);

    unsafe {
        MessageBoxW(None, &message, w!("GoXLR Utility"), MB_OK | MB_ICONERROR);
    }
}

fn get_official_app_count() -> usize {
    unsafe {
        let tasks = tasklist();
        tasks
            .keys()
            .filter(|task| {
                let task = task.to_owned().to_owned();
                let task = String::from(task.split('\0').collect::<Vec<_>>()[0]);
                task == *GOXLR_APP_NAME || task == *GOXLR_BETA_APP_NAME
            })
            .count()
    }
}

fn get_utility_count() -> usize {
    if let Ok(exe) = env::current_exe() {
        if let Some(file_name) = exe.file_name() {
            unsafe {
                let tasks = tasklist();
                return tasks
                    .keys()
                    .filter(|task| {
                        let task = task.to_owned().to_owned();
                        let task = String::from(task.split('\0').collect::<Vec<_>>()[0]);
                        *task == *file_name
                    })
                    .count();
            }
        }
    }
    0
}

pub async fn spawn_platform_runtime(
    state: DaemonState,
    tx: mpsc::Sender<EventTriggers>,
) -> Result<()> {
    // Grab an async shutdown event..
    let mut shutdown = state.shutdown.clone();
    let mut duration = time::interval(Duration::from_millis(1000));

    let mut ctrl_break = ctrl_break()?;
    let mut ctrl_close = ctrl_close()?;
    let mut ctrl_shutdown = ctrl_shutdown()?;
    let mut ctrl_logoff = ctrl_logoff()?;

    loop {
        select! {
            _ = duration.tick() => {
                let count = get_official_app_count();
                if count > 0 {
                    throw_notification();
                    // We're calling 'DevicesStopped' here to force an end to the util, we can't use
                    // the regular Stop because it may attempt to load profiles, which isn't possible
                    // in a situation where the official app is running.
                    tx.send(EventTriggers::DevicesStopped).await?;
                    break;
                }
            },
            Some(_) = ctrl_break.recv() => {
                tx.send(EventTriggers::Stop(false)).await?;
            },
            Some(_) = ctrl_close.recv() => {
                debug!("Hit Ctrl+Close");
                tx.send(EventTriggers::Stop(false)).await?;
            }
            Some(_) = ctrl_shutdown.recv() => {
                debug!("Hit Ctrl+Shutdown");
                tx.send(EventTriggers::Stop(false)).await?;
            }
            Some(_) = ctrl_logoff.recv() => {
                debug!("Hit Ctrl+Logoff");
                tx.send(EventTriggers::Stop(false)).await?;
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

fn locate_goxlr_driver() -> bool {
    let regpath = "CLSID\\{024D0372-641F-4B7B-8140-F4DFE458C982}\\InprocServer32\\";
    let classes_root = RegKey::predef(HKEY_CLASSES_ROOT);
    if let Ok(folders) = classes_root.open_subkey(regpath) {
        // Name is blank because we need the default key
        if let Ok(api) = folders.get_value::<String, &str>("") {
            // Check the file exists..
            if PathBuf::from(&api).exists() {
                return true;
            }
        }
    }
    // If we get here, we didn't find it, return a default and hope for the best!
    let path = String::from(
        "C:/Program Files/TC-HELICON/GoXLR_Audio_Driver/W10_x64/goxlr_audioapi_x64.dll",
    );
    if PathBuf::from(&path).exists() {
        return true;
    }
    false
}
