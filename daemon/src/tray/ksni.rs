use crate::WHITE_ICON;
use anyhow::Result;
use ksni::menu::StandardItem;
use ksni::{Category, Icon, MenuItem, Status, Tray, TrayService};
use log::debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tiny_skia::Pixmap;

pub fn handle_tray(shutdown: Arc<AtomicBool>) -> Result<()> {
    let tray_service = TrayService::new(GoXLRTray::new());
    let tray_handle = tray_service.handle();
    tray_service.spawn();

    while !shutdown.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(10));
    }

    debug!("Shutting Down Tray Handler..");
    tray_handle.shutdown();
    Ok(())
}

struct GoXLRTray {}

impl GoXLRTray {
    fn new() -> Self {
        Self {}
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
        Status::Passive
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        let pixmap = Pixmap::decode_png(WHITE_ICON).unwrap();
        let rgba_data = self.rgba_to_argb(pixmap.data());

        vec![Icon {
            width: pixmap.width() as i32,
            height: pixmap.height() as i32,
            data: rgba_data,
        }]
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
