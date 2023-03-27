#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{bail, Result};
use std::path::PathBuf;

use goxlr_ipc::client::Client;
use goxlr_ipc::clients::ipc::ipc_client::IPCClient;
use goxlr_ipc::clients::ipc::ipc_socket::Socket;
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

#[cfg(unix)]
fn launch_daemon() -> Result<()> {
    use nix::unistd::execve;
    use std::env;
    use std::ffi::CString;

    if let Some(path) = locate_daemon_binary() {
        // Use execve to replace this process with the daemon..
        let c_path = CString::new(path.to_string_lossy().as_bytes())?;
        let c_daemon = CString::new(get_daemon_binary_name())?;
        let c_start_ui = CString::new("--start-ui")?;

        // TO-CONSIDER: Pass all env::args() through to the daemon?
        let c_params = vec![c_daemon, c_start_ui];

        // Copy all environment variables for this into the new process..
        let mut c_env = vec![];
        for (key, value) in env::vars() {
            c_env.push(CString::new(format!("{key}={value}"))?);
        }

        execve::<CString, CString>(&c_path, c_params.as_slice(), c_env.as_slice())?;
    }
    bail!("Unable to Locate GoXLR Daemon Binary");
}

#[cfg(windows)]
fn launch_daemon() -> Result<()> {
    use std::process::{exit, Command, Stdio};

    // Ok, try a simple spawn and exit..
    if let Some(path) = locate_daemon_binary() {
        let mut command = Command::new(&path);
        command.arg("--start-ui");
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());

        if let Some(parent) = path.parent() {
            command.current_dir(parent);
        }

        command.spawn().expect("Unable to Launch Child Process");
        exit(0);
    }

    bail!("Unable to Locate GoXLR Daemon Binary");
}

async fn open_ui() -> Result<()> {
    // We kinda have to hope for the best here..
    let mut usable_connection = None;

    if let Ok(connection) = get_connection().await {
        usable_connection.replace(connection);
    }

    if let Some(connection) = usable_connection {
        let socket: Socket<DaemonResponse, DaemonRequest> = Socket::new(connection);
        let mut client = IPCClient::new(socket);
        client.send(DaemonRequest::OpenUi).await?;
        return Ok(());
    }
    bail!("Unable to make a connection with the Daemon");
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
        format!("{DAEMON_NAME}.exe")
    } else {
        String::from(DAEMON_NAME)
    }
}
