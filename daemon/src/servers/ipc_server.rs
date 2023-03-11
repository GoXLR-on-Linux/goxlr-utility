use anyhow::{bail, Result};
use interprocess::local_socket::tokio::{LocalSocketListener, LocalSocketStream};
use interprocess::local_socket::NameTypeSupport;
use log::{debug, info, warn};
use std::fs;
use std::path::Path;

use NameTypeSupport::*;

use goxlr_ipc::clients::ipc::ipc_socket::Socket;
use goxlr_ipc::{DaemonRequest, DaemonResponse, HttpSettings};

use crate::primary_worker::DeviceSender;
use crate::servers::server_packet::handle_packet;
use crate::Shutdown;

static SOCKET_PATH: &str = "/tmp/goxlr.socket";
static NAMED_PIPE: &str = "@goxlr.socket";

async fn ipc_tidy() -> Result<()> {
    // We only need a possible cleanup if we're using file based sockets..
    let socket_type = NameTypeSupport::query();
    if socket_type == OnlyNamespaced {
        return Ok(());
    }

    // Check to see if the socket exists,
    if !Path::new(SOCKET_PATH).exists() {
        return Ok(());
    }

    debug!("Existing Socket Present, testing..");
    // Try sending a message to the socket, see if we get a reply..
    let connection = LocalSocketStream::connect(SOCKET_PATH).await;
    if connection.is_err() {
        debug!("Unable to connect to the socket, removing..");
        fs::remove_file(SOCKET_PATH)?;
        return Ok(());
    }

    debug!("Connected to socket, seeing if there's a Daemon on the other side..");
    let connection = connection.unwrap();
    let mut socket: Socket<DaemonResponse, DaemonRequest> = Socket::new(connection);
    if socket.send(DaemonRequest::Ping).await.is_err() {
        debug!("Socket Not Active, removing file..");
        fs::remove_file(SOCKET_PATH)?;
        return Ok(());
    }

    // If we get here, there's an active GoXLR Daemon running!
    bail!("The GoXLR Daemon is already running.");
}

pub async fn bind_socket() -> Result<LocalSocketListener> {
    ipc_tidy().await?;

    let name = {
        match NameTypeSupport::query() {
            OnlyPaths | Both => SOCKET_PATH,
            OnlyNamespaced => NAMED_PIPE,
        }
    };

    let listener = LocalSocketListener::bind(name)?;
    info!("Bound IPC Socket @ {}", name);
    Ok(listener)
}

pub async fn spawn_ipc_server(
    listener: LocalSocketListener,
    http_settings: HttpSettings,
    usb_tx: DeviceSender,
    mut shutdown_signal: Shutdown,
) {
    debug!("Running IPC Server..");
    loop {
        let http_settings = http_settings.clone();
        tokio::select! {
            Ok(connection) = listener.accept() => {
                let socket = Socket::new(connection);
                let usb_tx = usb_tx.clone();
                tokio::spawn(async move {
                    handle_connection(&http_settings.clone(), socket, usb_tx).await;
                });
            }
            () = shutdown_signal.recv() => {
                // If we're using a unix domain socket, remove it.
                match NameTypeSupport::query() {
                    OnlyPaths | Both => {
                        let _ = fs::remove_file(SOCKET_PATH);
                    },
                    OnlyNamespaced => {},
                }
                return;
            }
        };
    }
}

async fn handle_connection(
    http_settings: &HttpSettings,
    mut socket: Socket<DaemonRequest, DaemonResponse>,
    mut usb_tx: DeviceSender,
) {
    while let Some(msg) = socket.read().await {
        match msg {
            Ok(msg) => match handle_packet(http_settings, msg, &mut usb_tx).await {
                Ok(response) => {
                    if let Err(e) = socket.send(response).await {
                        warn!("Couldn't reply to {:?}: {}", socket.address(), e);
                        return;
                    }
                }
                Err(e) => {
                    if let Err(e) = socket.send(DaemonResponse::Error(e.to_string())).await {
                        warn!("Couldn't reply to {:?}: {}", socket.address(), e);
                        return;
                    }
                }
            },
            Err(e) => warn!("Invalid message from {:?}: {}", socket.address(), e),
        }
    }
    debug!("Disconnected {:?}", socket.address());
}
