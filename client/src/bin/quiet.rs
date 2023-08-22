#![windows_subsystem = "windows"]

/*
    This binary primarily exists so there's an option under windows to run commands without
    having a command prompt open, it runs the same code as the main client, but just does so
    silently.
*/

use anyhow::Result;
use client::runner;

#[tokio::main]
async fn main() -> Result<()> {
    runner::run_cli().await
}
