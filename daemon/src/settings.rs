use crate::mic_profile::DEFAULT_MIC_PROFILE_NAME;
use crate::profile::DEFAULT_PROFILE_NAME;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use goxlr_ipc::{GoXLRCommand, LogLevel};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::fs::{create_dir_all, File};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct SettingsHandle {
    path: PathBuf,
    data_dir: PathBuf,
    settings: Arc<RwLock<Settings>>,
}

enum Paths {
    Profiles,
    MicProfiles,
    Samples,
    Presets,
    Icons,
    Logs,
}

impl AsRef<Path> for Paths {
    fn as_ref(&self) -> &Path {
        match self {
            Paths::Profiles => Path::new("profiles"),
            Paths::MicProfiles => Path::new("mic-profiles"),
            Paths::Samples => Path::new("samples"),
            Paths::Presets => Path::new("presets"),
            Paths::Icons => Path::new("icons"),
            Paths::Logs => Path::new("logs"),
        }
    }
}

impl SettingsHandle {
    pub async fn load(path: PathBuf) -> Result<SettingsHandle> {
        // This is only used for defaults
        let proj_dirs = ProjectDirs::from("org", "GoXLR-on-Linux", "GoXLR-Utility")
            .context("Couldn't find project directories")?;
        let data_dir = proj_dirs.data_dir();

        let mut settings = Settings::read(&path)?.unwrap_or_else(|| Settings {
            show_tray_icon: Some(true),
            selected_locale: None,
            tts_enabled: Some(false),
            allow_network_access: Some(false),
            profile_directory: None,
            mic_profile_directory: None,
            samples_directory: None,
            presets_directory: None,
            icons_directory: None,
            logs_directory: None,
            log_level: Some(LogLevel::Debug),
            open_ui_on_launch: None,
            activate: None,
            devices: Some(Default::default()),
            sample_gain: Some(Default::default()),
        });

        // Forward compatibility, if the configured path is the same as the default path
        // remove the configured path (all path lookups will return the default if not set)
        if let Some(profiles) = &settings.profile_directory {
            if profiles == &data_dir.join(Paths::Profiles) {
                info!("Clearing 'Default' Profiles Directory configuration..");
                settings.profile_directory = None;
            }
        }

        if let Some(ref mic_profiles) = settings.mic_profile_directory {
            if mic_profiles == &data_dir.join(Paths::MicProfiles) {
                info!("Clearing 'Default' Mic Profiles Directory configuration..");
                settings.mic_profile_directory = None;
            }
        }

        if let Some(ref samples) = settings.samples_directory {
            if samples == &data_dir.join(Paths::Samples) {
                info!("Clearing 'Default' Samples Directory configuration..");
                settings.samples_directory = None;
            }
        }

        if let Some(ref presets) = settings.presets_directory {
            if presets == &data_dir.join(Paths::Presets) {
                info!("Clearing 'Default' Presets Directory configuration..");
                settings.presets_directory = None;
            }
        }

        if let Some(ref icons) = settings.icons_directory {
            if icons == &data_dir.join(Paths::Icons) {
                info!("Clearing 'Default' Icon Directory configuration..");
                settings.icons_directory = None;
            }
        }

        if let Some(ref logs) = settings.logs_directory {
            if logs == &data_dir.join(Paths::Logs) {
                info!("Clearing 'Default' Logs Directory configuration..");
                settings.logs_directory = None;
            }
        }

        if settings.log_level.is_none() {
            settings.log_level = Some(LogLevel::Debug);
        }

        if settings.open_ui_on_launch.is_none() {
            settings.open_ui_on_launch = Some(false);
        }

        if settings.show_tray_icon.is_none() {
            settings.show_tray_icon = Some(true);
        }

        if settings.tts_enabled.is_none() {
            settings.tts_enabled = Some(false);
        }

        if settings.allow_network_access.is_none() {
            settings.allow_network_access = Some(false);
        }

        if settings.devices.is_none() {
            settings.devices = Some(Default::default());
        }

        let handle = SettingsHandle {
            path,
            data_dir: data_dir.to_path_buf(),
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

    fn get_default_path(&self, suffix: Paths) -> PathBuf {
        self.data_dir.join(suffix)
    }

    pub async fn get_show_tray_icon(&self) -> bool {
        let settings = self.settings.read().await;
        settings.show_tray_icon.unwrap()
    }

    pub async fn set_show_tray_icon(&self, enabled: bool) {
        let mut settings = self.settings.write().await;
        settings.show_tray_icon = Some(enabled);
    }

    pub async fn get_selected_locale(&self) -> Option<String> {
        let settings = self.settings.read().await;
        settings.selected_locale.clone()
    }

    pub async fn set_selected_locale(&self, locale: Option<String>) {
        let mut settings = self.settings.write().await;
        settings.selected_locale = locale;
    }

    pub async fn get_tts_enabled(&self) -> Option<bool> {
        // If the TTS feature isn't compiled in, we shouldn't return a value here..
        #[cfg(feature = "tts")]
        {
            let settings = self.settings.read().await;
            return Some(settings.tts_enabled.unwrap());
        }

        // Because whether we get here is defined by a feature, clippy can't be completely
        // objective on the matter, so we allow the behaviour.
        #[allow(unreachable_code)]
        None
    }

    pub async fn set_tts_enabled(&self, enabled: bool) {
        let mut settings = self.settings.write().await;
        settings.tts_enabled = Some(enabled);
    }

    pub async fn get_allow_network_access(&self) -> bool {
        let settings = self.settings.read().await;
        settings.allow_network_access.unwrap()
    }

    pub async fn set_allow_network_access(&self, enabled: bool) {
        let mut settings = self.settings.write().await;
        settings.allow_network_access = Some(enabled);
    }

    pub async fn get_profile_directory(&self) -> PathBuf {
        let settings = self.settings.read().await;
        if let Some(directory) = settings.profile_directory.clone() {
            directory
        } else {
            self.get_default_path(Paths::Profiles)
        }
    }

    pub async fn get_mic_profile_directory(&self) -> PathBuf {
        let settings = self.settings.read().await;
        if let Some(directory) = settings.mic_profile_directory.clone() {
            directory
        } else {
            self.get_default_path(Paths::MicProfiles)
        }
    }

    pub async fn get_samples_directory(&self) -> PathBuf {
        let settings = self.settings.read().await;
        if let Some(directory) = settings.samples_directory.clone() {
            directory
        } else {
            self.get_default_path(Paths::Samples)
        }
    }

    pub async fn get_presets_directory(&self) -> PathBuf {
        let settings = self.settings.read().await;
        if let Some(directory) = settings.presets_directory.clone() {
            directory
        } else {
            self.get_default_path(Paths::Presets)
        }
    }

    pub async fn get_icons_directory(&self) -> PathBuf {
        let settings = self.settings.read().await;
        if let Some(directory) = settings.icons_directory.clone() {
            directory
        } else {
            self.get_default_path(Paths::Icons)
        }
    }

    pub async fn get_log_directory(&self) -> PathBuf {
        let settings = self.settings.read().await;
        if let Some(directory) = settings.logs_directory.clone() {
            directory
        } else {
            self.get_default_path(Paths::Logs)
        }
    }

    pub async fn set_log_level(&self, level: LogLevel) {
        let mut settings = self.settings.write().await;
        settings.log_level = Some(level);
    }

    pub async fn get_log_level(&self) -> LogLevel {
        let settings = self.settings.read().await;
        settings.log_level.clone().unwrap_or(LogLevel::Info)
    }

    pub async fn get_open_ui_on_launch(&self) -> bool {
        let settings = self.settings.read().await;
        settings.open_ui_on_launch.unwrap_or(false)
    }
    pub async fn set_open_ui_on_launch(&self, enable: bool) {
        let mut settings = self.settings.write().await;
        settings.open_ui_on_launch = Some(enable);
    }

    pub async fn get_activate(&self) -> Option<String> {
        let settings = self.settings.read().await;
        settings.activate.clone()
    }

    #[allow(dead_code)]
    pub async fn set_activate(&self, activate: Option<String>) {
        let mut settings = self.settings.write().await;
        settings.activate = activate;
    }

    pub async fn get_device_profile_name(&self, device_serial: &str) -> Option<String> {
        let settings = self.settings.read().await;
        settings
            .devices
            .as_ref()
            .unwrap()
            .get(device_serial)
            .map(|d| d.profile.clone())
    }

    pub async fn get_device_mic_profile_name(&self, device_serial: &str) -> Option<String> {
        let settings = self.settings.read().await;
        settings
            .devices
            .as_ref()
            .unwrap()
            .get(device_serial)
            .map(|d| d.mic_profile.clone())
    }

    pub async fn get_device_shutdown_commands(&self, device_serial: &str) -> Vec<GoXLRCommand> {
        let settings = self.settings.read().await;
        let value = settings
            .devices
            .as_ref()
            .unwrap()
            .get(device_serial)
            .map(|d| d.shutdown_commands.clone());

        if let Some(value) = value {
            return value;
        }
        vec![]
    }

    pub async fn get_device_sleep_commands(&self, device_serial: &str) -> Vec<GoXLRCommand> {
        let settings = self.settings.read().await;
        let value = settings
            .devices
            .as_ref()
            .unwrap()
            .get(device_serial)
            .map(|d| d.sleep_commands.clone());

        if let Some(value) = value {
            return value;
        }
        vec![]
    }

    pub async fn get_device_wake_commands(&self, device_serial: &str) -> Vec<GoXLRCommand> {
        let settings = self.settings.read().await;
        let value = settings
            .devices
            .as_ref()
            .unwrap()
            .get(device_serial)
            .map(|d| d.wake_commands.clone());

        if let Some(value) = value {
            return value;
        }
        vec![]
    }

    pub async fn get_device_sampler_pre_buffer(&self, device_serial: &str) -> u16 {
        let settings = self.settings.read().await;
        let value = settings
            .devices
            .as_ref()
            .unwrap()
            .get(device_serial)
            .map(|d| d.sampler_pre_buffer.unwrap_or(0));
        if let Some(value) = value {
            return value;
        }
        0
    }

    pub async fn get_device_hold_time(&self, device_serial: &str) -> u16 {
        let settings = self.settings.read().await;
        let value = settings
            .devices
            .as_ref()
            .unwrap()
            .get(device_serial)
            .map(|d| d.hold_delay.unwrap_or(500));

        if let Some(value) = value {
            return value;
        }
        500
    }

    // I absolutely hate this naming.. O_O
    pub async fn get_device_chat_mute_mutes_mic_to_chat(&self, device_serial: &str) -> bool {
        let settings = self.settings.read().await;
        let value = settings
            .devices
            .as_ref()
            .unwrap()
            .get(device_serial)
            .map(|d| d.chat_mute_mutes_mic_to_chat.unwrap_or(true));

        if let Some(value) = value {
            return value;
        }
        true
    }

    pub async fn get_device_lock_faders(&self, device_serial: &str) -> bool {
        let settings = self.settings.read().await;
        let value = settings
            .devices
            .as_ref()
            .unwrap()
            .get(device_serial)
            .map(|d| d.lock_faders.unwrap_or(true));

        if let Some(value) = value {
            return value;
        }
        true
    }

    pub async fn get_enable_monitor_with_fx(&self, device_serial: &str) -> bool {
        let settings = self.settings.read().await;
        let value = settings
            .devices
            .as_ref()
            .unwrap()
            .get(device_serial)
            .map(|d| d.enable_monitor_with_fx.unwrap_or(false));
        if let Some(value) = value {
            return value;
        }
        false
    }

    pub async fn get_sampler_reset_on_clear(&self, device_serial: &str) -> bool {
        let settings = self.settings.read().await;
        settings
            .devices
            .as_ref()
            .unwrap()
            .get(device_serial)
            .map(|d| d.sampler_reset_on_clear.unwrap_or(true))
            .unwrap_or(true)
    }

    pub async fn get_sample_gain_percent(&self, name: String) -> u8 {
        let settings = self.settings.read().await;
        if let Some(gain) = &settings.sample_gain {
            if let Some(percent) = gain.get(&*name) {
                return *percent;
            }
            return 100;
        }
        100
    }

    /// This exists so we don't have to repeatedly lock / unlock the struct to get individual
    /// gain values. We can simply clone off the list, and let it be handled elsewhere.
    pub async fn get_sample_gain_list(&self) -> HashMap<String, u8> {
        let settings = self.settings.read().await;
        if let Some(gain) = &settings.sample_gain {
            return gain.clone();
        }
        HashMap::default()
    }

    pub async fn set_device_profile_name(&self, device_serial: &str, profile_name: &str) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .as_mut()
            .unwrap()
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.profile = profile_name.to_owned();
    }

    pub async fn set_device_mic_profile_name(&self, device_serial: &str, mic_profile_name: &str) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .as_mut()
            .unwrap()
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.mic_profile = mic_profile_name.to_owned();
    }

