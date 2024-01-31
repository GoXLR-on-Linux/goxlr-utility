use anyhow::{bail, Result};
use interprocess::local_socket::tokio::{LocalSocketListener, LocalSocketStream};
use interprocess::local_socket::NameTypeSupport;
use log::{debug, info, warn};
use std::fs;
use std::path::Path;

use NameTypeSupport::*;

use goxlr_ipc::clients::ipc::ipc_socket::Socket;
use goxlr_ipc::{DaemonRequest, DaemonResponse};

use crate::primary_worker::DeviceSender;
use crate::servers::server_packet::handle_packet;
use crate::Shutdown;

static SOCKET_PATH: &str = "/tmp/goxlr.socket";
static NAMED_PIPE: &str = "@goxlr.socket";

async fn ipc_tidy() -> Result<()> {
    // We only need a possible cleanup if we're using file based sockets..
    let socket_type = NameTypeSupport::query();

    // Check to see if the socket exists,
    if (socket_type == OnlyPaths || socket_type == Both) && !Path::new(SOCKET_PATH).exists() {
        return Ok(());
    }

    let name = match socket_type {
        OnlyPaths | Both => {
            debug!("Unix Socket file present, performing connection test..");
            SOCKET_PATH
        }
        OnlyNamespaced => {
            debug!("Checking for Presence of Windows Named Pipe..");
            NAMED_PIPE
        }
    };

    // Try connecting to the socket, see if we're accepted..
    let connection = LocalSocketStream::connect(name).await;
    if connection.is_err() {
        match socket_type {
            OnlyPaths | Both => {
                debug!("Connection Failed. Socket File is stale, removing..");
                fs::remove_file(SOCKET_PATH)?;
            }
            OnlyNamespaced => {
                debug!("Named Pipe not running, continuing..");
            }
        }
        return Ok(());
    }

    debug!("Connected to socket, seeing if there's a Daemon on the other side..");
    let connection = connection.unwrap();
    let mut socket: Socket<DaemonResponse, DaemonRequest> = Socket::new(connection);
    if let Err(e) = socket.send(DaemonRequest::Ping).await {
        match socket_type {
            OnlyPaths | Both => {
                // In some cases, a connection may be able to occur, even if there's nothing
                // on the other end. So we'll simply nuke the socket.
                debug!("Unable to send messages, removing socket..");
                fs::remove_file(SOCKET_PATH)?;
            }
            OnlyNamespaced => {
                // On Windows however, we don't have the luxury of nuking the named pipe externally,
                // so if we can't send a message, it's GGs, we have to bail.
                debug!("Our named pipe is broken, something is horribly wrong..");
                bail!("Named Pipe Error: {}", e);
            }
        }
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
    usb_tx: DeviceSender,
    mut shutdown_signal: Shutdown,
) {
    debug!("Running IPC Server..");
    loop {
        tokio::select! {
            Ok(connection) = listener.accept() => {
                let socket = Socket::new(connection);
                let usb_tx = usb_tx.clone();
                tokio::spawn(async move {
                    handle_connection(socket, usb_tx).await;
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
        }
    }
}

async fn handle_connection(
    mut socket: Socket<DaemonRequest, DaemonResponse>,
    mut usb_tx: DeviceSender,
) {
    while let Some(msg) = socket.read().await {
        match msg {
            Ok(msg) => match handle_packet(msg, &mut usb_tx).await {
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
