mod core_audio;
mod device;
pub mod runtime;

use crate::ICON;
use anyhow::{bail, Result};
use cocoa::appkit::NSImage;
use cocoa_foundation::base::{id, nil};
use cocoa_foundation::foundation::{NSData, NSString};
use log::debug;
use objc::{class, msg_send, sel, sel_impl};
use std::path::Path;
use std::{env, fs};

const PLIST: &[u8] = include_bytes!("../../resources/goxlr-utility.plist.xml");
const PLIST_FILENAME: &str = "com.github.goxlr-on-linux.goxlr-utility.plist";

pub fn display_error(message: String) {
    unsafe {
        let alert: id = msg_send![class!(NSAlert), alloc];
        let () = msg_send![alert, init];
        let () = msg_send![alert, autorelease];
        let () = msg_send![alert, setIcon: get_icon()];
        let () = msg_send![alert, setMessageText: NSString::alloc(nil).init_str("GoXLR Utility")];
        let () = msg_send![alert, setInformativeText: NSString::alloc(nil).init_str(&message)];
        let () = msg_send![alert, setAlertStyle: 2];

        // Get the Window..
        let window: id = msg_send![alert, window];
        let () = msg_send![window, setLevel: 10];

        // Send the Alert..
        let () = msg_send![alert, runModal];
    }
}

fn get_icon() -> id {
    unsafe {
        let data = NSData::dataWithBytes_length_(
            nil,
            ICON.as_ptr() as *const std::os::raw::c_void,
            ICON.len() as u64,
        );
        NSImage::initWithData_(NSImage::alloc(nil), data)
    }
}

pub fn has_autostart() -> bool {
    // Check for the Presence of the PLIST file in the Users home..
    return if let Ok(path) = env::var("HOME") {
        let path = Path::new(&path)
            .join("Library")
            .join("LaunchAgents")
            .join(PLIST_FILENAME);

        debug!("Checking for {:?}", path);
        path.exists()
    } else {
        false
    };
}

pub fn set_autostart(enabled: bool) -> Result<()> {
    if let Ok(path) = env::var("HOME") {
        let path = Path::new(&path)
            .join("Library")
            .join("LaunchAgents")
            .join(PLIST_FILENAME);

        debug!("Checking for {:?}", path);

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
