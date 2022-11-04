use actix_web::dev::ServerHandle;
use anyhow::{bail, Context, Result};
use clap::Parser;
use log::{error, info, warn};
use nix::unistd::Uid;
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode};
use tokio::sync::mpsc;
use tokio::{join, signal};

use crate::cli::{Cli, LevelFilter};
use crate::files::FileManager;
use crate::primary_worker::handle_changes;
use crate::servers::http_server::launch_httpd;
use crate::servers::ipc_server::{bind_socket, run_server};
use crate::settings::SettingsHandle;
use crate::shutdown::Shutdown;

mod audio;
mod cli;
mod device;
mod files;
mod mic_profile;
mod primary_worker;
mod profile;
mod servers;
mod settings;
mod shutdown;

// This can probably go somewhere else, but for now..
const DISTRIBUTABLE_ROOT: &str = "/usr/share/goxlr/";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<()> {
    let args: Cli = Cli::parse();

    CombinedLogger::init(vec![TermLogger::new(
        match args.log_level {
            LevelFilter::Off => log::LevelFilter::Off,
            LevelFilter::Error => log::LevelFilter::Error,
            LevelFilter::Warn => log::LevelFilter::Warn,
            LevelFilter::Info => log::LevelFilter::Info,
            LevelFilter::Debug => log::LevelFilter::Debug,
            LevelFilter::Trace => log::LevelFilter::Trace,
        },
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .context("Could not configure the logger")?;

    if Uid::effective().is_root() {
        if args.force_root {
            error!("GoXLR Utility running as root, this is generally considered bad.");
        } else {
            error!("The GoXLR Utility Daemon is not designed to be run as root, and should run");
            error!("as the current active user. If you're having problems with permissions,");
            error!("please consult the 'Permissions' section of the README. Running as root");
            error!("*WILL* cause issues with the sampler, and may pose a security threat.");
            error!("");
            error!("To override this message, please start with --force-root");
            std::process::exit(-1);
        }
    }

    info!("Starting GoXLR Daemon v{}", VERSION);
    let settings = SettingsHandle::load(args.config).await?;

    let mut shutdown = Shutdown::new();
    let file_manager = FileManager::new();
    let (usb_tx, usb_rx) = mpsc::channel(32);
    let usb_handle = tokio::spawn(handle_changes(
        usb_rx,
        shutdown.clone(),
        settings,
        file_manager,
    ));

    let ipc_socket = bind_socket().await;
    if ipc_socket.is_err() {
        error!("Error Starting Daemon: ");
        bail!("{}", ipc_socket.err().unwrap());
    }
    let ipc_socket = ipc_socket.unwrap();
    let communications_handle =
        tokio::spawn(run_server(ipc_socket, usb_tx.clone(), shutdown.clone()));

    let mut http_server: Option<ServerHandle> = None;
    if !args.http_disable {
        if args.http_enable_cors {
            warn!("HTTP Cross Origin Requests enabled, this may be a security risk.");
        }
        let (httpd_tx, httpd_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(launch_httpd(
            usb_tx.clone(),
            httpd_tx,
            args.http_port,
            args.http_enable_cors,
        ));
        http_server = Some(httpd_rx.await?);
    } else {
        warn!("HTTP Server Disabled");
    }

    await_ctrl_c(shutdown.clone()).await;

    info!("Shutting down daemon");
    if let Some(server) = http_server {
        // We only need to Join on the HTTP Server if it exists..
        let _ = join!(usb_handle, communications_handle, server.stop(true));
    } else {
        let _ = join!(usb_handle, communications_handle);
    }

    shutdown.recv().await;
    Ok(())
}

async fn await_ctrl_c(shutdown: Shutdown) {
    if signal::ctrl_c().await.is_ok() {
        shutdown.trigger();
    }
}
