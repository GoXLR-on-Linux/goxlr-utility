use anyhow::Result;
use client::runner;

#[tokio::main]
async fn main() -> Result<()> {
    runner::run_cli().await
}
