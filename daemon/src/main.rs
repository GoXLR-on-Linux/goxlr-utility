#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate core;

use std::fs::create_dir_all;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use actix_web::dev::ServerHandle;
use anyhow::{bail, Context, Result};
use clap::Parser;
use enum_map::EnumMap;
use file_rotate::compression::Compression;
use file_rotate::suffix::AppendCount;
use file_rotate::{ContentLimit, FileRotate};
use json_patch::Patch;
use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use simplelog::{
    ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode, WriteLogger,
};
use sys_locale::get_locale;

use tokio::join;
use tokio::sync::{broadcast, mpsc};

use goxlr_ipc::{FirmwareSource, HttpSettings, LogLevel};

use crate::cli::{Cli, LevelFilter};
use crate::events::{spawn_event_handler, DaemonState, EventTriggers};
use crate::files::{spawn_file_notification_service, FileManager};
use crate::platform::perform_preflight;
use crate::platform::spawn_runtime;
use crate::primary_worker::spawn_usb_handler;
use crate::servers::http_server::spawn_http_server;
use crate::servers::ipc_server::{bind_socket, spawn_ipc_server};
use crate::settings::SettingsHandle;
use crate::shutdown::Shutdown;
use crate::tts::spawn_tts_service;

mod audio;
mod cli;
mod device;
mod events;
mod files;
mod firmware;
mod mic_profile;
mod platform;
mod primary_worker;
mod profile;
mod servers;
mod settings;
mod shutdown;
mod tray;
mod tts;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const ICON: &[u8] = include_bytes!("../resources/goxlr-utility-large.png");

const FIRMWARE_PATHS: EnumMap<FirmwareSource, &str> = EnumMap::from_array([
    "https://mediadl.musictribe.com/media/PLM/sftp/incoming/hybris/import/GOXLR/",
    "https://mediadl.musictribe.com/media/PLM/sftp/incoming/hybris/import/FirmwareAssets/GOXLR/LiveTestArea/",
]);

/**
This is ugly, and I know it's ugly. I need to rework how the Primary Worker is constructed
so that various variables can be easily passed through it down to the device level via a struct
rather than through additional parameters. When that comes, this will be removed!
*/
static OVERRIDE_SAMPLER_INPUT: Mutex<Option<String>> = Mutex::new(None);
static OVERRIDE_SAMPLER_OUTPUT: Mutex<Option<String>> = Mutex::new(None);

/**
    This is also ugly, but for now it's important to allow users to simply disable aggregate
    management, and have the utility obey.
*/
pub static HANDLE_MACOS_AGGREGATES: Mutex<Option<bool>> = Mutex::new(Some(true));

lazy_static! {
    /**
        This is a fetcher of the system locale, used for language and translations of the UI.
        the sys-locale package should give us valid readings on Linux, MacOS and Windows
     */
    static ref SYSTEM_LOCALE: String = get_locale()
        .unwrap_or_else(|| String::from("en_GB"))
        .replace('-', "_");
}

// This is for global 'JSON Patches', for when something changes.
#[derive(Debug, Clone)]
pub struct PatchEvent {
    pub data: Patch,
}

#[tokio::main]
async fn main() -> Result<()> {
    // If running the utility has an error, make sure log level is debug, and propagate the
    // error up to the user on Windows.
    if let Err(e) = run_utility().await {
        let args: Cli = Cli::parse();
        let settings = SettingsHandle::load(args.config).await?;

        if settings.get_log_level().await != LogLevel::Debug {
            info!("Setting Log Level to Debug for next run..");
            settings.set_log_level(LogLevel::Debug).await;
            settings.save().await;
        }

        // Message is Cross-Platform now :)
        let message = format!("Error Starting the GoXLR Utility:\r\n\r\n{}", e);
        platform::display_error(message);

        bail!("{}", e);
    }

    Ok(())
}

