/*
This is simply a struct that manages and returns a list of files in various directories.

I considered sending this data on-demand, however things like the UI may poll incredibly
frequently, and given the infrequency of changes holding a 1 second cache is useful.

This has been created as a separate mod primarily because profile.rs is big enough, and
secondly because it's managing different types of files
 */

use std::collections::HashSet;
use std::fs;
use std::fs::{create_dir_all, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use futures::executor::block_on;
use log::{debug, info, warn};

use crate::{SettingsHandle, DISTRIBUTABLE_PROFILES};

#[derive(Debug)]
pub struct FileManager {
    profiles: FileList,
    mic_profiles: FileList,
}

#[derive(Debug, Clone)]
struct FileList {
    names: HashSet<String>,
    timeout: Instant,
}

impl Default for FileList {
    fn default() -> Self {
        Self {
            timeout: Instant::now(),
            names: HashSet::new(),
        }
    }
}

impl FileManager {
    pub fn new() -> Self {
        Self {
            profiles: Default::default(),
            mic_profiles: Default::default(),
        }
    }

    pub fn invalidate_caches(&mut self) {
        debug!("Invalidating File Caches..");
        self.profiles = Default::default();
        self.mic_profiles = Default::default();
    }

    pub fn get_profiles(&mut self, settings: &SettingsHandle) -> HashSet<String> {
        // There might be a nicer way to do this, which doesn't result in duplicating
        // code with different members..
        if self.profiles.timeout > Instant::now() {
            return self.profiles.names.clone();
        }

        let path = block_on(settings.get_profile_directory());
        let extension = "goxlr";

        let distrib_path = Path::new(DISTRIBUTABLE_PROFILES);
        self.profiles = self.get_file_list(vec![distrib_path.to_path_buf(), path], extension);
        self.profiles.names.clone()
    }

    pub fn get_mic_profiles(&mut self, settings: &SettingsHandle) -> HashSet<String> {
        if self.mic_profiles.timeout > Instant::now() {
            return self.mic_profiles.names.clone();
        }

        let path = block_on(settings.get_mic_profile_directory());
        let extension = "goxlrMicProfile";

        self.mic_profiles = self.get_file_list(vec![path], extension);
        self.mic_profiles.names.clone()
    }

    fn get_file_list(&self, path: Vec<PathBuf>, extension: &str) -> FileList {
        // We need to refresh..
        FileList {
            names: self.get_files_from_paths(path, extension),
            timeout: Instant::now() + Duration::from_secs(5),
        }
    }

    fn get_files_from_paths(&self, paths: Vec<PathBuf>, extension: &str) -> HashSet<String> {
        let mut result = HashSet::new();

        for path in paths {
            result.extend(self.get_files_from_drive(path, extension));
        }

        result
    }

    fn get_files_from_drive(&self, path: PathBuf, extension: &str) -> HashSet<String> {
        if let Err(error) = create_path(&path) {
            warn!(
                "Unable to create path: {}: {}",
                &path.to_string_lossy(),
                error
            );
        }

        if let Ok(list) = path.read_dir() {
            return list
                .filter_map(|entry| {
                    entry
                        .ok()
                        // Make sure this has an extension..
                        .filter(|e| e.path().extension().is_some())
                        // Is it the extension we're looking for?
                        .filter(|e| e.path().extension().unwrap() == extension)
                        // Get the File Name..
                        .and_then(|e| {
                            e.path().file_stem().and_then(
                                // Convert it to a String..
                                |n| n.to_str().map(String::from),
                            )
                        })
                    // Collect the result.
                })
                .collect::<HashSet<String>>();
        }

        debug!(
            "Path not found, or unable to read: {:?}",
            path.to_string_lossy()
        );
        HashSet::new()
    }
}

pub fn create_path(path: &Path) -> Result<()> {
    if !path.exists() {
        // Attempt to create the profile directory..
        if let Err(e) = create_dir_all(&path) {
            return Err(e).context(format!(
                "Could not create profile directory at {}",
                &path.to_string_lossy()
            ))?;
        } else {
            info!("Created Path: {}", path.to_string_lossy());
        }
    }
    Ok(())
}

pub fn can_create_new_file(path: PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_path(parent)?;
    }

    if path.exists() {
        return Err(anyhow!("File already exists."));
    }

    // Attempt to create a file in the path, throw an error if fails..
    File::create(&path)?;

    // Remove the file again.
    fs::remove_file(&path)?;

    Ok(())
}
