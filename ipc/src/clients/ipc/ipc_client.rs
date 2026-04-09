use crate::client::Client;
use crate::clients::ipc::ipc_socket::Socket;
use crate::{DaemonRequest, DaemonResponse, DaemonStatus, GoXLRCommand, HttpSettings};
use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;

#[derive(Debug)]
pub struct IPCClient {
    socket: Socket<DaemonResponse, DaemonRequest>,
    status: DaemonStatus,
    http_settings: HttpSettings,
}

impl IPCClient {
    pub fn new(socket: Socket<DaemonResponse, DaemonRequest>) -> Self {
        Self {
            socket,
            status: DaemonStatus::default(),
            http_settings: Default::default(),
        }
    }
}

#[async_trait]
impl Client for IPCClient {
    async fn send(&mut self, request: DaemonRequest) -> Result<()> {
        self.socket
            .send(request)
            .await
            .context("Failed to send a command to the GoXLR daemon process")?;
        let result = self
            .socket
            .read()
            .await
            .context("Failed to retrieve the command result from the GoXLR daemon process")?
            .context("Failed to parse the command result from the GoXLR daemon process")?;

        match result {
            DaemonResponse::Status(status) => {
                self.status = status.clone();
                self.http_settings = status.config.http_settings;
                Ok(())
            }
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error(error) => Err(anyhow!("{}", error)),
            DaemonResponse::MicLevel(_level) => {
                bail!("Received Mic Level as Response, shouldn't happen!");
            }
            DaemonResponse::Patch(_patch) => {
                Err(anyhow!("Received Patch as response, shouldn't happen!"))
            }
        }
    }

    async fn poll_status(&mut self) -> Result<()> {
        self.send(DaemonRequest::GetStatus).await
    }

    async fn get_mic_level(&mut self, serial: &str) -> Result<f64> {
        self.socket
            .send(DaemonRequest::GetMicLevel(serial.to_string()))
            .await
            .context("Failed to send mic level request to the GoXLR daemon process")?;
        let result = self
            .socket
            .read()
            .await
            .context("Failed to retrieve mic level response from the GoXLR daemon process")?
            .context("Failed to parse mic level response from the GoXLR daemon process")?;

        match result {
            DaemonResponse::MicLevel(level) => Ok(level),
            DaemonResponse::Error(error) => Err(anyhow!("{}", error)),
            DaemonResponse::Status(status) => {
                self.status = status.clone();
                self.http_settings = status.config.http_settings;
                bail!("Received status while waiting for mic level response");
            }
            DaemonResponse::Ok => bail!("Received OK while waiting for mic level response"),
            DaemonResponse::Patch(_) => {
                bail!("Received patch while waiting for mic level response")
            }
        }
    }

    async fn command(&mut self, serial: &str, command: GoXLRCommand) -> Result<()> {
        self.send(DaemonRequest::Command(serial.to_string(), command))
            .await
    }

    async fn daemon_command(&mut self, command: DaemonRequest) -> Result<()> {
        self.send(command).await
    }

    fn status(&self) -> &DaemonStatus {
        &self.status
    }

    fn http_status(&self) -> &HttpSettings {
        &self.http_settings
    }
}
