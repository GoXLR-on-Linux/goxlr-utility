use crate::events::EventTriggers;
use crate::{DaemonState, ICON};
use anyhow::Result;
use goxlr_ipc::PathTypes::{Icons, MicProfiles, Presets, Profiles, Samples};
use ksni::menu::{StandardItem, SubMenu};
use ksni::{Category, Icon, MenuItem, Status, ToolTip, Tray, TrayService};
use log::debug;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use tiny_skia::Pixmap;
use tokio::sync::mpsc;

pub fn handle_tray(state: DaemonState, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
    // Attempt to immediately update the environment..
    let tray_service = TrayService::new(GoXLRTray::new(tx));
    let tray_handle = tray_service.handle();
    tray_service.spawn();

    while !state.shutdown_blocking.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
        tray_handle.update(|_| {});
    }

    debug!("Shutting Down Tray Handler..");
    tray_handle.shutdown();
    Ok(())
}

struct GoXLRTray {
    tx: mpsc::Sender<EventTriggers>,
    icon: Icon,
}

impl GoXLRTray {
    fn new(tx: mpsc::Sender<EventTriggers>) -> Self {
        // Generate the icon..
        let pixmap = Pixmap::decode_png(ICON).unwrap();
        let rgba_data = GoXLRTray::rgba_to_argb(pixmap.data());

        let icon = Icon {
            width: pixmap.width() as i32,
            height: pixmap.height() as i32,
            data: rgba_data,
        };
        Self { tx, icon }
    }

    // Probably a better way to handle this..
    fn rgba_to_argb(input: &[u8]) -> Vec<u8> {
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

    fn id(&self) -> String {
        "goxlr-utility".to_string()
    }

    fn title(&self) -> String {
        String::from("GoXLR Utility")
    }

    fn status(&self) -> Status {
        Status::Active
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![self.icon.clone()]
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
