use crate::profile::{DEFAULT_MIC_PROFILE_NAME, DEFAULT_PROFILE_NAME};
use anyhow::{Context, Result};
use log::error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct SettingsHandle {
    path: PathBuf,
    settings: Arc<RwLock<Settings>>,
}

impl SettingsHandle {
    pub async fn load(path: PathBuf, data_dir: &Path) -> Result<SettingsHandle> {
        let settings = Settings::read(&path)?.unwrap_or_else(|| Settings {
            profile_directory: data_dir.join("profiles"),
            devices: Default::default(),
        });
        let handle = SettingsHandle {
            path,
            settings: Arc::new(RwLock::new(settings)),
        };
        handle.save().await;
        Ok(handle)
    }

    pub async fn save(&self) {
        let settings = self.settings.write().await;
        if let Err(e) = settings.write(&self.path) {
            error!(
                "Couldn't save settings to {}: {}",
                self.path.to_string_lossy(),
                e
            );
        }
    }

    pub async fn get_profile_directory(&self) -> PathBuf {
        let settings = self.settings.read().await;
        settings.profile_directory.clone()
    }

    pub async fn get_device_profile_name(&self, device_serial: &str) -> Option<String> {
        let settings = self.settings.read().await;
        settings
            .devices
            .get(device_serial)
            .map(|d| d.profile.clone())
    }

    pub async fn get_device_mic_profile_name(&self, device_serial: &str) -> Option<String> {
        let settings = self.settings.read().await;
        settings
            .devices
            .get(device_serial)
            .map(|d| d.mic_profile.clone())
    }

    pub async fn set_device_profile_name(&self, device_serial: &str, profile_name: &str) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .entry(device_serial.to_owned())
            .or_insert_with(|| DeviceSettings::default());
        entry.profile = profile_name.to_owned();
    }

    pub async fn set_device_mic_profile_name(&self, device_serial: &str, mic_profile_name: &str) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .entry(device_serial.to_owned())
            .or_insert_with(|| DeviceSettings::default());
        entry.mic_profile = mic_profile_name.to_owned();
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    profile_directory: PathBuf,
    devices: HashMap<String, DeviceSettings>,
}

impl Settings {
    pub fn read(path: &Path) -> Result<Option<Settings>> {
        match File::open(path) {
            Ok(reader) => Ok(Some(serde_json::from_reader(reader).context(format!(
                "Could not parse daemon settings file at {}",
                path.to_string_lossy()
            ))?)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).context(format!(
                "Could not open daemon settings file for reading at {}",
                path.to_string_lossy()
            )),
        }
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if let Err(e) = create_dir_all(parent) {
                if e.kind() != ErrorKind::AlreadyExists {
                    return Err(e).context(format!(
                        "Could not create settings directory at {}",
                        parent.to_string_lossy()
                    ))?;
                }
            }
        }
        let writer = File::create(path).context(format!(
            "Could not open daemon settings file for writing at {}",
            path.to_string_lossy()
        ))?;
        serde_json::to_writer_pretty(writer, self).context(format!(
            "Could not write to daemon settings file at {}",
            path.to_string_lossy()
        ))?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct DeviceSettings {
    profile: String,
    mic_profile: String,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        DeviceSettings {
            profile: DEFAULT_PROFILE_NAME.to_owned(),
            mic_profile: DEFAULT_MIC_PROFILE_NAME.to_owned(),
        }
    }
}
