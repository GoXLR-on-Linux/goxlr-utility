/*
This is simply a struct that manages and returns a list of files in various directories.

I considered sending this data on-demand, however things like the UI may poll incredibly
frequently, and given the infrequency of changes holding a 1 second cache is useful.

This has been created as a separate mod primarily because profile.rs is big enough, and
secondly because it's managing different types of files
 */

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::fs::{create_dir_all, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, bail, Context, Result};
use futures::channel::mpsc::{channel, Receiver};
use futures::executor::block_on;
use futures::{SinkExt, StreamExt};
use log::{debug, info, warn};

use glob::glob;
use goxlr_ipc::PathTypes;
use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::Sender;

use crate::{SettingsHandle, Shutdown};

// This should probably be handled with an EnumSet..
#[derive(Debug)]
pub struct FilePaths {
    profiles: PathBuf,
    mic_profiles: PathBuf,
    presets: PathBuf,
    icons: PathBuf,
    samples: PathBuf,
}

#[derive(Debug)]
pub struct FileManager {
    paths: FilePaths,
}

impl FileManager {
    pub fn new(settings: &SettingsHandle) -> Self {
        let paths = get_file_paths_from_settings(settings);
        FileManager::create_paths(&paths);

        Self { paths }
    }

    pub fn create_paths(paths: &FilePaths) {
        if !paths.profiles.exists() {
            if let Err(e) = create_path(&paths.profiles) {
                warn!("Unable to Create Path: {:?}, {}", &paths.profiles, e);
            } else if let Err(e) = extract_defaults(PathTypes::Profiles, &paths.profiles) {
                warn!("Unable to Extract Default Profiles: {}", e);
            }
        }

        // Microphone Path..
        if !&paths.mic_profiles.exists() {
            if let Err(e) = create_path(&paths.mic_profiles) {
                warn!("Unable to Create Path: {:?}, {}", &paths.mic_profiles, e);
            } else if let Err(e) = extract_defaults(PathTypes::MicProfiles, &paths.mic_profiles) {
                warn!("Unable to Extract Default Mic Profiles {}", e);
            }
        }

        // Presets Path..
        if !&paths.presets.exists() {
            if let Err(e) = create_path(&paths.presets) {
                warn!("Unable to Create Path: {:?}, {}", &paths.presets, e);
            } else if let Err(e) = extract_defaults(PathTypes::Presets, &paths.presets) {
                warn!("Unable to Extract Default Presets: {}", e);
            }
        }

        // Icons..
        if !&paths.icons.exists() {
            if let Err(e) = create_path(&paths.icons) {
                warn!("Unable to Create Path: {:?}, {}", &paths.icons, e);
            } else if let Err(e) = extract_defaults(PathTypes::Icons, &paths.icons) {
                warn!("Unable to Extract Default Icons: {}", e);
            }
        }

        // This will create the Samples and Samples/Recorded directories
        let recorded_path = &paths.samples.join("Recorded");
        if !recorded_path.exists() {
            if let Err(e) = create_path(recorded_path) {
                warn!("Unable to Create Path: {:?}, {}", recorded_path, e);
            }
        }
    }

    pub fn get_profiles(&mut self) -> Vec<String> {
        let path = self.paths.profiles.clone();
        let extension = ["goxlr"].to_vec();
        self.get_files_from_path(path, extension, false)
    }

    pub fn get_mic_profiles(&mut self) -> Vec<String> {
        let path = self.paths.mic_profiles.clone();
        let extension = ["goxlrMicProfile"].to_vec();

        self.get_files_from_path(path, extension, false)
    }

    pub fn get_presets(&mut self) -> Vec<String> {
        let path = self.paths.presets.clone();
        let extension = ["preset"].to_vec();

        self.get_files_from_path(path, extension, false)
    }

    pub fn get_samples(&mut self) -> BTreeMap<String, String> {
        let base_path = self.paths.samples.clone();
        let extensions = ["wav", "mp3"].to_vec();

        self.get_recursive_file_list(base_path, extensions)
    }

    pub fn get_icons(&mut self) -> Vec<String> {
        let path = self.paths.icons.clone();
        let extension = ["gif", "jpg", "png"].to_vec();

        self.get_files_from_path(path, extension, true)
    }

