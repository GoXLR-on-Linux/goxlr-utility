use crate::primary_worker::{DeviceCommand, DeviceSender};
use crate::Shutdown;
use anyhow::{anyhow, Context, Result};
use goxlr_ipc::Socket;
use goxlr_ipc::{DaemonRequest, DaemonResponse};
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

async fn handle_packet(
    request: DaemonRequest,
    usb_tx: &mut DeviceSender,
) -> Result<DaemonResponse> {
    match request {
        DaemonRequest::Ping => Ok(DaemonResponse::Ok),
        DaemonRequest::GetStatus => {
            let (tx, rx) = oneshot::channel();
            usb_tx
                .send(DeviceCommand::SendDaemonStatus(tx))
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Could not communicate with the device task")?;
            Ok(DaemonResponse::Status(rx.await.context(
                "Could not execute the command on the device task",
            )?))
        }
        DaemonRequest::Command(serial, command) => {
            let (tx, rx) = oneshot::channel();
            usb_tx
                .send(DeviceCommand::RunDeviceCommand(serial, command, tx))
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Could not communicate with the GoXLR device")?;
            rx.await
                .context("Could not execute the command on the GoXLR device")??;
            Ok(DaemonResponse::Ok)
        }
    }
}