async fn run_utility() -> Result<()> {
    // We're just going to re-parse the args here, while we've technically done it above,
    // they get moved into the settings loader, which just causes headaches :D
    let args: Cli = Cli::parse();
    let settings = SettingsHandle::load(args.config).await?;

    // Set the MacOS Aggregate management..
    let aggregates = settings.get_macos_handle_aggregates().await;
    HANDLE_MACOS_AGGREGATES.lock().unwrap().replace(aggregates);

    // Configure and / or create the log path, and file name.
    let log_path = settings.get_log_directory().await;
    if !log_path.clone().exists() {
        if let Err(e) = create_dir_all(log_path.clone()) {
            bail!("Unable to create log directory: {}", e);
        }
    }
    let log_file = log_path.join("goxlr-daemon.log");

    // We need to ignore a couple of packages log output so create a builder.
    let mut config = ConfigBuilder::new();

    // The tracing package, when used, will output to INFO from zbus every second..
    config.add_filter_ignore_str("tracing");

    // Actix is a little noisy on startup and shutdown, so quiet it a bit :)
    config.add_filter_ignore_str("actix_server::accept");
    config.add_filter_ignore_str("actix_server::worker");
    config.add_filter_ignore_str("actix_server::server");
    config.add_filter_ignore_str("actix_server::builder");
    config.add_filter_ignore_str("zbus");
    config.add_filter_ignore_str("hyper_util");
    config.add_filter_ignore_str("reqwest");

    // I'm generally not interested in the Symphonia header announcements which go to INFO,
    // it's only useful in a development setting!
    config.add_filter_ignore_str("symphonia");

    let timezone_calculated = config.set_time_offset_to_local().is_ok();

    // Create a file rotator, that will compress and rotate files after 5Mb
    let file_rotator = FileRotate::new(
        log_file,
        AppendCount::new(5),
        ContentLimit::Bytes(1024 * 1024 * 2),
        Compression::OnRotate(1),
        #[cfg(unix)]
        None,
    );

    // Configure the log level, prioritise the CLI, but otherwise config.
    let log_level = if let Some(cli_level) = args.log_level {
        match cli_level {
            LevelFilter::Off => log::LevelFilter::Off,
            LevelFilter::Error => log::LevelFilter::Error,
            LevelFilter::Warn => log::LevelFilter::Warn,
            LevelFilter::Info => log::LevelFilter::Info,
            LevelFilter::Debug => log::LevelFilter::Debug,
            LevelFilter::Trace => log::LevelFilter::Trace,
        }
    } else {
        match settings.get_log_level().await {
            LogLevel::Off => log::LevelFilter::Off,
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    };

    // Create the loggers :)
    CombinedLogger::init(vec![
        TermLogger::new(
            log_level,
            config.build(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(log_level, config.build(), file_rotator),
    ])
    .context("Could not configure the logger")?;

    // Enable the PANIC logger..
    log_panics::init();

    if !timezone_calculated {
        warn!("Unable to calculate timezone, using UTC for log timestamps");
    }

    if cfg!(target_os = "macos") {
        debug!(
            "Configure MacOS Aggregates: {:?}",
            HANDLE_MACOS_AGGREGATES.lock().unwrap().unwrap()
        );
    }
    if is_root() {
        if args.force_root {
            error!("GoXLR Utility running as root, this is generally considered bad.");
        } else {
            error!("The GoXLR Utility Daemon is not designed to be run as root, and should run");
            error!("as the current active user. If you're having problems with permissions,");
            error!("please consult the 'Permissions' section of the README. Running as root");
            error!("*WILL* cause issues with the sampler, and may pose a security risk.");
            error!("");

            if cfg!(target_os = "macos") {
                error!("As a MacOS user, you may be attempting to run as root to solve the");
                error!("issues of initialisation. The correct approach to this is to run the");
                error!("goxlr-initialiser binary via sudo whenever a GoXLR device is attached.");
                error!("This can be achieved either via a launchctl script or manually on the");
                error!("command line.");
                error!("");
            }
            error!("To override this message, please start with --force-root");
            std::process::exit(-1);
        }
    }

    if let Some(device) = args.override_sample_input_device {
        OVERRIDE_SAMPLER_INPUT.lock().unwrap().replace(device);
    }

    if let Some(device) = args.override_sample_output_device {
        OVERRIDE_SAMPLER_OUTPUT.lock().unwrap().replace(device);
    }

    info!("Starting GoXLR Daemon v{}", VERSION);
    info!("System Locale: {}", *SYSTEM_LOCALE);

    // Before we do anything, perform platform pre-flight to make
    // sure we're allowed to start.
    info!("Performing Platform Preflight...");
    perform_preflight()?;

    let bind_address = if let Some(address) = args.http_bind_address {
        debug!("Command Line Override, binding to: {}", address);
        address
    } else if settings.get_allow_network_access().await {
        String::from("0.0.0.0")
    } else {
        String::from("localhost")
    };

    debug!("HTTP Bind Address: {}", bind_address);
    let http_settings = HttpSettings {
        enabled: !args.http_disable,
        bind_address,
        cors_enabled: args.http_enable_cors,
        port: args.http_port,
    };

    // Create the Global Event Channel..
    let (global_tx, global_rx) = mpsc::channel(32);

    // Create the 'Patch' Sending Channel..
    let (broadcast_tx, broadcast_rx) = broadcast::channel(16);
    drop(broadcast_rx);

    // Create the USB Event Channel..
    let (usb_tx, usb_rx) = mpsc::channel(32);

    // Create the TTS Event Channel..
    let (tts_sender, tts_rx) = mpsc::channel(32);

    // Create the HTTP Run Channel..
    let (httpd_tx, httpd_rx) = tokio::sync::oneshot::channel();

    // Create the Device shutdown signallers..
    let (device_state_tx, device_state_rx) = mpsc::channel(1);

    // Create the Shutdown Signallers..
    let shutdown = Shutdown::new();
    let shutdown_blocking = Arc::new(AtomicBool::new(false));

    // Configure Showing the Tray Icon
    let show_tray = Arc::new(AtomicBool::new(settings.get_show_tray_icon().await));
    if let Some(override_tray) = args.disable_tray {
        show_tray.store(override_tray, Ordering::Relaxed);
    }

    // Configure, and Start the File Manager Service..
    let file_manager = FileManager::new(&settings).await;
    let file_paths = file_manager.paths().clone();

    let (file_tx, file_rx) = mpsc::channel(20);
    let file_handle = tokio::spawn(spawn_file_notification_service(
        file_paths.clone(),
        file_tx,
        shutdown.clone(),
    ));

    // Spawn the IPC Socket..
    let ipc_socket = bind_socket().await;
    if let Err(e) = ipc_socket {
        error!("Error Binding IPC Socket: {}", e);
        bail!("{}", e);
    }

    // Start the USB Device Handler
    let usb_handle = tokio::spawn(spawn_usb_handler(
        usb_rx,
        file_rx,
        device_state_rx,
        broadcast_tx.clone(),
        global_tx.clone(),
        shutdown.clone(),
        settings.clone(),
        http_settings.clone(),
        file_manager,
    ));

    // Launch the IPC Server..
    let ipc_socket = ipc_socket?;
    let communications_handle = tokio::spawn(spawn_ipc_server(
        ipc_socket,
        usb_tx.clone(),
        shutdown.clone(),
    ));

    // Run the HTTP Server (if enabled)..
    let mut http_server: Result<Option<ServerHandle>> = Ok(None);
    if http_settings.enabled {
        // Spawn a oneshot channel for managing the HTTP Server
        if http_settings.cors_enabled {
            warn!("HTTP Cross Origin Requests enabled, this may be a security risk.");
        }

        tokio::spawn(spawn_http_server(
            usb_tx.clone(),
            httpd_tx,
            broadcast_tx.clone(),
            http_settings.clone(),
            file_paths.clone(),
        ));
        http_server = httpd_rx.await?;
        if let Err(e) = http_server {
            bail!("Unable to Start HTTP Server: {}", e);
        }
    } else {
        warn!("HTTP Server Disabled");
    }

    // Start the TTS Service..
    let tts_handle = tokio::spawn(spawn_tts_service(
        settings.clone(),
        tts_rx,
        shutdown.clone(),
    ));

    let mut local_shutdown = shutdown.clone();
    let state = DaemonState {
        tts_sender,

        show_tray,
        shutdown,
        shutdown_blocking,

        settings_handle: settings.clone(),
        http_settings: http_settings.clone(),
    };

    // Spawn the general event handler..
    let event_handle = tokio::spawn(spawn_event_handler(
        state.clone(),
        global_rx,
        device_state_tx,
    ));

    // Spawn the Platform Runtime (if needed)
    let platform_handle = tokio::spawn(spawn_runtime(state.clone(), global_tx.clone()));

    if args.start_ui || settings.get_open_ui_on_launch().await {
        let _ = global_tx.send(EventTriggers::Activate).await;
    }

    // Tray management has to occur on the main thread, so we'll start it now.
    tray::handle_tray(state.clone(), global_tx.clone())?;

    // If the tray handler dies for any reason, we should still make sure we've been asked to
    // shut down.
    local_shutdown.recv().await;
    info!("Shutting down daemon");

    if let Ok(Some(server)) = http_server {
        // We only need to Join on the HTTP Server if it exists..
        let _ = join!(
            usb_handle,
            communications_handle,
            server.stop(false),
            file_handle,
            tts_handle,
            event_handle,
            platform_handle
        );
    } else {
        let _ = join!(
            usb_handle,
            communications_handle,
            file_handle,
            tts_handle,
            event_handle,
            platform_handle
        );
    }
    Ok(())
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
