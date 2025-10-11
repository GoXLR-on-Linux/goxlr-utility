mod core_audio;
mod device;
pub mod runtime;

use crate::ICON_MAC;
use anyhow::{bail, Result};
use objc2::__framework_prelude::Retained;
use objc2::{AllocAnyThread, MainThreadMarker};
use objc2_app_kit::{NSAlert, NSAlertStyleCritical, NSImage, NSWindowLevel};
use objc2_foundation::{NSData, NSString};
use std::path::Path;
use std::{env, fs};

const PLIST: &[u8] = include_bytes!("../../resources/goxlr-utility.plist.xml");
const PLIST_FILENAME: &str = "com.github.goxlr-on-linux.goxlr-utility.plist";

pub fn display_error(message: String) {
    let mtm = MainThreadMarker::new().unwrap();

    unsafe {
        let alert = NSAlert::new(mtm);
        alert.setIcon(get_icon().as_deref());
        alert.setMessageText(&NSString::from_str("GoXLR Utility"));
        alert.setInformativeText(&NSString::from_str(&message));
        alert.setAlertStyle(NSAlertStyleCritical);

        // Get the Window
        let window = alert.window();
        window.setLevel(NSWindowLevel::from(10u8));

        // Send the Alert
        alert.runModal();
    }
}

fn get_icon() -> Option<Retained<NSImage>> {
    let data = NSData::with_bytes(ICON_MAC);
    NSImage::initWithData(NSImage::alloc(), &data)
}

pub fn has_autostart() -> bool {
    // Check for the Presence of the PLIST file in the Users home..
    if let Ok(path) = env::var("HOME") {
        let path = Path::new(&path)
            .join("Library")
            .join("LaunchAgents")
            .join(PLIST_FILENAME);

        path.exists()
    } else {
        false
    }
}

pub fn set_autostart(enabled: bool) -> Result<()> {
    if let Ok(path) = env::var("HOME") {
        let path = Path::new(&path)
            .join("Library")
            .join("LaunchAgents")
            .join(PLIST_FILENAME);

        if path.exists() && !enabled {
            return fs::remove_file(path).map_err(anyhow::Error::from);
        }

        if path.exists() && enabled {
            bail!("Autostart Already Present");
        }

        let executable = env::current_exe()?;

        // Create the file..
        let plist = String::from_utf8(Vec::from(PLIST))?;
        let built = plist.replace("{{BINARY_PATH}}", &executable.to_string_lossy());

        fs::write(path, built).map_err(anyhow::Error::from)
    } else {
        bail!("Unable to Locate HOME Path");
    }
}
