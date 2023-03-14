use crate::client::Client;
use crate::clients::ipc::ipc_socket::Socket;
use crate::{DaemonRequest, DaemonResponse, DaemonStatus, GoXLRCommand, HttpSettings};
use anyhow::{anyhow, Context, Result};
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
            DaemonResponse::HttpState(state) => {
                self.http_settings = state;
                Ok(())
            }
            DaemonResponse::Status(status) => {
                self.status = status;
                Ok(())
            }
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error(error) => Err(anyhow!("{}", error)),
            DaemonResponse::Patch(_patch) => {
                Err(anyhow!("Received Patch as response, shouldn't happen!"))
            }
        }
    }

    async fn poll_status(&mut self) -> Result<()> {
        self.send(DaemonRequest::GetStatus).await
    }

    async fn poll_http_status(&mut self) -> Result<()> {
        self.send(DaemonRequest::GetHttpState).await
    }

    async fn command(&mut self, serial: &str, command: GoXLRCommand) -> Result<()> {
        self.send(DaemonRequest::Command(serial.to_string(), command))
            .await
    }

    fn status(&self) -> &DaemonStatus {
        &self.status
    }

    fn http_status(&self) -> &HttpSettings {
        &self.http_settings
    }
}
