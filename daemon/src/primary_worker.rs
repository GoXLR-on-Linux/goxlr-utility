use crate::device::Device;
use crate::Shutdown;
use anyhow::Result;
use goxlr_ipc::{DeviceStatus, GoXLRCommand};
use goxlr_usb::goxlr::GoXLR;
use goxlr_usb::rusb::UsbContext;
use log::{error, info};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;

pub type DeviceSender = mpsc::Sender<(GoXLRCommand, oneshot::Sender<Result<Option<DeviceStatus>>>)>;
pub type DeviceReceiver =
    mpsc::Receiver<(GoXLRCommand, oneshot::Sender<Result<Option<DeviceStatus>>>)>;

const MIN_RECONNECT_SLEEP_DURATION: Duration = Duration::from_secs(1);
const MAX_RECONNECT_SLEEP_DURATION: Duration = Duration::from_secs(60 * 5);

pub async fn handle_changes(mut rx: DeviceReceiver, mut shutdown: Shutdown) {
    let mut warn_on_connect_error = true;
    let mut sleep_duration = MIN_RECONNECT_SLEEP_DURATION;

    loop {
        tokio::select! {
            () = sleep(sleep_duration) => {
                match GoXLR::open() {
                    Ok(goxlr) => {
                        let mut device = Device::new(goxlr);
                        match device.initialize() {
                            Ok(()) => {
                                warn_on_connect_error = true;
                                sleep_duration = MIN_RECONNECT_SLEEP_DURATION;
                                if let Err(e) = device_loop(&mut device, &mut rx, &mut shutdown).await {
                                    error!("Error whilst running device loop: {}", e);
                                }
                            },
                            Err(e) => {
                                error!("Error initializing device: {}", e);
                            },
                        }

                        info!("Disconnected from GoXLR");
                    }
                    Err(error) => {
                        if warn_on_connect_error {
                            error!("Couldn't connect to GoXLR: {}", error);
                        }
                        sleep_duration *= 2;
                        if sleep_duration > MAX_RECONNECT_SLEEP_DURATION {
                            sleep_duration = MAX_RECONNECT_SLEEP_DURATION;
                        }
                        warn_on_connect_error = false;
                    }
                }
            },
            () = shutdown.recv() => {
                info!("Shutting down device worker");
                return;
            },
        };
    }
}

async fn device_loop<C: UsbContext>(
    device: &mut Device<C>,
    rx: &mut DeviceReceiver,
    shutdown_signal: &mut Shutdown,
) -> Result<()> {
    let sleep_duration = Duration::from_secs(1);
    loop {
        tokio::select! {
            Some((command, response)) = rx.recv() => {
                let _ = response.send(device.perform_command(command));
            },
            () = shutdown_signal.recv() => {
                return Ok(());
            },
            () = sleep(sleep_duration) => {
                device.monitor_inputs()?;
            },
        };
    }
}
