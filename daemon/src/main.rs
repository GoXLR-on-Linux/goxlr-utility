#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use actix_web::dev::ServerHandle;
use anyhow::{bail, Context, Result};
use clap::Parser;
use goxlr_ipc::HttpSettings;
use json_patch::Patch;
use log::{debug, error, info, warn};
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::{join, signal};
use tray_icon::menu::{menu_event_receiver, Menu, MenuItem};
use tray_icon::{tray_event_receiver, TrayIconBuilder};
use winit::event_loop::EventLoopBuilder;
use winit::platform::run_return::EventLoopExtRunReturn;

use crate::cli::{Cli, LevelFilter};
use crate::files::{get_file_paths_from_settings, run_notification_service, FileManager};
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

const ICON: &[u8] = include_bytes!("../resources/goxlr-icon-dark.png");

// This is for global 'JSON Patches', for when something changes.
#[derive(Debug, Clone)]
pub struct PatchEvent {
    pub data: Patch,
}

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

    if is_root() {
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

    let http_settings = HttpSettings {
        enabled: !args.http_disable,
        bind_address: args.http_bind_address,
        cors_enabled: args.http_enable_cors,
        port: args.http_port,
    };

    info!("Starting GoXLR Daemon v{}", VERSION);
    let settings = SettingsHandle::load(args.config).await?;

    let mut shutdown = Shutdown::new();

    let file_manager = FileManager::new(&settings);

    let (file_tx, file_rx) = mpsc::channel(20);
    let file_handle = tokio::spawn(run_notification_service(
        get_file_paths_from_settings(&settings),
        file_tx,
        shutdown.clone(),
    ));

    // This is essentially a SPMC (Single Producer (main worker), Multi-Consumer (IPC and Websocket))
    // which is triggered by the primary worker in the event of a change.
    let (broadcast_tx, broadcast_rx) = broadcast::channel(16);

    // we don't use the receiver generated here, so we'll just drop it and subscribe when needed.
    drop(broadcast_rx);

    let (usb_tx, usb_rx) = mpsc::channel(32);
    let usb_handle = tokio::spawn(handle_changes(
        usb_rx,
        file_rx,
        broadcast_tx.clone(),
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
    let communications_handle = tokio::spawn(run_server(
        ipc_socket,
        http_settings.clone(),
        usb_tx.clone(),
        shutdown.clone(),
    ));

    let mut http_server: Option<ServerHandle> = None;
    if http_settings.enabled {
        if http_settings.cors_enabled {
            warn!("HTTP Cross Origin Requests enabled, this may be a security risk.");
        }
        let (httpd_tx, httpd_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(launch_httpd(
            usb_tx.clone(),
            httpd_tx,
            broadcast_tx.clone(),
            http_settings,
        ));
        http_server = Some(httpd_rx.await?);
    } else {
        warn!("HTTP Server Disabled");
    }

    // Create non-async method of shutting down threads..
    let blocking_shutdown = Arc::new(AtomicBool::new(false));

    // Setup Ctrl+C Monitoring..
    tokio::spawn(await_ctrl_c(shutdown.clone(), blocking_shutdown.clone()));

    // Spawn the Systray Icon + Menu.. This will block until Ctrl+C is received.
    spawn_menu(blocking_shutdown.clone())?;

    info!("Shutting down daemon");

    if let Some(server) = http_server {
        // We only need to Join on the HTTP Server if it exists..
        let _ = join!(
            usb_handle,
            communications_handle,
            server.stop(true),
            file_handle
        );
    } else {
        let _ = join!(usb_handle, communications_handle, file_handle);
    }

    shutdown.recv().await;
    Ok(())
}

async fn await_ctrl_c(shutdown: Shutdown, blocking_shutdown: Arc<AtomicBool>) {
    if signal::ctrl_c().await.is_ok() {
        shutdown.trigger();
        blocking_shutdown.store(true, Ordering::Relaxed);
    }
}

#[cfg(target_family = "unix")]
fn is_root() -> bool {
    nix::unistd::Uid::effective().is_root()
}

#[cfg(not(target_family = "unix"))]
fn is_root() -> bool {
    // On non-unix systems, we can't root check, assume we're good!
    false
}

fn spawn_menu(shutdown: Arc<AtomicBool>) -> Result<()> {
    let tray_menu = Menu::new();
    let hello_menu = MenuItem::new("Hello", true, None);
    tray_menu.append_items(&[&hello_menu]);

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("GoXLR Utility")
        .with_icon(load_icon())
        .build()?;

    let tray_channel = tray_event_receiver();
    let menu_channel = menu_event_receiver();

    // So the problem is, on certain OSs, the Event Loop handler *HAS* to be handled on
    // the main thread. So this is a blocking call. We'll keep an eye out for the shutdown
    // handle being changed, so we can exit gracefully when Ctrl+C is hit.
    let mut event_loop = EventLoopBuilder::new().build();
    event_loop.run_return(move |_event, _, control_flow| {
        // We set this to poll, so we can monitor both the menu, and tray icon..
        control_flow.set_poll();

        if let Ok(event) = menu_channel.try_recv() {
            if event.id == hello_menu.id() {
                debug!("Hello Button Pressed! :)");
            }
            debug!("{:?}", event);
        }

        if let Ok(event) = tray_channel.try_recv() {
            debug!("{:?}", event);
        }

        if shutdown.load(Ordering::Relaxed) {
            debug!("Shutting down Window Event Handler..");
            control_flow.set_exit();
        }
    });

    // When we get here we're done with the event listener. We need to drop the tray icon
    // to ensure any 'background' cleanup is done.
    drop(tray_icon);
    Ok(())
}

fn load_icon() -> tray_icon::icon::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(ICON)
            .expect("Failed to load Icon")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::icon::Icon::from_rgba(icon_rgba, icon_width, icon_height)
        .expect("Failed to load Icon")
}
