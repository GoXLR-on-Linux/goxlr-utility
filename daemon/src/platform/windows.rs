use crate::DaemonState;
use crate::events::EventTriggers;
use anyhow::{Result, bail};
use lazy_static::lazy_static;
use log::{debug, error};
use mslnk::ShellLink;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::{env, fs};
use sysinfo::{ProcessRefreshKind, RefreshKind, System};
use tokio::signal::windows::{ctrl_break, ctrl_close, ctrl_logoff, ctrl_shutdown};
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio::{select, time};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::{HSTRING, w};
use winreg::RegKey;
use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER};
use winrt_toast_reborn::content::audio::Sound;
use winrt_toast_reborn::{Audio, Toast, ToastDuration, ToastManager};

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
        bail!(
            "The official GoXLR Application is currently running, Please close it before running the Utility"
        );
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
    let process_refresh_kind = ProcessRefreshKind::everything().without_tasks();
    let refresh_kind = RefreshKind::nothing().with_processes(process_refresh_kind);
    let system = System::new_with_specifics(refresh_kind);

    let stable = OsStr::new(GOXLR_APP_NAME);
    let beta = OsStr::new(GOXLR_BETA_APP_NAME);
    system.processes_by_exact_name(stable).count() + system.processes_by_exact_name(beta).count()
}

fn get_utility_count() -> usize {
    if let Ok(exe) = env::current_exe()
        && let Some(file_name) = exe.file_name()
    {
        let process_refresh_kind = ProcessRefreshKind::everything().without_tasks();
        let refresh_kind = RefreshKind::nothing().with_processes(process_refresh_kind);
        let system = System::new_with_specifics(refresh_kind);

        return system.processes_by_exact_name(file_name).count();
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
    let manager = ToastManager::new(ToastManager::POWERSHELL_AUM_ID);

    let mut toast = Toast::new();
    toast.text1("GoXLR Utility Daemon Terminated");
    toast.text2("Please stop the official app before using the Utility");
    toast.audio(Audio::new(Sound::SMS));
    toast.duration(ToastDuration::Short);

    let _ = manager.show(&toast);
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
        if let Ok(folders) = local_user.open_subkey(reg_path)
            && let Ok(startup) = folders.get_value::<String, &str>("Startup")
        {
            let full_path = startup.replace("%USERPROFILE%", &profile);
            let path_buf = PathBuf::from(&full_path);

            if path_buf.exists() {
                debug!("Setting Startup Path: {:?}", path_buf);
                return Some(path_buf);
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
