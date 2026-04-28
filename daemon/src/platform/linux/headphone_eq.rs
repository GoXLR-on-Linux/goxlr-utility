use crate::settings::HeadphoneEqProfile;
use anyhow::{Context, Result, bail};
use log::warn;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use which::which;

const PRESET_PREFIX: &str = "GoXLR-HeadphoneEQ";

pub fn backend_name() -> &'static str {
    "EasyEffects"
}

pub fn is_backend_available() -> bool {
    which("easyeffects").is_ok()
}

pub fn apply_headphone_eq(
    serial: &str,
    enabled: bool,
    profile: &HeadphoneEqProfile,
) -> Result<()> {
    if !is_backend_available() {
        bail!("EasyEffects backend is not available on this system");
    }

    let preset_name = preset_name_for_serial(serial);
    let preset_path = resolve_preset_path(&preset_name)?;
    let preset = create_preset_json(enabled, profile);

    if let Some(parent) = preset_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Unable to create EasyEffects preset directory {}",
                parent.to_string_lossy()
            )
        })?;
    }

    let payload = serde_json::to_string_pretty(&preset)?;
    fs::write(&preset_path, payload).with_context(|| {
        format!(
            "Unable to write EasyEffects preset {}",
            preset_path.to_string_lossy()
        )
    })?;

    if let Err(first_error) = run_easyeffects(&["-l", &preset_name]) {
        // If loading failed, try to start EasyEffects hidden and retry briefly;
        // DBus/session registration can lag behind the process spawn.
        let _ = Command::new("easyeffects").arg("-w").spawn();

        let mut last_error = None;
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(250));
            match run_easyeffects(&["-l", &preset_name]) {
                Ok(()) => return Ok(()),
                Err(error) => last_error = Some(error),
            }
        }

        if let Some(retry_error) = last_error {
            let retry_message = retry_error.to_string();
            if is_display_error(&retry_message) {
                warn!(
                    "EasyEffects preset '{}' was written but cannot be auto-loaded in this session (no active display).",
                    preset_name
                );
                return Ok(());
            }

            return Err(retry_error).with_context(|| {
                format!(
                    "Unable to auto-load EasyEffects preset '{}' (first attempt error: {})",
                    preset_name, first_error
                )
            });
        }
    }

    Ok(())
}

fn preset_name_for_serial(serial: &str) -> String {
    let safe_serial: String = serial
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    format!("{PRESET_PREFIX}-{safe_serial}")
}

fn resolve_preset_path(preset_name: &str) -> Result<PathBuf> {
    for directory in candidate_preset_dirs() {
        if directory.exists() {
            return Ok(directory.join(format!("{preset_name}.json")));
        }
    }

    let directory = default_preset_dir()?;
    Ok(directory.join(format!("{preset_name}.json")))
}

fn candidate_preset_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = home_dir() {
        dirs.push(home.join(".local/share/easyeffects/output"));
        dirs.push(home.join(".config/easyeffects/output"));
        dirs.push(home.join(".var/app/com.github.wwmm.easyeffects/data/easyeffects/output"));
        dirs.push(home.join(".var/app/com.github.wwmm.easyeffects/config/easyeffects/output"));
    }
    dirs
}

fn default_preset_dir() -> Result<PathBuf> {
    let home = home_dir().context("Unable to resolve HOME for EasyEffects preset path")?;
    Ok(home.join(".local/share/easyeffects/output"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn create_preset_json(enabled: bool, profile: &HeadphoneEqProfile) -> Value {
    let mut left = serde_json::Map::new();
    let mut right = serde_json::Map::new();

    for (idx, band) in profile.bands.iter().enumerate() {
        let key = format!("band{idx}");
        let band_json = json!({
            "frequency": band.frequency_hz,
            "gain": band.gain_db,
            "mode": "RLC (BT)",
            "mute": false,
            "q": band.q,
            "slope": "x1",
            "solo": false,
            "type": "Bell",
            "width": 4.0
        });
        left.insert(key.clone(), band_json.clone());
        right.insert(key, band_json);
    }

    json!({
        "output": {
            "blocklist": [],
            "plugins_order": ["equalizer#0"],
            "equalizer#0": {
                "balance": 0.0,
                "bypass": !enabled,
                "input-gain": profile.preamp_db,
                "left": left,
                "mode": "IIR",
                "num-bands": profile.bands.len(),
                "output-gain": 0.0,
                "pitch-left": 0.0,
                "pitch-right": 0.0,
                "right": right,
                "split-channels": false
            }
        }
    })
}

fn run_easyeffects(args: &[&str]) -> Result<()> {
    let mut child = Command::new("easyeffects")
        .args(args)
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Unable to execute EasyEffects with args {args:?}"))?;

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if child
            .try_wait()
            .with_context(|| format!("Unable to poll EasyEffects with args {args:?}"))?
            .is_some()
        {
            let output = child
                .wait_with_output()
                .with_context(|| format!("Unable to collect EasyEffects output for args {args:?}"))?;
            if output.status.success() {
                return Ok(());
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "EasyEffects command failed (args {args:?}) with status {}: {}",
                output.status,
                stderr.trim()
            );
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().ok();
            let stderr = output
                .as_ref()
                .map(|output| String::from_utf8_lossy(&output.stderr).trim().to_string())
                .unwrap_or_default();
            bail!("EasyEffects command timed out (args {args:?}): {stderr}");
        }

        thread::sleep(Duration::from_millis(50));
    }
}

fn is_display_error(message: &str) -> bool {
    let msg = message.to_ascii_lowercase();
    msg.contains("failed to open display")
        || msg.contains("cannot open display")
        || msg.contains("wayland")
        || msg.contains("xdg_runtime_dir")
        || msg.contains("session bus")
        || msg.contains("dbus")
        || msg.contains("pipewire")
        || msg.contains("pulse")
}
