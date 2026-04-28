#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Result, bail};
use std::path::PathBuf;
use std::time::Duration;

use goxlr_ipc::client::Client;
use goxlr_ipc::clients::ipc::ipc_client::IPCClient;
use goxlr_ipc::clients::ipc::ipc_socket::Socket;
use goxlr_ipc::{DaemonCommand, DaemonRequest, DaemonResponse, ipc_socket_path};
use interprocess::local_socket::tokio::prelude::LocalSocketStream;
use interprocess::local_socket::traits::tokio::Stream;
use interprocess::local_socket::{GenericFilePath, GenericNamespaced, ToFsName, ToNsName};
use tokio::time::{sleep, timeout};
use which::which;

static DAEMON_NAME: &str = "goxlr-daemon";
const UI_CONNECT_RETRY_ATTEMPTS: u8 = 20;
const UI_CONNECT_RETRY_DELAY_MS: u64 = 250;

#[tokio::main]
async fn main() -> Result<()> {
    // A process named goxlr-daemon is not enough: IPC sockets are per-user, so
    // verify that this user can actually talk to the daemon before skipping launch.
    if !is_daemon_running().await {
        launch_daemon()?;
    }

    open_ui().await?;
    Ok(())
}

async fn get_connection() -> Result<LocalSocketStream> {
    let socket_path = ipc_socket_path();
    let path = if cfg!(windows) {
        socket_path.as_str().to_ns_name::<GenericNamespaced>()
    } else {
        socket_path.as_str().to_fs_name::<GenericFilePath>()
    };

    let path = match path {
        Ok(path) => path,
        Err(e) => {
            bail!("Unable to Process Path {}", e);
        }
    };

    LocalSocketStream::connect(path)
        .await
        .map_err(anyhow::Error::msg)
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

async fn is_daemon_running() -> bool {
    let Ok(connection) = get_connection().await else {
        return false;
    };

    let socket: Socket<DaemonResponse, DaemonRequest> = Socket::new(connection);
    let mut client = IPCClient::new(socket);
    timeout(Duration::from_secs(1), client.send(DaemonRequest::Ping))
        .await
        .is_ok_and(|result| result.is_ok())
}

#[cfg(windows)]
fn launch_daemon() -> Result<()> {
    use std::process::{Command, Stdio, exit};

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
    let mut last_error = None;

    for attempt in 0..UI_CONNECT_RETRY_ATTEMPTS {
        match get_connection().await {
            Ok(connection) => {
                let socket: Socket<DaemonResponse, DaemonRequest> = Socket::new(connection);
                let mut client = IPCClient::new(socket);
                match client
                    .send(DaemonRequest::Daemon(DaemonCommand::Activate))
                    .await
                {
                    Ok(()) => return Ok(()),
                    Err(error) => last_error = Some(error.to_string()),
                }
            }
            Err(error) => {
                last_error = Some(error.to_string());
            }
        }

        // Don't delay after the final attempt.
        if attempt + 1 < UI_CONNECT_RETRY_ATTEMPTS {
            sleep(Duration::from_millis(UI_CONNECT_RETRY_DELAY_MS)).await;
        }
    }

    if let Some(error) = last_error {
        bail!(
            "Unable to make a connection with the Daemon after {} attempts: {}",
            UI_CONNECT_RETRY_ATTEMPTS,
            error
        );
    }

    bail!(
        "Unable to make a connection with the Daemon after {} attempts",
        UI_CONNECT_RETRY_ATTEMPTS
    );
}

fn locate_daemon_binary() -> Option<PathBuf> {
    let mut binary_path = None;
    let bin_name = get_daemon_binary_name();

    // There are three possible places to check for this, the CWD, the binary WD, and $PATH
    if let Ok(cwd) = std::env::current_dir() {
        let cwd = cwd.join(bin_name.clone());
        if cwd.exists() {
            binary_path.replace(cwd);
        }
    }

    if binary_path.is_none()
        && let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        let bin = parent.join(bin_name.clone());
        if bin.exists() {
            binary_path.replace(bin);
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
