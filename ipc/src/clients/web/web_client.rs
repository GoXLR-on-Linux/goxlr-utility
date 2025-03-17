use crate::client::Client;
use crate::{DaemonRequest, DaemonResponse, DaemonStatus, GoXLRCommand, HttpSettings};
use anyhow::{bail, Result};
use async_trait::async_trait;

#[derive(Debug)]
pub struct WebClient {
    url: String,
    status: DaemonStatus,
    http_settings: HttpSettings,
}

impl WebClient {
    pub fn new(url: String) -> Self {
        Self {
            url,
            status: DaemonStatus::default(),
            http_settings: Default::default(),
        }
    }
}

#[async_trait]
impl Client for WebClient {
    async fn send(&mut self, request: DaemonRequest) -> anyhow::Result<()> {
        let resp = reqwest::Client::new()
            .post(&self.url)
            .json(&request)
            .send()
            .await?
            .json::<DaemonResponse>()
            .await?;

        // Should probably abstract this part, it's common between clients..
        match resp {
            DaemonResponse::Status(status) => {
                self.status = status.clone();
                self.http_settings = status.config.http_settings;
                Ok(())
            }
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error(error) => bail!("{}", error),
            DaemonResponse::MicLevel(_level) => {
                bail!("Received Mic Level as response, shouldn't happen!")
            }
            DaemonResponse::Patch(_patch) => {
                bail!("Received Patch as response, shouldn't happen!")
            }
        }
    }

    async fn poll_status(&mut self) -> anyhow::Result<()> {
        self.send(DaemonRequest::GetStatus).await
    }

    async fn command(&mut self, serial: &str, command: GoXLRCommand) -> anyhow::Result<()> {
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
