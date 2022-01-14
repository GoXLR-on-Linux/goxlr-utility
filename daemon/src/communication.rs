use crate::primary_worker::DeviceSender;
use crate::Shutdown;
use anyhow::{Context, Result};
use goxlr_ipc::{DaemonRequest, DaemonResponse};
use goxlr_ipc::{DeviceStatus, Socket};
use log::{debug, info, warn};
use tokio::net::UnixListener;
use tokio::sync::oneshot;

pub async fn listen_for_connections(
    listener: UnixListener,
    usb_tx: DeviceSender,
    mut shutdown_signal: Shutdown,
) {
    loop {
        tokio::select! {
            Ok((stream, addr)) = listener.accept() => {
                let usb_tx = usb_tx.clone();
                tokio::spawn(async move {
                    let socket = Socket::new(addr, stream);
                    handle_connection(socket, usb_tx).await
                });
            }
            () = shutdown_signal.recv() => {
                info!("Shutting down communications worker");
                return;
            }
        };
    }
}

async fn handle_connection(
    mut socket: Socket<DaemonRequest, DaemonResponse>,
    mut usb_tx: DeviceSender,
) {
    while let Some(msg) = socket.read().await {
        match msg {
            Ok(msg) => match handle_packet(msg, &mut usb_tx).await {
                Ok(device_status) => {
                    if let Err(e) = socket.send(DaemonResponse::Ok(device_status)).await {
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

async fn handle_packet(
    request: DaemonRequest,
    usb_tx: &mut DeviceSender,
) -> Result<Option<DeviceStatus>> {
    match request {
        DaemonRequest::Ping => Ok(None),
        DaemonRequest::Command(command) => {
            let (tx, rx) = oneshot::channel();
            usb_tx
                .send((command, tx))
                .await
                .context("Could not communicate with the GoXLR device")?;
            rx.await
                .context("Could not execute the command on the GoXLR device")?
        }
    }
}