    pub async fn set_device_shutdown_commands(
        &self,
        device_serial: &str,
        commands: Vec<GoXLRCommand>,
    ) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .as_mut()
            .unwrap()
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.shutdown_commands = commands.to_owned();
    }

    pub async fn set_device_sleep_commands(
        &self,
        device_serial: &str,
        commands: Vec<GoXLRCommand>,
    ) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .as_mut()
            .unwrap()
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.sleep_commands = commands.to_owned();
    }

    pub async fn set_device_wake_commands(&self, device_serial: &str, commands: Vec<GoXLRCommand>) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .as_mut()
            .unwrap()
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.wake_commands = commands.to_owned();
    }

    pub async fn set_device_sampler_pre_buffer(&self, device_serial: &str, duration: u16) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .as_mut()
            .unwrap()
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.sampler_pre_buffer = Some(duration);
    }

    pub async fn set_device_mute_hold_duration(&self, device_serial: &str, duration: u16) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .as_mut()
            .unwrap()
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.hold_delay = Some(duration);
    }

    pub async fn set_device_vc_mute_also_mute_cm(&self, device_serial: &str, setting: bool) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .as_mut()
            .unwrap()
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.chat_mute_mutes_mic_to_chat = Some(setting);
    }

    pub async fn set_device_lock_faders(&self, device_serial: &str, setting: bool) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .as_mut()
            .unwrap()
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.lock_faders = Some(setting);
    }

    pub async fn set_enable_monitor_with_fx(&self, device_serial: &str, setting: bool) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .as_mut()
            .unwrap()
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.enable_monitor_with_fx = Some(setting);
    }

    pub async fn set_sampler_reset_on_clear(&self, device_serial: &str, setting: bool) {
        let mut settings = self.settings.write().await;
        let entry = settings
            .devices
            .as_mut()
            .unwrap()
            .entry(device_serial.to_owned())
            .or_insert_with(DeviceSettings::default);
        entry.sampler_reset_on_clear = Some(setting);
    }

    pub async fn set_sample_gain_percent(&self, name: String, value: u8) {
        let mut settings = self.settings.write().await;
        if settings.sample_gain.is_none() {
            settings.sample_gain.replace(HashMap::default());
        }

        let entry = settings.sample_gain.as_mut().unwrap().entry(name);
        entry.and_modify(|v| *v = value).or_insert(value);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    show_tray_icon: Option<bool>,
    selected_locale: Option<String>,
    tts_enabled: Option<bool>,
    allow_network_access: Option<bool>,
    profile_directory: Option<PathBuf>,
    mic_profile_directory: Option<PathBuf>,
    samples_directory: Option<PathBuf>,
    presets_directory: Option<PathBuf>,
    icons_directory: Option<PathBuf>,
    logs_directory: Option<PathBuf>,
    log_level: Option<LogLevel>,
    open_ui_on_launch: Option<bool>,
    activate: Option<String>,
    devices: Option<HashMap<String, DeviceSettings>>,
    sample_gain: Option<HashMap<String, u8>>,
}

