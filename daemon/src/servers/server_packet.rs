use crate::primary_worker::{DeviceCommand, DeviceSender};
use anyhow::{anyhow, Context, Result};
use goxlr_ipc::{DaemonRequest, DaemonResponse, HttpSettings};
use tokio::sync::oneshot;

pub async fn handle_packet(
    http_settings: &HttpSettings,
    request: DaemonRequest,
    usb_tx: &mut DeviceSender,
) -> Result<DaemonResponse> {
    match request {
        DaemonRequest::Ping => Ok(DaemonResponse::Ok),
        DaemonRequest::GetHttpState => Ok(DaemonResponse::HttpState(http_settings.clone())),
        DaemonRequest::OpenUi => {
            let (tx, rx) = oneshot::channel();
            usb_tx
                .send(DeviceCommand::OpenUi(tx))
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Cound not communicate with the device task")?;
            Ok(rx.await?)
        }
        DaemonRequest::RecoverDefaults(path_type) => {
            let (tx, rx) = oneshot::channel();
            usb_tx
                .send(DeviceCommand::RecoverDefaults(path_type, tx))
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Cound not communicate with the device task")?;
            Ok(rx.await?)
        }
        DaemonRequest::SetAutoStartEnabled(enabled) => {
            let (tx, rx) = oneshot::channel();
            usb_tx
                .send(DeviceCommand::SetAutoStartEnabled(enabled, tx))
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Cound not communicate with the device task")?;
            Ok(rx.await?)
        }
        DaemonRequest::StopDaemon => {
            let (tx, rx) = oneshot::channel();
            usb_tx
                .send(DeviceCommand::StopDaemon(tx))
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Cound not communicate with the device task")?;
            Ok(rx.await?)
        }
        DaemonRequest::SetShowTrayIcon(enabled) => {
            let (tx, rx) = oneshot::channel();
            usb_tx
                .send(DeviceCommand::SetShowTrayIcon(enabled, tx))
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Cound not communicate with the device task")?;
            Ok(rx.await?)
        }
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
        DaemonRequest::OpenPath(path_type) => {
            let (tx, _rx) = oneshot::channel();
            usb_tx
                .send(DeviceCommand::OpenPath(path_type, tx))
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Could not communicate with the device task")?;
            Ok(DaemonResponse::Ok)
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
