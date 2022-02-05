use anyhow::{anyhow, Context, Result};
use goxlr_ipc::{DaemonRequest, DaemonResponse, DaemonStatus, GoXLRCommand, Socket};

#[derive(Debug)]
pub struct Client {
    socket: Socket<DaemonResponse, DaemonRequest>,
    status: DaemonStatus,
}

impl Client {
    pub fn new(socket: Socket<DaemonResponse, DaemonRequest>) -> Self {
        Self {
            socket,
            status: DaemonStatus::default(),
        }
    }

    pub async fn send(&mut self, request: DaemonRequest) -> Result<()> {
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
                self.status = status;
                Ok(())
            }
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error(error) => Err(anyhow!("{}", error)),
        }
    }

    pub async fn poll_status(&mut self) -> Result<()> {
        self.send(DaemonRequest::GetStatus).await
    }

    pub async fn command(&mut self, serial: &str, command: GoXLRCommand) -> Result<()> {
        self.send(DaemonRequest::Command(serial.to_string(), command))
            .await
    }

    pub fn status(&self) -> &DaemonStatus {
        &self.status
    }
}
