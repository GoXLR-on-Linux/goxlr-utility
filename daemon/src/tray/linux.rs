use crate::events::EventTriggers;
use crate::shutdown::Shutdown;
use crate::ICON;
use anyhow::Result;
use goxlr_ipc::PathTypes::{Icons, Logs, MicProfiles, Presets, Profiles, Samples};
use ksni::menu::{StandardItem, SubMenu};
use ksni::{Category, MenuItem, Status, ToolTip, Tray, TrayMethods};
use log::{debug, warn};
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub async fn handle_tray(mut stop: Shutdown, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
    // Before we spawn the tray, we're going to extract our icon to a temporary location
    // so that it can be immediately used. Depending on pixmaps seems to cause issues under
    // gnome, where occasionally the icon will not correctly spawn.

    // We'll dump the icon here :)
    let tmp_file_dir = PathBuf::from("/tmp/goxlr-utility/");

    // Extract the icon to a temporary directory, and pass its path..
    let tmp_file_path = tmp_file_dir.join("goxlr-utility-icon.png");
    if !tmp_file_dir.exists() {
        fs::create_dir_all(&tmp_file_dir)?;
    }

    // Rather than random shenanigans, we'll simply try to remove any existing files and
    // recycle whatever is there if we can't. These should evaluate in order, so if the
    // file is absent, or the file was successfully removed, we can write to it.
    if !tmp_file_path.exists() || fs::remove_file(&tmp_file_path).is_ok() {
        fs::write(&tmp_file_path, ICON)?;
    } else {
        warn!("Unable to remove existing icon, using whatever is already there..");
    }

    // Attempt to immediately update the environment..
    let icon = GoXLRTray::new(tx, &tmp_file_path);
    let handle = icon.spawn().await;

    let handle = match handle {
        Ok(handle) => handle,
        Err(e) => {
            // There's no harm in running without a tray icon, in some cases this may actually
            // be preferable (for example, when running under the CLI), so we just warn, tidy
            // up, and consider our work here done.
            fs::remove_file(&tmp_file_path)?;
            warn!("Unable to Spawn the Tray Handler: {}", e);
            return Ok(());
        }
    };

    stop.recv().await;

    debug!("Shutting Down Tray Handler..");
    let _ = handle.shutdown().await;
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
    fn id(&self) -> String {
        "goxlr-utility".to_string()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.try_send(EventTriggers::Activate);
    }

    fn category(&self) -> Category {
        Category::Hardware
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
                    let _ = this.tx.try_send(EventTriggers::Activate);
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
                            let _ = this.tx.try_send(EventTriggers::Open(Profiles));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: String::from("Mic Profiles"),
                        activate: Box::new(|this: &mut GoXLRTray| {
                            let _ = this.tx.try_send(EventTriggers::Open(MicProfiles));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    MenuItem::Separator,
                    StandardItem {
                        label: String::from("Presets"),
                        activate: Box::new(|this: &mut GoXLRTray| {
                            let _ = this.tx.try_send(EventTriggers::Open(Presets));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: String::from("Samples"),
                        activate: Box::new(|this: &mut GoXLRTray| {
                            let _ = this.tx.try_send(EventTriggers::Open(Samples));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: String::from("Icons"),
                        activate: Box::new(|this: &mut GoXLRTray| {
                            let _ = this.tx.try_send(EventTriggers::Open(Icons));
                        }),
                        ..Default::default()
                    }
                    .into(),
                    MenuItem::Separator,
                    StandardItem {
                        label: String::from("Logs"),
                        activate: Box::new(|this: &mut GoXLRTray| {
                            let _ = this.tx.try_send(EventTriggers::Open(Logs));
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
                    let _ = this.tx.try_send(EventTriggers::Stop(false));
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