impl Settings {
    pub fn read(path: &Path) -> Result<Option<Settings>> {
        match File::open(path) {
            Ok(reader) => {
                let settings = serde_json::from_reader(reader);

                match settings {
                    Ok(settings) => Ok(Some(settings)),
                    Err(_) => {
                        // Something's gone wrong loading the settings, rather than immediately
                        // exiting, we'll try to backup the original file, and reload the defaults.
                        let mut backup = PathBuf::from(path);
                        backup.set_extension(".failed");

                        let copy_result = fs::copy(path, backup);
                        println!("{:?}", copy_result);

                        println!("Error Loading configuration, loading defaults.");
                        Ok(None)
                    }
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).context(format!(
                "Could not open daemon settings file for reading at {}",
                path.to_string_lossy()
            )),
        }
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        debug!("Saving Settings");
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

        let temp_file = tempfile::NamedTempFile::new()?;

        debug!("Writing Config to Temporary File: {:?}", temp_file.path());
        serde_json::to_writer_pretty(temp_file.as_file(), self)?;

        // Sync the file written to disk..
        debug!("Syncing Disk..");
        temp_file.as_file().sync_all()?;

        debug!("Write Complete, saving to {:?}", path);
        fs::copy(temp_file.path(), path)?;

        debug!("Removing Temporary File..");
        fs::remove_file(temp_file.path())?;

        debug!("Settings Saved.");
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct DeviceSettings {
    profile: String,
    mic_profile: String,

    hold_delay: Option<u16>,
    sampler_pre_buffer: Option<u16>,

    // 'Voice Chat Mute All Also Mutes Mic to Chat Mic' O_O
    chat_mute_mutes_mic_to_chat: Option<bool>,

    // Disables the Movement of the Faders when Muting to All (full device only)
    lock_faders: Option<bool>,

    // Enable Monitoring when FX are Enabled
    enable_monitor_with_fx: Option<bool>,

    // Clear Sample Settings when Clearing Button
    sampler_reset_on_clear: Option<bool>,

    // 'Shutdown' commands..
    shutdown_commands: Vec<GoXLRCommand>,
    sleep_commands: Vec<GoXLRCommand>,
    wake_commands: Vec<GoXLRCommand>,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        DeviceSettings {
            profile: DEFAULT_PROFILE_NAME.to_owned(),
            mic_profile: DEFAULT_MIC_PROFILE_NAME.to_owned(),

            hold_delay: Some(500),
            sampler_pre_buffer: None,
            chat_mute_mutes_mic_to_chat: Some(true),
            lock_faders: Some(false),
            enable_monitor_with_fx: Some(false),
            sampler_reset_on_clear: Some(true),

            shutdown_commands: vec![],
            sleep_commands: vec![],
            wake_commands: vec![],
        }
    }
}
