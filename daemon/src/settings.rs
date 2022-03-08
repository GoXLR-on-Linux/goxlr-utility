use anyhow::{Context, Result};
use log::error;
use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct SettingsHandle {
    path: PathBuf,
    settings: Arc<RwLock<Settings>>,
}

impl SettingsHandle {
    pub async fn load(path: PathBuf) -> Result<SettingsHandle> {
        let settings = Settings::read(&path)?;
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {}

impl Settings {
    pub fn read(path: &Path) -> Result<Settings> {
        match File::open(path) {
            Ok(reader) => serde_json::from_reader(reader).context(format!(
                "Could not parse daemon settings file at {}",
                path.to_string_lossy()
            )),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Settings {}),
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
