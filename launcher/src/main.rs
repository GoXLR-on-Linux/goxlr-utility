use anyhow::{bail, Result};
use fork::{daemon, Fork};
use std::path::PathBuf;
use std::process::{exit, Command, Stdio};

use goxlr_ipc::client::Client;
use goxlr_ipc::ipc_socket::Socket;
use goxlr_ipc::{DaemonRequest, DaemonResponse};
use interprocess::local_socket::tokio::LocalSocketStream;
use interprocess::local_socket::NameTypeSupport;
use sysinfo::{ProcessRefreshKind, RefreshKind, System, SystemExt};
use which::which;

static SOCKET_PATH: &str = "/tmp/goxlr.socket";
static NAMED_PIPE: &str = "@goxlr.socket";
static DAEMON_NAME: &str = "goxlr-daemon";

#[tokio::main]
async fn main() -> Result<()> {
    // First thing to do, is check to see if the Daemon is running..
    if !is_daemon_running() {
        launch_daemon()?;
    }

    open_ui().await?;
    Ok(())
}

async fn get_connection() -> std::io::Result<LocalSocketStream> {
    LocalSocketStream::connect(match NameTypeSupport::query() {
        NameTypeSupport::OnlyPaths | NameTypeSupport::Both => SOCKET_PATH,
        NameTypeSupport::OnlyNamespaced => NAMED_PIPE,
    })
    .await
}

#[cfg(target_os = "linux")]
fn launch_daemon() -> Result<()> {
    if let Some(path) = locate_daemon_binary() {
        let mut command = Command::new(&path);
        command.arg("--start-ui");
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());

        if let Some(parent) = path.parent() {
            command.current_dir(parent);
        }

        match daemon(true, true) {
            Ok(Fork::Parent(_child)) => {
                exit(0);
            }
            Ok(Fork::Child) => {
                command.spawn().expect("Failed to Launch Child Process");
                exit(0);
            }
            Err(_) => {}
        }
    }
    bail!("Unable to Locate GoXLR Daemon Binary");
}

#[cfg(windows)]
fn launch_daemon() -> Result<()> {
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux")))]
fn launch_daemon() -> Result<()> {
    Ok(())
}

async fn open_ui() -> Result<()> {
    // We kinda have to hope for the best here..
    let mut usable_connection = None;

    if let Ok(connection) = get_connection().await {
        usable_connection.replace(connection);
    }

    if let Some(connection) = usable_connection {
        let socket: Socket<DaemonResponse, DaemonRequest> = Socket::new(connection);
        let mut client = Client::new(socket);
        client.send(DaemonRequest::OpenUi).await?;
    }
    Ok(())
}

fn is_daemon_running() -> bool {
    let refresh_kind = RefreshKind::new().with_processes(ProcessRefreshKind::new().with_user());
    let system = System::new_with_specifics(refresh_kind);

    let binding = get_daemon_binary_name();
    let processes = system.processes_by_exact_name(&binding);
    processes.count() > 0
}

fn locate_daemon_binary() -> Option<PathBuf> {
    let mut binary_path = None;
    let bin_name = get_daemon_binary_name();

    // There are three possible places to check for this, the CWD, the binary WD, and $PATH
    let cwd = std::env::current_dir().unwrap().join(bin_name.clone());
    if cwd.exists() {
        binary_path.replace(cwd);
    }

    if binary_path.is_none() {
        if let Some(parent) = std::env::current_exe().unwrap().parent() {
            let bin = parent.join(bin_name.clone());
            if bin.exists() {
                binary_path.replace(bin);
            }
        }
    }

    if binary_path.is_none() {
        // Try and locate the binary on $PATH
        if let Ok(path) = which(bin_name) {
            binary_path.replace(path);
        }
    }

    binary_path
}

fn get_daemon_binary_name() -> String {
    if cfg!(windows) {
        format!("{}.exe", DAEMON_NAME)
    } else {
        String::from(DAEMON_NAME)
    }
}
