use anyhow::{bail, Result};
use goxlr_ipc::clients::ipc::ipc_socket::Socket;
use goxlr_ipc::{DaemonRequest, DaemonResponse};
use interprocess::local_socket::tokio::prelude::{LocalSocketListener, LocalSocketStream};
use interprocess::local_socket::traits::tokio::{Listener, Stream};
use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, ListenerOptions, ToFsName, ToNsName,
};
use log::{debug, info, warn};
use std::fs;
use std::path::Path;

use crate::primary_worker::DeviceSender;
use crate::servers::server_packet::handle_packet;
use crate::Shutdown;

static SOCKET_PATH: &str = "/tmp/goxlr.socket";
static NAMED_PIPE: &str = "@goxlr.socket";

async fn ipc_tidy() -> Result<()> {
    // We only need a possible cleanup if we're using file based sockets, this has changed
    // substantially with the latest interprocess crate, so we're OS based now..
    let socket_type = if cfg!(windows) {
        NAMED_PIPE.to_ns_name::<GenericNamespaced>()?
    } else {
        if !Path::new(SOCKET_PATH).exists() {
            return Ok(());
        }
        SOCKET_PATH.to_fs_name::<GenericFilePath>()?
    };

    let connection = LocalSocketStream::connect(socket_type).await;
    if connection.is_err() {
        match cfg!(windows) {
            true => {
                debug!("Named Pipe not running, continuing..");
            }
            false => {
                debug!("Connection Failed. Socket File is stale, removing..");
                fs::remove_file(SOCKET_PATH)?;
            }
        }
        return Ok(());
    }

    debug!("Connected to socket, seeing if there's a Daemon on the other side..");
    let connection = connection.unwrap();

    let mut socket: Socket<DaemonResponse, DaemonRequest> = Socket::new(connection);
    if let Err(e) = socket.send(DaemonRequest::Ping).await {
        match cfg!(windows) {
            true => {
                debug!("Our named pipe is broken, something is horribly wrong..");
                bail!("Named Pipe Error: {}", e);
            }
            false => {
                debug!("Unable to send messages, removing socket..");
                fs::remove_file(SOCKET_PATH)?;
            }
        }
        return Ok(());
    }

    // If we get here, there's an active GoXLR Daemon running!
    bail!("The GoXLR Daemon is already running.");
}

pub async fn bind_socket() -> Result<LocalSocketListener> {
    ipc_tidy().await?;

    let name = if cfg!(windows) {
        NAMED_PIPE.to_ns_name::<GenericNamespaced>()?
    } else {
        SOCKET_PATH.to_fs_name::<GenericFilePath>()?
    };

    let opts = ListenerOptions::new().name(name.clone());
    let listener = opts.create_tokio()?;

    info!("Bound IPC Socket @ {:?}", name);
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
                if !cfg!(windows) {
                    let _ = fs::remove_file(SOCKET_PATH);
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
            Err(e) => {
                warn!("Invalid message from {:?}: {}", socket.address(), e);
                if let Err(e) = socket.send(DaemonResponse::Error(e.to_string())).await {
                    warn!("Could not reply to {:?}: {}", socket.address(), e);
                    return;
                }
            }
        }
    }
    debug!("Disconnected {:?}", socket.address());
}
