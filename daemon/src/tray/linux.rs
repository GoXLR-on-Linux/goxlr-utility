use crate::ICON;
use anyhow::Result;
use detect_desktop_environment::DesktopEnvironment;
use ksni::menu::StandardItem;
use ksni::{Category, Icon, MenuItem, Status, ToolTip, Tray, TrayService};
use log::debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tiny_skia::Pixmap;

pub fn handle_tray(shutdown: Arc<AtomicBool>) -> Result<()> {
    let tray_service = TrayService::new(GoXLRTray::new(DesktopEnvironment::detect()));
    let tray_handle = tray_service.handle();
    tray_service.spawn();

    while !shutdown.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
    }

    debug!("Shutting Down Tray Handler..");
    tray_handle.shutdown();
    Ok(())
}

struct GoXLRTray {
    environment: DesktopEnvironment,
}

impl GoXLRTray {
    fn new(environment: DesktopEnvironment) -> Self {
        Self { environment }
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
        debug!("Icon Pressed!");
    }

    fn category(&self) -> Category {
        Category::Hardware
    }

    fn title(&self) -> String {
        String::from("GoXLR Utility")
    }

    fn status(&self) -> Status {
        if self.environment == DesktopEnvironment::Kde {
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
        vec![StandardItem {
            label: String::from("Hello!"),
            activate: Box::new(|_this: &mut GoXLRTray| {
                debug!("Hello Pressed!");
            }),
            ..Default::default()
        }
        .into()]
    }
}
