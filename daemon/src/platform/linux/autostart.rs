use anyhow::{bail, Result};
use ini::Ini;
use lazy_static::lazy_static;
use log::debug;
use std::path::PathBuf;
use std::{env, fs};

const AUTOSTART_FILENAME: &str = "goxlr-daemon.desktop";

lazy_static! {
    static ref STARTUP_PATH: Option<PathBuf> = get_startup_dir();
}

pub fn has_autostart() -> bool {
    if let Some(path) = &*STARTUP_PATH {
        return path.join(AUTOSTART_FILENAME).exists();
    }

    false
}

pub fn remove_startup_link() -> Result<()> {
    if let Some(path) = &*STARTUP_PATH {
        let file = path.join(AUTOSTART_FILENAME);
        if !file.exists() {
            debug!("Attempted to remove link on non-existent file");
            return Ok(());
        }

        // Remove the file.
        fs::remove_file(file)?;
    }
    Ok(())
}

pub fn create_startup_link() -> Result<()> {
    if let Some(path) = &*STARTUP_PATH {
        // Get the Executable Path..
        let executable = env::current_exe()?;

        if let Some(parent) = executable.parent() {
            let file = path.join(AUTOSTART_FILENAME);

            let mut conf = Ini::new();
            conf.with_section(Some("Desktop Entry"))
                .set("Type", "Application")
                .set("Name", "GoXLR Utility")
                .set("Comment", "A Tool for Configuring a GoXLR")
                .set("Path", parent.to_string_lossy())
                .set("Exec", executable.to_string_lossy())
                .set("Terminal", "false");

            conf.write_to_file(file)?;
            return Ok(());
        }
    }
    bail!("Unable to create Startup File (Startup Path not found)");
    // Ok(())
}

fn get_startup_dir() -> Option<PathBuf> {
    // Attempt to locate the 'Autostart' directory based on the XDG Specification:
    // https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html

    let mut xdg_config = None;

    // Check if $XDG_CONFIG_HOME is set..
    if let Ok(path) = env::var("XDG_CONFIG_HOME") {
        xdg_config.replace(path);
    } else if let Ok(path) = env::var("HOME") {
        xdg_config.replace(format!("{path}/.config"));
    }

    if let Some(path) = xdg_config {
        let path_buf = PathBuf::from(path).join("autostart");
        if path_buf.exists() {
            debug!("Found XDG AutoStart Path: {:?}", path_buf);
            return Some(path_buf);
        } else if fs::create_dir_all(&path_buf).is_ok() {
            // Attempt to create the path if it doesn't exist..
            return Some(path_buf);
        }
    }

    None
}
