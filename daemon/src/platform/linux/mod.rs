pub mod autostart;
pub mod headphone_eq;
pub mod sleep;

use anyhow::Result;
use log::{debug, warn};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RULE_SEARCH_PATHS: [&str; 3] = ["/etc/udev/rules.d", "/usr/lib/udev/rules.d", "/lib/udev/rules.d"];
const SPLIT_CONF_PATH: &str = "/usr/share/alsa/ucm2/common/pcm/split.conf";

pub fn perform_platform_preflight() -> Result<()> {
    check_udev_rules();
    check_usb_power_policy();
    check_audio_services();
    check_alsa_split_conf();
    Ok(())
}

pub fn display_error(message: String) {
    use std::process::Command;
    // We have two choices here, kdialog, or zenity. We'll try both.
    if let Err(e) = Command::new("kdialog")
        .arg("--title")
        .arg("GoXLR Utility")
        .arg("--error")
        .arg(message.clone())
        .output()
    {
        println!("Error Running kdialog: {e}, falling back to zenity..");
        let _ = Command::new("zenity")
            .arg("--title")
            .arg("GoXLR Utility")
            .arg("--error")
            .arg("--text")
            .arg(message)
            .output();
    }
}

fn check_udev_rules() {
    if has_goxlr_rule() {
        return;
    }

    warn!(
        "No GoXLR udev rule found. Device permission/reconnect issues may occur until rules are installed."
    );
}

fn has_goxlr_rule() -> bool {
    for directory in RULE_SEARCH_PATHS {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let Some(ext) = path.extension() else {
                continue;
            };
            if ext != "rules" {
                continue;
            }

            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().contains("goxlr"))
            {
                return true;
            }

            if let Ok(contents) = fs::read_to_string(path)
                && (contents.contains("1220") || contents.to_lowercase().contains("goxlr"))
            {
                return true;
            }
        }
    }

    false
}

fn check_usb_power_policy() {
    let mut found_devices = 0usize;
    let mut non_on_devices = Vec::new();

    let entries = match fs::read_dir("/sys/bus/usb/devices") {
        Ok(entries) => entries,
        Err(_) => {
            debug!("Unable to inspect /sys/bus/usb/devices for GoXLR preflight checks.");
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_goxlr_usb_device(&path) {
            continue;
        }

        found_devices += 1;
        let control_path = path.join("power/control");
        let Some(control) = read_trimmed(&control_path) else {
            continue;
        };

        if control != "on" {
            let name = path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| String::from("unknown"));
            non_on_devices.push((name, control));
        }
    }

    if found_devices == 0 {
        debug!("No GoXLR USB device detected during preflight.");
        return;
    }

    for (name, control) in non_on_devices {
        warn!(
            "GoXLR USB power policy is '{}' on {} (expected 'on'). Suspend/reconnect instability may occur.",
            control, name
        );
    }
}

fn is_goxlr_usb_device(path: &Path) -> bool {
    let Some(vendor) = read_trimmed(&path.join("idVendor")) else {
        return false;
    };
    let Some(product) = read_trimmed(&path.join("idProduct")) else {
        return false;
    };

    vendor == "1220" && product.starts_with("8fe")
}

fn check_audio_services() {
    let pipewire_running = process_running("pipewire");
    let wireplumber_running = process_running("wireplumber");

    if !(pipewire_running && wireplumber_running) {
        warn!(
            "PipeWire session looks incomplete (pipewire={}, wireplumber={}).",
            pipewire_running as u8, wireplumber_running as u8
        );
    }

    let pactl = Command::new("pactl").arg("info").status();
    if let Ok(status) = pactl
        && !status.success()
    {
        warn!("Unable to query active PulseAudio/PipeWire control socket via pactl.");
    }
}

fn process_running(name: &str) -> bool {
    Command::new("pgrep")
        .arg("-x")
        .arg(name)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn check_alsa_split_conf() {
    let split_conf = PathBuf::from(SPLIT_CONF_PATH);
    if !split_conf.exists() {
        debug!("ALSA split.conf not found, skipping compatibility check.");
        return;
    }

    let Ok(contents) = fs::read_to_string(&split_conf) else {
        return;
    };

    if has_split_conf_compat_markers(&contents) {
        return;
    }

    if patch_split_conf_contents(&contents) != contents {
        warn!(
            "ALSA split.conf compatibility markers are incomplete. GoXLR UCM profile errors may occur. \
The daemon will not modify distro-owned ALSA files automatically; apply the documented split.conf workaround manually if audio setup fails."
        );
    }
}

fn has_split_conf_compat_markers(contents: &str) -> bool {
    contents.contains("${var:-__dev}") && contents.contains("${var:-__Device}")
}

fn patch_split_conf_contents(contents: &str) -> String {
    contents
        .replace("${var:__dev}", "${var:-__dev}")
        .replace("${var:__Device}", "${var:-__Device}")
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}
