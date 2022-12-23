use crate::events::EventTriggers;
use crate::DaemonState;
use anyhow::Result;
use cfg_if::cfg_if;
use tokio::sync::mpsc;

cfg_if! {
    if #[cfg(windows)] {
        mod windows;
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
            unix::spawn_platform_runtime(state, tx).await
        }

        pub fn has_autostart() -> bool {
            linux::has_autostart()
        }

        pub fn set_autostart(enabled: bool) -> Result<()> {
            if enabled {
                return linux::create_startup_link();
            }
            linux::remove_startup_link()
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
