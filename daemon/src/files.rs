/*
This is simply a struct that manages and returns a list of files in various directories.

I considered sending this data on-demand, however things like the UI may poll incredibly
frequently, and given the infrequency of changes holding a 1 second cache is useful.

This has been created as a separate mod primarily because profile.rs is big enough, and
secondly because it's managing different types of files
 */

use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::{create_dir_all, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use futures::executor::block_on;
use log::{debug, info, warn};

use glob::glob;
use nix::NixPath;

use crate::{SettingsHandle, DISTRIBUTABLE_ROOT};

#[derive(Debug)]
pub struct FileManager {
    profiles: FileList,
    mic_profiles: FileList,
    presets: FileList,
    samples: RecursiveFileList,
}

#[derive(Debug, Clone)]
struct FileList {
    names: HashSet<String>,
    timeout: Instant,
}

#[derive(Debug, Clone)]
struct RecursiveFileList {
    names: HashMap<String, String>,
    timeout: Instant,
}

impl Default for FileList {
    fn default() -> Self {
        Self {
            names: HashSet::new(),
            timeout: Instant::now(),
        }
    }
}

impl Default for RecursiveFileList {
    fn default() -> Self {
        Self {
            names: HashMap::new(),
            timeout: Instant::now(),
        }
    }
}

impl FileManager {
    pub fn new() -> Self {
        Self {
            profiles: Default::default(),
            mic_profiles: Default::default(),
            presets: Default::default(),
            samples: Default::default(),
        }
    }

    pub fn invalidate_caches(&mut self) {
        debug!("Invalidating File Caches..");
        self.profiles = Default::default();
        self.mic_profiles = Default::default();
        self.presets = Default::default();
        self.samples = Default::default();
    }

    pub fn get_profiles(&mut self, settings: &SettingsHandle) -> HashSet<String> {
        // There might be a nicer way to do this, which doesn't result in duplicating
        // code with different members..
        if self.profiles.timeout > Instant::now() {
            return self.profiles.names.clone();
        }

        let path = block_on(settings.get_profile_directory());
        let extension = ["goxlr"].to_vec();

        let distrib_path = Path::new(DISTRIBUTABLE_ROOT).join("profiles/");
        self.profiles = self.get_file_list(vec![distrib_path, path], extension);
        self.profiles.names.clone()
    }

    pub fn get_mic_profiles(&mut self, settings: &SettingsHandle) -> HashSet<String> {
        if self.mic_profiles.timeout > Instant::now() {
            return self.mic_profiles.names.clone();
        }

        let path = block_on(settings.get_mic_profile_directory());
        let extension = ["goxlrMicProfile"].to_vec();

        self.mic_profiles = self.get_file_list(vec![path], extension);
        self.mic_profiles.names.clone()
    }

    pub fn get_presets(&mut self, settings: &SettingsHandle) -> HashSet<String> {
        if self.presets.timeout > Instant::now() {
            return self.presets.names.clone();
        }

        let path = block_on(settings.get_presets_directory());
        let distrib_path = Path::new(DISTRIBUTABLE_ROOT).join("presets/");
        let extension = ["preset"].to_vec();

        self.presets = self.get_file_list(vec![path, distrib_path], extension);
        self.presets.names.clone()
    }

    pub fn get_samples(&mut self, settings: &SettingsHandle) -> HashMap<String, String> {
        if self.samples.timeout > Instant::now() {
            return self.samples.names.clone();
        }

        let base_path = block_on(settings.get_samples_directory());
        let extensions = ["wav", "mp3"].to_vec();

        self.samples.names.clear();

        self.samples = self.get_recursive_file_list(base_path, extensions);
        self.samples.names.clone()
    }

    fn get_recursive_file_list(&self, path: PathBuf, extensions: Vec<&str>) -> RecursiveFileList {
        //let extensions = extensions.join(",");
        let mut paths: Vec<PathBuf> = Vec::new();

        for extension in extensions {
            let format = format!("{}/**/*.{}", path.to_string_lossy(), extension);
            let files = glob(format.as_str());
            if let Ok(files) = files {
                files.for_each(|f| paths.push(f.unwrap()));
            }
        }

        let mut map: HashMap<String, String> = HashMap::new();
        // Ok, we need to split stuff up..
        for file_path in paths {
            map.insert(
                file_path.to_string_lossy()[path.len() + 1..].to_string(),
                file_path.file_name().unwrap().to_string_lossy().to_string(),
            );
        }

        RecursiveFileList {
            names: map,
            timeout: Instant::now(),
        }
    }

    fn get_file_list(&self, path: Vec<PathBuf>, extensions: Vec<&str>) -> FileList {
        // We need to refresh..
        FileList {
            names: self.get_files_from_paths(path, extensions),
            timeout: Instant::now() + Duration::from_secs(5),
        }
    }

    fn get_files_from_paths(&self, paths: Vec<PathBuf>, extensions: Vec<&str>) -> HashSet<String> {
        let mut result = HashSet::new();

        for path in paths {
            result.extend(self.get_files_from_drive(path, extensions.clone()));
        }

        result
    }

    fn get_files_from_drive(&self, path: PathBuf, extensions: Vec<&str>) -> HashSet<String> {
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
                        .filter(|e| {
                            let path = e.path();
                            let os_ext = path.extension().unwrap();
                            for extension in extensions.clone() {
                                if extension == os_ext {
                                    return true;
                                }
                            }
                            false
                        })
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

        if !path.starts_with(Path::new(DISTRIBUTABLE_ROOT)) {
            debug!(
                "Path not found, or unable to read: {:?}",
                path.to_string_lossy()
            );
        }

        HashSet::new()
    }
}

pub fn create_path(path: &Path) -> Result<()> {
    if path.starts_with(Path::new(DISTRIBUTABLE_ROOT)) {
        return Ok(());
    }
    if !path.exists() {
        // Attempt to create the profile directory..
        if let Err(e) = create_dir_all(&path) {
            return Err(e).context(format!("Could not create path {}", &path.to_string_lossy()))?;
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
