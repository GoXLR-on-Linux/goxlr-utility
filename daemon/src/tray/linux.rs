use crate::events::EventTriggers;
use crate::{DaemonState, ICON};
use anyhow::{anyhow, Result};
use detect_desktop_environment::DesktopEnvironment;
use goxlr_ipc::PathTypes::{Icons, MicProfiles, Presets, Profiles, Samples};
use ksni::menu::{StandardItem, SubMenu};
use ksni::{Category, Icon, MenuItem, Status, ToolTip, Tray, TrayService};
use log::debug;
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{env, thread};
use tiny_skia::Pixmap;
use tokio::sync::mpsc;

pub fn handle_tray(state: DaemonState, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
    // Attempt to immediately update the environment..
    let _ = update_environment();

    let tray_service = TrayService::new(GoXLRTray::new(tx));
    let tray_handle = tray_service.handle();
    tray_service.spawn();

    let mut count = 0;
    while !state.shutdown_blocking.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));

        count += 1;
        if count == 50 {
            count = 0;

            // Do an environment update check every 5 seconds..
            let _ = update_environment();

            // Instruct the icon to update.
            tray_handle.update(|_| {});
        }
    }

    debug!("Shutting Down Tray Handler..");
    tray_handle.shutdown();
    Ok(())
}

struct GoXLRTray {
    tx: mpsc::Sender<EventTriggers>,
}

impl GoXLRTray {
    fn new(tx: mpsc::Sender<EventTriggers>) -> Self {
        Self { tx }
    }

    // Probably a better way to handle this..
    fn rgba_to_argb(&self, input: &[u8]) -> Vec<u8> {
        let mut moved = Vec::new();

        for chunk in input.chunks(4) {
            moved.push(chunk[3]);
            moved.push(chunk[0]);
            moved.push(chunk[1]);
            moved.push(chunk[2]);
        }

        moved
    }
}

impl Tray for GoXLRTray {
    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.blocking_send(EventTriggers::OpenUi);
    }

    fn category(&self) -> Category {
        Category::Hardware
    }

    fn title(&self) -> String {
        String::from("GoXLR Utility")
    }

    fn status(&self) -> Status {
        if DesktopEnvironment::detect() == DesktopEnvironment::Kde {
            // Under KDE, setting this to 'Passive' puts it cleanly in 'Status and Notifications'.
            return Status::Passive;
        }

        // Under other DEs (inc gnome), if it's passive, it disappears.
        Status::Active
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        let pixmap = Pixmap::decode_png(ICON).unwrap();

        let rgba_data = self.rgba_to_argb(pixmap.data());

        vec![Icon {
            width: pixmap.width() as i32,
            height: pixmap.height() as i32,
            data: rgba_data,
        }]
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: String::from("GoXLR Utility"),
            description: String::from("A Tool for Configuring a GoXLR under Linux"),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: String::from("Configure GoXLR"),
                activate: Box::new(|this: &mut GoXLRTray| {
                    let _ = this.tx.blocking_send(EventTriggers::OpenUi);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            SubMenu {
                label: String::from("Open Path"),
                submenu: vec![
                    StandardItem {
                        label: String::from("Profiles"),
                        activate: Box::new(|this: &mut GoXLRTray| {
                            let _ = this.tx.blocking_send(EventTriggers::Open(Profiles));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: String::from("Mic Profiles"),
                        activate: Box::new(|this: &mut GoXLRTray| {
                            let _ = this.tx.blocking_send(EventTriggers::Open(MicProfiles));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    MenuItem::Separator,
                    StandardItem {
                        label: String::from("Presets"),
                        activate: Box::new(|this: &mut GoXLRTray| {
                            let _ = this.tx.blocking_send(EventTriggers::Open(Presets));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: String::from("Samples"),
                        activate: Box::new(|this: &mut GoXLRTray| {
                            let _ = this.tx.blocking_send(EventTriggers::Open(Samples));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: String::from("Icons"),
                        activate: Box::new(|this: &mut GoXLRTray| {
                            let _ = this.tx.blocking_send(EventTriggers::Open(Icons));
                        }),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: String::from("Quit"),
                activate: Box::new(|this: &mut GoXLRTray| {
                    let _ = this.tx.blocking_send(EventTriggers::Stop);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// This simply attempts to update the Daemon Environment based on what systemd says the
/// Current ENV is. The main reason for doing this is that during startup, systemd can
/// launch the Daemon prior to things like BROWSER and XDG_CURRENT_DESKTOP being set, and
/// we need both of those for things like launching a web browser, and correctly rendering
/// the System Tray Icon.
fn update_environment() -> Result<()> {
    // These variables are used by xdg-open to determine how to launch stuff.
    let vars: Vec<&str> = Vec::from([
        "XDG_CURRENT_DESKTOP",
        "DESKTOP_SESSION",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "KDE_SESSION_VERSION",
        "XDG_DATA_HOME",
        "XDG_DATA_DIRS",
        "XDG_RUNTIME_DIR",
        "XDG_SESSION_TYPE",
        "XAUTHORITY",
        "BROWSER",
    ]);

    let env_list = get_current_environment_vars();
    if env_list.is_err() {
        // Likely systemctl command failed, ignore gracefully.
        return Ok(());
    }

    let env_list = env_list.unwrap();
    for variable in vars {
        if env::var(variable).is_err() && env_list.contains_key(variable) {
            debug!(
                "Setting Environmental Variable: {} to {}",
                variable,
                env_list.get(variable).unwrap()
            );
            env::set_var(variable, env_list.get(variable).unwrap());
        }
    }
    Ok(())
}

fn get_current_environment_vars() -> Result<HashMap<String, String>> {
    let command = Command::new("systemctl")
        .arg("--user")
        .arg("show-environment")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()?;

    if !command.status.success() {
        return Err(anyhow!("Unable to fetch environment from systemd"));
    }

    // Grab the output, and split it into key/value pairs..
    let found = String::from_utf8(command.stdout)?;
    Ok(found
        .lines()
        .map(|s| s.split_at(s.find('=').unwrap()))
        .map(|(key, val)| (String::from(key), String::from(&val[1..])))
        .collect())
}
