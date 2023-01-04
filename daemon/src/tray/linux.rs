use crate::events::EventTriggers;
use crate::{DaemonState, ICON};
use anyhow::Result;
use goxlr_ipc::PathTypes::{Icons, MicProfiles, Presets, Profiles, Samples};
use ksni::menu::{StandardItem, SubMenu};
use ksni::{Category, MenuItem, Status, ToolTip, Tray, TrayService};
use log::debug;
use rand::Rng;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::{fs, thread};
use tokio::sync::mpsc;

pub fn handle_tray(state: DaemonState, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
    // Before we spawn the tray, we're going to extract our icon to a temporary location
    // so that it can be immediately used. Depending on pixmaps seems to cause issues under
    // gnome, where occasionally the icon wont correctly spawn.

    // Firstly, we use a random filename (it'll be removed on shutdown) to prevent potential
    // weirdness in the event it gets locked somehow (different user / crash scenario)
    let file_name = format!("goxlr-utility-{}.png", rand::thread_rng().gen::<u16>());

    // We'll dump the icon here :)
    let tmp_file_dir = PathBuf::from("/tmp/goxlr-utility/");

    // Extract the icon to a temporary directory, and pass its path..
    let tmp_file_path = tmp_file_dir.join(file_name);
    if !tmp_file_dir.exists() {
        fs::create_dir_all(&tmp_file_dir)?;
    }
    fs::write(&tmp_file_path, ICON)?;

    // Attempt to immediately update the environment..
    let tray_service = TrayService::new(GoXLRTray::new(tx, &tmp_file_path));
    let tray_handle = tray_service.handle();
    tray_service.spawn();

    while !state.shutdown_blocking.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
    }

    debug!("Shutting Down Tray Handler..");
    tray_handle.shutdown();
    fs::remove_file(&tmp_file_path)?;
    Ok(())
}

struct GoXLRTray {
    tx: mpsc::Sender<EventTriggers>,
    icon: PathBuf,
}

impl GoXLRTray {
    fn new(tx: mpsc::Sender<EventTriggers>, icon: &Path) -> Self {
        let icon = icon.to_path_buf();
        Self { tx, icon }
    }
}

impl Tray for GoXLRTray {
    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.blocking_send(EventTriggers::OpenUi);
    }

    fn category(&self) -> Category {
        Category::Hardware
    }

    fn id(&self) -> String {
        "goxlr-utility".to_string()
    }

    fn title(&self) -> String {
        String::from("GoXLR Utility")
    }

    fn status(&self) -> Status {
        Status::Active
    }

    fn icon_theme_path(&self) -> String {
        if let Some(parent) = self.icon.parent() {
            return parent.to_string_lossy().to_string();
        }

        String::from("")
    }

    fn icon_name(&self) -> String {
        if let Some(file) = self.icon.file_stem() {
            return file.to_string_lossy().to_string();
        }
        "goxlr-utility".to_string()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: String::from("GoXLR Utility"),
            description: String::from("A Tool for Configuring a GoXLR"),
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
