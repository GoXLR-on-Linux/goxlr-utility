use crate::{DaemonRequest, DaemonStatus, GoXLRCommand, HttpSettings};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Client {
    async fn send(&mut self, request: DaemonRequest) -> Result<()>;
    async fn poll_status(&mut self) -> Result<()>;
    async fn poll_http_status(&mut self) -> Result<()>;
    async fn command(&mut self, serial: &str, command: GoXLRCommand) -> Result<()>;
    fn status(&self) -> &DaemonStatus;
    fn http_status(&self) -> &HttpSettings;
}
