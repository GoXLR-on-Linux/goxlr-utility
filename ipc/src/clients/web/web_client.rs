use crate::client::Client;
use crate::{DaemonRequest, DaemonResponse, DaemonStatus, GoXLRCommand, HttpSettings};
use anyhow::{Context, Result, bail};
use async_trait::async_trait;

#[derive(Debug)]
pub struct WebClient {
    url: String,
    status: DaemonStatus,
    auth_token: Option<String>,
}

impl WebClient {
    pub fn new(url: String, auth_token: Option<String>) -> Self {
        Self {
            url,
            status: DaemonStatus::default(),
            auth_token,
        }
    }
}

#[async_trait]
impl Client for WebClient {
    async fn send(&mut self, request: DaemonRequest) -> anyhow::Result<()> {
        let client = reqwest::Client::new();
        let mut request = client.post(&self.url).json(&request);
        if let Some(token) = &self.auth_token {
            request = request.bearer_auth(token);
        }

        let resp = request
            .send()
            .await?
            .error_for_status()
            .context("HTTP command request failed")?
            .json::<DaemonResponse>()
            .await?;

        // Should probably abstract this part, it's common between clients..
        match resp {
            DaemonResponse::Status(status) => {
                self.status = status.clone();
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

    async fn get_mic_level(&mut self, serial: &str) -> anyhow::Result<f64> {
        let client = reqwest::Client::new();
        let mut request = client
            .post(&self.url)
            .json(&DaemonRequest::GetMicLevel(serial.to_string()));
        if let Some(token) = &self.auth_token {
            request = request.bearer_auth(token);
        }

        let resp = request
            .send()
            .await?
            .error_for_status()
            .context("HTTP mic-level request failed")?
            .json::<DaemonResponse>()
            .await?;

        match resp {
            DaemonResponse::MicLevel(level) => Ok(level),
            DaemonResponse::Error(error) => bail!("{}", error),
            DaemonResponse::Status(status) => {
                self.status = status.clone();
                bail!("Received status while waiting for mic level response")
            }
            DaemonResponse::Ok => bail!("Received OK while waiting for mic level response"),
            DaemonResponse::Patch(_) => {
                bail!("Received patch while waiting for mic level response")
            }
        }
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
        &self.status.config.http_settings
    }
}