    fn get_recursive_file_list(
        &self,
        path: PathBuf,
        extensions: Vec<&str>,
    ) -> BTreeMap<String, String> {
        let mut paths: Vec<PathBuf> = Vec::new();

        for extension in extensions {
            let format = format!("{}/**/*.{}", path.to_string_lossy(), extension);
            let files = glob(format.as_str());
            if let Ok(files) = files {
                files.for_each(|f| paths.push(f.unwrap()));
            }
        }

        let mut map: BTreeMap<String, String> = BTreeMap::new();
        // Ok, we need to split stuff up..
        for file_path in paths {
            map.insert(
                file_path.to_string_lossy()[path.to_string_lossy().len() + 1..].to_string(),
                file_path.file_name().unwrap().to_string_lossy().to_string(),
            );
        }
        map
    }

    fn get_files_from_path(
        &self,
        path: PathBuf,
        extensions: Vec<&str>,
        with_extension: bool,
    ) -> Vec<String> {
        let mut result = self.get_files_from_drive(path, extensions.clone(), with_extension);
        result.sort_by_key(|a| a.to_lowercase());
        result.dedup();
        result
    }

    fn get_files_from_drive(
        &self,
        path: PathBuf,
        extensions: Vec<&str>,
        with_extension: bool,
    ) -> Vec<String> {
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
                            return if with_extension {
                                e.path()
                                    .file_name()
                                    .and_then(|n| n.to_str().map(String::from))
                            } else {
                                e.path().file_stem().and_then(
                                    // Convert it to a String..
                                    |n| n.to_str().map(String::from),
                                )
                            };
                        })
                    // Collect the result.
                })
                .collect::<Vec<String>>();
        }

        Vec::new()
    }
}

pub async fn spawn_file_notification_service(
    paths: FilePaths,
    sender: Sender<PathTypes>,
    mut shutdown_signal: Shutdown,
) {
    let watcher = create_watcher();
    if let Err(error) = watcher {
        warn!("Error Creating the File Watcher, aborting: {:?}", error);
        return;
    }

    // Create the worker..
    let (mut watcher, mut rx) = watcher.unwrap();

    // Add the Paths to the Watcher..
    if let Err(error) = watcher.watch(&paths.profiles, RecursiveMode::NonRecursive) {
        warn!("Unable to Monitor Profiles Path: {:?}", error);
    }
    if let Err(error) = watcher.watch(&paths.mic_profiles, RecursiveMode::NonRecursive) {
        warn!("Unable to Monitor the Microphone Profile Path {:?}", error);
    }
    if let Err(error) = watcher.watch(&paths.presets, RecursiveMode::NonRecursive) {
        warn!("Unable to Monitor the Presets Path: {:?}", error)
    }
    if let Err(error) = watcher.watch(&paths.icons, RecursiveMode::NonRecursive) {
        warn!("Unable to monitor the Icons Path: {:?}", error);
    }
    if let Err(error) = watcher.watch(&paths.samples, RecursiveMode::Recursive) {
        warn!("Unable to Monitor the Samples Path: {:?}", error);
    }

    // Wait for any changes..
    loop {
        tokio::select! {
            () = shutdown_signal.recv() => {
                debug!("Shutdown Signal Received.");
                break;
            },
            result = rx.next() => {
                if let Some(result) = result {
                    match result {
                        Ok(event) => {
                            match event.kind {
                                // Triggered on the Creation of a file / folder..
                                EventKind::Create(CreateKind::File) |
                                EventKind::Create(CreateKind::Folder) |
                                EventKind::Create(CreateKind::Any) |

                                // Triggered on the Removal of a File / Folder
                                EventKind::Remove(RemoveKind::File) |
                                EventKind::Remove(RemoveKind::Folder) |
                                EventKind::Remove(RemoveKind::Any) |

                                // Triggered on Rename / Move of a file
                                EventKind::Modify(ModifyKind::Name(RenameMode::From)) |
                                EventKind::Modify(ModifyKind::Name(RenameMode::To)) |
                                EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {

                                    let path = &event.paths[0];
                                    if path.starts_with(&paths.profiles) {
                                        let _ = sender.send(PathTypes::Profiles).await;
                                        continue;
                                    }

                                    if path.starts_with(&paths.mic_profiles) {
                                        let _ = sender.send(PathTypes::MicProfiles).await;
                                        continue;
                                    }

                                    if path.starts_with(&paths.icons) {
                                        let _ = sender.send(PathTypes::Icons).await;
                                        continue;
                                    }

                                    if path.starts_with(&paths.presets) {
                                        let _ = sender.send(PathTypes::Presets).await;
                                        continue;
                                    }

                                    if path.starts_with(&paths.samples) {
                                        let _ = sender.send(PathTypes::Samples).await;
                                        continue;
                                    }
                                },

                                _ => {
                                    // Do nothing, not our kind of event!
                                }
                            }
                        },
                        Err(error) => {
                            warn!("Error Reading File Event: {:?}", error);
                        }
                    }
                }
            }
        }
    }
}

