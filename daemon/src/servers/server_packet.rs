use crate::primary_worker::{DeviceCommand, DeviceSender};
use anyhow::{anyhow, Context, Result};
use goxlr_ipc::{DaemonRequest, DaemonResponse};
use std::path::PathBuf;
use tokio::sync::oneshot;

pub async fn handle_packet(
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
        DaemonRequest::Daemon(command) => {
            let (tx, rx) = oneshot::channel();
            usb_tx
                .send(DeviceCommand::RunDaemonCommand(command, tx))
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Could not communicate with the GoXLR device")?;
            rx.await
                .context("Could not execute the command on the GoXLR device")??;
            Ok(DaemonResponse::Ok)
        }
        DaemonRequest::GetMicLevel(serial) => {
            let (tx, rx) = oneshot::channel();
            usb_tx
                .send(DeviceCommand::GetDeviceMicLevel(serial, tx))
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .map_err(|e| anyhow!(e.to_string()))
                .context("Could not communicate with the GoXLR device")?;
            let result = rx
                .await
                .context("Could not execute the command on the GoXLR device")?;

            match result {
                Ok(value) => Ok(DaemonResponse::MicLevel(value)),
                Err(e) => Ok(DaemonResponse::Error(e.to_string())),
            }
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

        DaemonRequest::RunFirmwareUpdate(serial, path, force) => {
            let (tx, rx) = oneshot::channel();
            usb_tx
                .send(DeviceCommand::RunFirmwareUpdate(serial, path, force, tx))
                .await
                .map_err(anyhow::Error::msg)?;
            rx.await
                .context("Could not execute the command on the GoXLR device")??;
            Ok(DaemonResponse::Ok)
        }

        DaemonRequest::ContinueFirmwareUpdate(serial) => {
            let (tx, rx) = oneshot::channel();
            usb_tx
                .send(DeviceCommand::ContinueFirmwareUpdate(serial, tx))
                .await
                .map_err(|e| anyhow!(e.to_string()))
                .context("Could not communicate with the GoXLR device")?;
            rx.await
                .context("Could not execute the command on the GoXLR device")??;
            Ok(DaemonResponse::Ok)
        }

        DaemonRequest::ClearFirmwareState(serial) => {
            let (tx, rx) = oneshot::channel();
            usb_tx
                .send(DeviceCommand::ClearFirmwareState(serial, tx))
                .await
                .map_err(anyhow::Error::msg)?;
            rx.await
                .context("Could not execute the command on the GoXLR device")??;
            Ok(DaemonResponse::Ok)
        }
    }
}
