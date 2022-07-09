use crate::profile::{DEFAULT_MIC_PROFILE_NAME, DEFAULT_PROFILE_NAME};
use anyhow::{Context, Result};
use directories::ProjectDirs;
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
    pub async fn load(path: PathBuf) -> Result<SettingsHandle> {
        // This is only used for defaults
        let proj_dirs = ProjectDirs::from("org", "GoXLR-on-Linux", "GoXLR-Utility")
            .context("Couldn't find project directories")?;
        let data_dir = proj_dirs.data_dir();

        let mut settings = Settings::read(&path)?.unwrap_or_else(|| Settings {
            profile_directory: Some(data_dir.join("profiles")),
            mic_profile_directory: Some(data_dir.join("mic-profiles")),
            samples_directory: Some(data_dir.join("samples")),
            devices: Default::default(),
        });

        // Set these values if they're missing from the configuration
        if settings.profile_directory.is_none() {
            settings.profile_directory = Some(data_dir.join("profiles"));
        }

        if settings.mic_profile_directory.is_none() {
            settings.mic_profile_directory = Some(data_dir.join("mic-profiles"));
        }

        if settings.samples_directory.is_none() {
            settings.samples_directory = Some(data_dir.join("samples"));
        }

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
        settings.profile_directory.clone().unwrap()
    }

    pub async fn get_mic_profile_directory(&self) -> PathBuf {
        let settings = self.settings.read().await;
        settings.mic_profile_directory.clone().unwrap()
    }

    pub async fn get_samples_directory(&self) -> PathBuf {
        let settings = self.settings.read().await;
        settings.samples_directory.clone().unwrap()
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

    pub async fn get_device_bleep_volume(&self, device_serial: &str) -> Option<i8> {
        let settings = self.settings.read().await;
        settings.devices.get(device_serial).map(|d| d.bleep_volume)
    }

    pub async fn set_device_profile_name(&self, device_serial: &str, profile_name: &str) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.profile = profile_name.to_owned();
    }

    pub async fn set_device_mic_profile_name(&self, device_serial: &str, mic_profile_name: &str) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.mic_profile = mic_profile_name.to_owned();
    }

    pub async fn set_device_bleep_volume(&self, device_serial: &str, bleep_volume: i8) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.bleep_volume = bleep_volume;
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    profile_directory: Option<PathBuf>,
    mic_profile_directory: Option<PathBuf>,
    samples_directory: Option<PathBuf>,
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
#[serde(default)]
struct DeviceSettings {
    profile: String,
    mic_profile: String,
    bleep_volume: i8,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        DeviceSettings {
            profile: DEFAULT_PROFILE_NAME.to_owned(),
            mic_profile: DEFAULT_MIC_PROFILE_NAME.to_owned(),
            bleep_volume: -20,
        }
    }
}
