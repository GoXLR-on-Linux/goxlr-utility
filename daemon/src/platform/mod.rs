use crate::events::EventTriggers;
use crate::DaemonState;
use anyhow::Result;
use cfg_if::cfg_if;
use log::debug;
use std::path::PathBuf;
use tokio::sync::mpsc;
use which::which;

cfg_if! {
    if #[cfg(windows)] {
        pub mod windows;

        pub fn perform_preflight() -> Result<()> {
            windows::perform_platform_preflight()
        }

        pub async fn spawn_runtime(state: DaemonState, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
            windows::spawn_platform_runtime(state, tx).await
        }

        pub fn has_autostart() -> bool {
            windows::has_autostart()
        }

        pub fn set_autostart(enabled: bool) -> Result<()> {
            if enabled {
                return windows::create_startup_link();
            }
            windows::remove_startup_link()
        }
    } else if #[cfg(target_os = "linux")] {
        mod linux;
        mod unix;


        pub fn perform_preflight() -> Result<()> {
            Ok(())
        }

        pub async fn spawn_runtime(state: DaemonState, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
            tokio::spawn(linux::sleep::run(tx.clone(), state.shutdown.clone()));
            unix::spawn_platform_runtime(state, tx).await
        }

        pub fn has_autostart() -> bool {
            linux::autostart::has_autostart()
        }

        pub fn set_autostart(enabled: bool) -> Result<()> {
            if enabled {
                return linux::autostart::create_startup_link();
            }
            linux::autostart::remove_startup_link()
        }
    } else if #[cfg(target_os = "macos")] {
        mod unix;
        use anyhow::bail;

        pub fn perform_preflight() -> Result<()> {
            Ok(())
        }

        pub async fn spawn_runtime(state: DaemonState, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
            unix::spawn_platform_runtime(state, tx).await
        }

        pub fn has_autostart() -> bool {
            false
        }

        pub fn set_autostart(_enabled: bool) -> Result<()> {
            bail!("Autostart Not Supported on this Platform");
        }
    } else {
        use anyhow::bail;

        pub fn perform_preflight() -> Result<()> {
            Ok(())
        }

        pub async fn spawn_runtime(_state: DaemonState, _tx: mpsc::Sender<EventTriggers>) -> Result<()> {
            Ok(())
        }

        pub fn has_autostart() -> bool {
            false
        }

        pub fn set_autostart(_enabled: bool) -> Result<()> {
            bail!("Autostart Not Supported on this Platform");
        }
    }
}

pub fn get_ui_app_path() -> Option<PathBuf> {
    debug!("Refreshing App Path..");

    // This simply looks for the GoXLR UI App alongside the daemon binary and returns it..
    let mut path = None;
    let bin_name = get_ui_binary_name();

    // There are three possible places to check for this, the CWD, the binary WD, and $PATH
    let cwd = std::env::current_dir().unwrap().join(bin_name.clone());
    if cwd.exists() {
        path.replace(cwd);
    }

    // IntelliJ complains about duplicate code here, and while yes, it's technically duplicated
    // from goxlr-launcher, the launcher and daemon don't have dependencies on each other.
    if path.is_none() {
        if let Some(parent) = std::env::current_exe().unwrap().parent() {
            let bin = parent.join(bin_name.clone());
            if bin.exists() {
                path.replace(bin);
            }
        }
    }

    if path.is_none() {
        // Try and locate the binary on $PATH
        if let Ok(which) = which(bin_name) {
            path.replace(which);
        }
    }

    path
}

static UI_NAME: &str = "goxlr-utility-ui";
fn get_ui_binary_name() -> String {
    if cfg!(windows) {
        format!("{UI_NAME}.exe")
    } else {
        String::from(UI_NAME)
    }
}