fn create_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
    let (mut tx, rx) = channel(1);

    let watcher = RecommendedWatcher::new(
        move |res| {
            block_on(async {
                tx.send(res).await.unwrap();
            })
        },
        Config::default(),
    )?;

    Ok((watcher, rx))
}

pub fn get_file_paths_from_settings(settings: &SettingsHandle) -> FilePaths {
    FilePaths {
        profiles: block_on(settings.get_profile_directory()),
        mic_profiles: block_on(settings.get_mic_profile_directory()),
        presets: block_on(settings.get_presets_directory()),
        icons: block_on(settings.get_icons_directory()),
        samples: block_on(settings.get_samples_directory()),
    }
}

pub fn find_file_in_path(path: PathBuf, file: PathBuf) -> Option<PathBuf> {
    let format = format!("{}/**/{}", path.to_string_lossy(), file.to_string_lossy());
    let files = glob(format.as_str());
    if let Ok(files) = files {
        if let Some(file) = files.into_iter().next() {
            return Some(file.unwrap());
        }
    }

    None
}

pub fn create_path(path: &Path) -> Result<()> {
    if !path.exists() {
        // Attempt to create the profile directory..
        if let Err(e) = create_dir_all(path) {
            return Err(e).context(format!("Could not create path {}", &path.to_string_lossy()))?;
        } else {
            info!("Created Path: {}", path.to_string_lossy());
        }
    }
    Ok(())
}

pub fn can_create_new_file(path: PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            bail!("Parent Directory doesn't exist");
        }
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

const DEFAULTS_BINARY: &str = "goxlr-defaults";
pub fn extract_defaults(file_type: PathTypes, path: &Path) -> Result<()> {
    let binary_name = if cfg!(target_os = "windows") {
        format!("{DEFAULTS_BINARY}.exe")
    } else {
        String::from(DEFAULTS_BINARY)
    };

    let mut binary_path = None;

    // There are three possible places to check for this, the CWD, the binary WD, and $PATH
    let cwd = std::env::current_dir()?.join(binary_name.clone());
    if cwd.exists() {
        binary_path.replace(cwd);
    }

    if binary_path.is_none() {
        if let Some(parent) = std::env::current_exe()?.parent() {
            let bin = parent.join(binary_name.clone());
            if bin.exists() {
                binary_path.replace(bin);
            }
        }
    }

    let final_bin = if let Some(path) = binary_path {
        path.into_os_string()
    } else {
        OsString::from(binary_name)
    };

    let file_type = match file_type {
        PathTypes::Profiles => "profiles",
        PathTypes::MicProfiles => "mic-profiles",
        PathTypes::Presets => "presets",
        PathTypes::Icons => "icons",
        _ => bail!("Invalid File Type Specified"),
    };

    let command = Command::new(final_bin)
        .arg(file_type)
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();

    match command {
        Ok(output) => {
            if !output.status.success() {
                if let Some(code) = output.status.code() {
                    bail!("Unable to extract defaults, Error Code: {}", code);
                }
            }
        }
        Err(error) => {
            bail!("Unable to run Default extractor: {}", error);
        }
    }
    Ok(())
}
