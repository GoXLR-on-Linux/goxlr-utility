use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use goxlr_ipc::client::Client;
use goxlr_ipc::clients::ipc::ipc_client::IPCClient;
use goxlr_ipc::clients::ipc::ipc_socket::Socket;
use goxlr_ipc::{DaemonRequest, DaemonResponse, DaemonStatus, GoXLRCommand, ipc_socket_path};
use goxlr_types::{
    ChannelName, CompressorAttackTime, CompressorRatio, CompressorReleaseTime, DeviceType,
    EchoStyle, EffectBankPresets, GateTimes, GenderStyle, HardTuneStyle, MegaphoneStyle,
    MicrophoneType, PitchStyle, ReverbStyle, RobotStyle,
};
use interprocess::local_socket::tokio::prelude::LocalSocketStream;
use interprocess::local_socket::traits::tokio::Stream;
use interprocess::local_socket::{GenericFilePath, GenericNamespaced, ToFsName, ToNsName};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlledChannel {
    pub label: &'static str,
    pub channel: ChannelName,
}

impl ControlledChannel {
    pub fn mvp_channels() -> Vec<Self> {
        vec![
            Self {
                label: "Headphones",
                channel: ChannelName::Headphones,
            },
            Self {
                label: "Music",
                channel: ChannelName::Music,
            },
            Self {
                label: "Game",
                channel: ChannelName::Game,
            },
            Self {
                label: "Chat",
                channel: ChannelName::Chat,
            },
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardCopy;

impl DashboardCopy {
    pub fn mixer_tab() -> &'static str {
        "Mixer"
    }

    pub fn configuration_tab() -> &'static str {
        "Config / Routing"
    }

    pub fn active_playback_heading() -> &'static str {
        "ACTIVE APPS / ROUTING"
    }

    pub fn manual_route_label() -> &'static str {
        "Move now:"
    }

    pub fn persistent_route_label() -> &'static str {
        "Always route:"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalAudioTool {
    Pavucontrol,
    Qpwgraph,
}

impl ExternalAudioTool {
    pub fn daily_helpers() -> Vec<Self> {
        vec![Self::Pavucontrol, Self::Qpwgraph]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pavucontrol => "Open pavucontrol",
            Self::Qpwgraph => "Open qpwgraph",
        }
    }

    pub fn command(self) -> &'static str {
        match self {
            Self::Pavucontrol => "pavucontrol",
            Self::Qpwgraph => "qpwgraph",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PersonalCommand {
    SetVolume(ChannelName, u8),
    SetMicrophoneType(MicrophoneType),
    SetMicrophoneGain(MicrophoneType, u16),
    SetGateActive(bool),
    SetGateThreshold(i8),
    SetGateAttenuation(u8),
    SetGateAttack(GateTimes),
    SetGateRelease(GateTimes),
    SetCompressorThreshold(i8),
    SetCompressorRatio(CompressorRatio),
    SetCompressorAttack(CompressorAttackTime),
    SetCompressorReleaseTime(CompressorReleaseTime),
    SetCompressorMakeupGain(i8),
    SetDeesser(u8),
    SetClipGuardEnabled(bool),
    SetClipGuardThreshold(u8),
    SetHeadphoneLimiterEnabled(bool),
    SetHeadphoneLimiterThreshold(u8),
    SetHeadphoneEqEnabled(bool),
    LoadHeadphoneEqProfile(String),
    SetActiveEffectPreset(EffectBankPresets),
    SetFXEnabled(bool),
    SetReverbStyle(ReverbStyle),
    SetReverbAmount(u8),
    SetEchoStyle(EchoStyle),
    SetEchoAmount(u8),
    SetPitchStyle(PitchStyle),
    SetPitchAmount(i8),
    SetGenderStyle(GenderStyle),
    SetGenderAmount(i8),
    SetMegaphoneEnabled(bool),
    SetMegaphoneStyle(MegaphoneStyle),
    SetMegaphoneAmount(u8),
    SetRobotEnabled(bool),
    SetRobotStyle(RobotStyle),
    SetHardTuneEnabled(bool),
    SetHardTuneStyle(HardTuneStyle),
    SaveMicProfile,
    ReloadSettings,
}

impl From<PersonalCommand> for GoXLRCommand {
    fn from(value: PersonalCommand) -> Self {
        match value {
            PersonalCommand::SetVolume(channel, volume) => GoXLRCommand::SetVolume(channel, volume),
            PersonalCommand::SetMicrophoneType(mic_type) => {
                GoXLRCommand::SetMicrophoneType(mic_type)
            }
            PersonalCommand::SetMicrophoneGain(mic_type, gain) => {
                GoXLRCommand::SetMicrophoneGain(mic_type, gain)
            }
            PersonalCommand::SetGateActive(enabled) => GoXLRCommand::SetGateActive(enabled),
            PersonalCommand::SetGateThreshold(threshold) => {
                GoXLRCommand::SetGateThreshold(threshold)
            }
            PersonalCommand::SetGateAttenuation(attenuation) => {
                GoXLRCommand::SetGateAttenuation(attenuation)
            }
            PersonalCommand::SetGateAttack(attack) => GoXLRCommand::SetGateAttack(attack),
            PersonalCommand::SetGateRelease(release) => GoXLRCommand::SetGateRelease(release),
            PersonalCommand::SetCompressorThreshold(threshold) => {
                GoXLRCommand::SetCompressorThreshold(threshold)
            }
            PersonalCommand::SetCompressorRatio(ratio) => GoXLRCommand::SetCompressorRatio(ratio),
            PersonalCommand::SetCompressorAttack(attack) => {
                GoXLRCommand::SetCompressorAttack(attack)
            }
            PersonalCommand::SetCompressorReleaseTime(release) => {
                GoXLRCommand::SetCompressorReleaseTime(release)
            }
            PersonalCommand::SetCompressorMakeupGain(gain) => {
                GoXLRCommand::SetCompressorMakeupGain(gain)
            }
            PersonalCommand::SetDeesser(deesser) => GoXLRCommand::SetDeeser(deesser),
            PersonalCommand::SetClipGuardEnabled(enabled) => {
                GoXLRCommand::SetClipGuardEnabled(enabled)
            }
            PersonalCommand::SetClipGuardThreshold(threshold) => {
                GoXLRCommand::SetClipGuardThreshold(threshold)
            }
            PersonalCommand::SetHeadphoneLimiterEnabled(enabled) => {
                GoXLRCommand::SetHeadphoneLimiterEnabled(enabled)
            }
            PersonalCommand::SetHeadphoneLimiterThreshold(threshold) => {
                GoXLRCommand::SetHeadphoneLimiterThreshold(threshold)
            }
            PersonalCommand::SetHeadphoneEqEnabled(enabled) => {
                GoXLRCommand::SetHeadphoneEqEnabled(enabled)
            }
            PersonalCommand::LoadHeadphoneEqProfile(profile) => {
                GoXLRCommand::LoadHeadphoneEqProfile(profile)
            }
            PersonalCommand::SetActiveEffectPreset(preset) => {
                GoXLRCommand::SetActiveEffectPreset(preset)
            }
            PersonalCommand::SetFXEnabled(enabled) => GoXLRCommand::SetFXEnabled(enabled),
            PersonalCommand::SetReverbStyle(style) => GoXLRCommand::SetReverbStyle(style),
            PersonalCommand::SetReverbAmount(amount) => GoXLRCommand::SetReverbAmount(amount),
            PersonalCommand::SetEchoStyle(style) => GoXLRCommand::SetEchoStyle(style),
            PersonalCommand::SetEchoAmount(amount) => GoXLRCommand::SetEchoAmount(amount),
            PersonalCommand::SetPitchStyle(style) => GoXLRCommand::SetPitchStyle(style),
            PersonalCommand::SetPitchAmount(amount) => GoXLRCommand::SetPitchAmount(amount),
            PersonalCommand::SetGenderStyle(style) => GoXLRCommand::SetGenderStyle(style),
            PersonalCommand::SetGenderAmount(amount) => GoXLRCommand::SetGenderAmount(amount),
            PersonalCommand::SetMegaphoneEnabled(enabled) => {
                GoXLRCommand::SetMegaphoneEnabled(enabled)
            }
            PersonalCommand::SetMegaphoneStyle(style) => GoXLRCommand::SetMegaphoneStyle(style),
            PersonalCommand::SetMegaphoneAmount(amount) => GoXLRCommand::SetMegaphoneAmount(amount),
            PersonalCommand::SetRobotEnabled(enabled) => GoXLRCommand::SetRobotEnabled(enabled),
            PersonalCommand::SetRobotStyle(style) => GoXLRCommand::SetRobotStyle(style),
            PersonalCommand::SetHardTuneEnabled(enabled) => {
                GoXLRCommand::SetHardTuneEnabled(enabled)
            }
            PersonalCommand::SetHardTuneStyle(style) => GoXLRCommand::SetHardTuneStyle(style),
            PersonalCommand::SaveMicProfile => GoXLRCommand::SaveMicProfile(),
            PersonalCommand::ReloadSettings => GoXLRCommand::ReloadSettings(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectsQuickPreset {
    name: &'static str,
    description: &'static str,
    commands: Vec<PersonalCommand>,
}

impl EffectsQuickPreset {
    pub fn new(
        name: &'static str,
        description: &'static str,
        commands: Vec<PersonalCommand>,
    ) -> Self {
        Self {
            name,
            description,
            commands,
        }
    }

    pub fn daily_presets() -> Vec<Self> {
        vec![
            Self::new(
                "FX Off",
                "Disable voice effects for normal calls and recording.",
                vec![
                    PersonalCommand::SetFXEnabled(false),
                    PersonalCommand::SetMegaphoneEnabled(false),
                    PersonalCommand::SetRobotEnabled(false),
                    PersonalCommand::SetHardTuneEnabled(false),
                ],
            ),
            Self::new(
                "Clean Reverb",
                "Light plate reverb without echo for a subtle broadcast sound.",
                vec![
                    PersonalCommand::SetActiveEffectPreset(EffectBankPresets::Preset1),
                    PersonalCommand::SetFXEnabled(true),
                    PersonalCommand::SetReverbStyle(ReverbStyle::RealPlate),
                    PersonalCommand::SetReverbAmount(28),
                    PersonalCommand::SetEchoStyle(EchoStyle::Quarter),
                    PersonalCommand::SetEchoAmount(0),
                ],
            ),
            Self::new(
                "Robot Fun",
                "Turn on robot voice while keeping megaphone and hard tune off.",
                vec![
                    PersonalCommand::SetActiveEffectPreset(EffectBankPresets::Preset2),
                    PersonalCommand::SetFXEnabled(true),
                    PersonalCommand::SetRobotEnabled(true),
                    PersonalCommand::SetRobotStyle(RobotStyle::Robot1),
                    PersonalCommand::SetMegaphoneEnabled(false),
                    PersonalCommand::SetHardTuneEnabled(false),
                ],
            ),
            Self::new(
                "Hard Tune",
                "Enable hard tune with neutral pitch/gender/megaphone shaping.",
                vec![
                    PersonalCommand::SetActiveEffectPreset(EffectBankPresets::Preset3),
                    PersonalCommand::SetFXEnabled(true),
                    PersonalCommand::SetHardTuneEnabled(true),
                    PersonalCommand::SetHardTuneStyle(HardTuneStyle::Medium),
                    PersonalCommand::SetPitchStyle(PitchStyle::Narrow),
                    PersonalCommand::SetPitchAmount(0),
                    PersonalCommand::SetGenderStyle(GenderStyle::Medium),
                    PersonalCommand::SetGenderAmount(0),
                    PersonalCommand::SetMegaphoneStyle(MegaphoneStyle::Megaphone),
                    PersonalCommand::SetMegaphoneAmount(0),
                ],
            ),
        ]
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn description(&self) -> &'static str {
        self.description
    }

    pub fn commands(&self) -> Vec<PersonalCommand> {
        self.commands.clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiScene {
    name: String,
    commands: Vec<PersonalCommand>,
}

impl UiScene {
    pub fn new(name: impl Into<String>, commands: Vec<PersonalCommand>) -> Self {
        Self {
            name: name.into(),
            commands,
        }
    }

    pub fn gaming() -> Self {
        Self::new(
            "Gaming",
            vec![
                PersonalCommand::SetVolume(ChannelName::Game, 85),
                PersonalCommand::SetVolume(ChannelName::Chat, 70),
                PersonalCommand::SetVolume(ChannelName::Music, 35),
                PersonalCommand::SetVolume(ChannelName::Headphones, 75),
                PersonalCommand::SetHeadphoneLimiterEnabled(true),
                PersonalCommand::SetClipGuardEnabled(true),
            ],
        )
    }

    pub fn music() -> Self {
        Self::new(
            "Music",
            vec![
                PersonalCommand::SetVolume(ChannelName::Music, 85),
                PersonalCommand::SetVolume(ChannelName::Game, 30),
                PersonalCommand::SetVolume(ChannelName::Chat, 35),
                PersonalCommand::SetVolume(ChannelName::Headphones, 80),
                PersonalCommand::SetHeadphoneLimiterEnabled(true),
                PersonalCommand::SetHeadphoneEqEnabled(true),
                PersonalCommand::LoadHeadphoneEqProfile("Music".to_string()),
            ],
        )
    }

    pub fn night() -> Self {
        Self::new(
            "Night",
            vec![
                PersonalCommand::SetVolume(ChannelName::Music, 35),
                PersonalCommand::SetVolume(ChannelName::Game, 35),
                PersonalCommand::SetVolume(ChannelName::Chat, 45),
                PersonalCommand::SetVolume(ChannelName::Headphones, 55),
                PersonalCommand::SetHeadphoneLimiterEnabled(true),
                PersonalCommand::SetHeadphoneEqEnabled(true),
                PersonalCommand::LoadHeadphoneEqProfile("Night".to_string()),
            ],
        )
    }

    pub fn call_scene() -> Self {
        Self::new(
            "Call",
            vec![
                PersonalCommand::SetVolume(ChannelName::Chat, 85),
                PersonalCommand::SetVolume(ChannelName::Music, 15),
                PersonalCommand::SetVolume(ChannelName::Game, 20),
                PersonalCommand::SetVolume(ChannelName::Headphones, 70),
                PersonalCommand::SetHeadphoneLimiterEnabled(true),
                PersonalCommand::SetClipGuardEnabled(true),
            ],
        )
    }

    pub fn safe_now() -> Self {
        Self::new(
            "Safe Now",
            vec![
                PersonalCommand::SetVolume(ChannelName::Music, 0),
                PersonalCommand::SetVolume(ChannelName::Game, 0),
                PersonalCommand::SetVolume(ChannelName::Chat, 0),
                PersonalCommand::SetVolume(ChannelName::Headphones, 50),
                PersonalCommand::SetHeadphoneLimiterEnabled(true),
                PersonalCommand::SetClipGuardEnabled(true),
            ],
        )
    }

    pub fn personal_scenes() -> Vec<Self> {
        AppConfig::default().scenes()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn commands(&self) -> Vec<PersonalCommand> {
        self.commands.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub scenes: Vec<SceneConfig>,
    #[serde(default = "default_audio_routing_rule_configs")]
    pub audio_routing_rules: Vec<AudioRoutingRuleConfig>,
}

impl AppConfig {
    pub fn default_config_path() -> PathBuf {
        if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(config_home).join("goxlr-personal-ui/scenes.json");
        }

        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config/goxlr-personal-ui/scenes.json");
        }

        PathBuf::from("goxlr-personal-ui-scenes.json")
    }

    pub fn from_json_str(input: &str) -> Result<Self> {
        serde_json::from_str(input).context("failed to parse GoXLR personal UI scene config")
    }

    pub fn default_json() -> Result<String> {
        serde_json::to_string_pretty(&Self::default())
            .context("failed to serialize default GoXLR personal UI scene config")
    }

    pub fn load_or_create_default(path: &Path) -> Result<Self> {
        if path.exists() {
            let contents = fs::read_to_string(path)
                .with_context(|| format!("failed to read scene config {}", path.display()))?;
            return Self::from_json_str(&contents);
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create scene config directory {}",
                    parent.display()
                )
            })?;
        }

        let contents = Self::default_json()?;
        fs::write(path, contents)
            .with_context(|| format!("failed to write default scene config {}", path.display()))?;
        Ok(Self::default())
    }

    pub fn scenes(&self) -> Vec<UiScene> {
        self.scenes.iter().map(SceneConfig::to_ui_scene).collect()
    }

    pub fn audio_routing_rules(&self) -> Vec<AudioRoutingRule> {
        self.audio_routing_rules
            .iter()
            .map(AudioRoutingRuleConfig::to_rule)
            .collect()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            scenes: vec![
                SceneConfig::gaming(),
                SceneConfig::music(),
                SceneConfig::night(),
                SceneConfig::call_scene(),
                SceneConfig::safe_now(),
            ],
            audio_routing_rules: default_audio_routing_rule_configs(),
        }
    }
}

fn default_audio_routing_rule_configs() -> Vec<AudioRoutingRuleConfig> {
    vec![
        AudioRoutingRuleConfig::new("Spotify", "Music"),
        AudioRoutingRuleConfig::new("Discord", "Chat"),
    ]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioRoutingRuleConfig {
    pub app: String,
    pub route: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl AudioRoutingRuleConfig {
    pub fn new(app: impl Into<String>, route: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            route: route.into(),
            enabled: true,
        }
    }

    pub fn disabled(app: impl Into<String>, route: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            route: route.into(),
            enabled: false,
        }
    }

    fn to_rule(&self) -> AudioRoutingRule {
        AudioRoutingRule {
            app: self.app.clone(),
            route: self.route.clone(),
            enabled: self.enabled,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioRoutingRule {
    pub app: String,
    pub route: String,
    pub enabled: bool,
}

impl AudioRoutingRule {
    pub fn new(app: impl Into<String>, route: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            route: route.into(),
            enabled: true,
        }
    }

    pub fn disabled(app: impl Into<String>, route: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            route: route.into(),
            enabled: false,
        }
    }

    fn matches_stream(&self, stream: &AudioStream) -> bool {
        if !self.enabled {
            return false;
        }
        stream
            .display_name
            .to_ascii_lowercase()
            .contains(&self.app.to_ascii_lowercase())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppSceneConfig {
    path: PathBuf,
    config: AppConfig,
    scenes: Vec<UiScene>,
    reload_error: Option<String>,
}

impl AppSceneConfig {
    pub fn load_or_default(path: PathBuf) -> Self {
        match AppConfig::load_or_create_default(&path) {
            Ok(config) => Self {
                scenes: config.scenes(),
                path,
                config,
                reload_error: None,
            },
            Err(error) => {
                let config = AppConfig::default();
                Self {
                    scenes: config.scenes(),
                    path,
                    config,
                    reload_error: Some(error.to_string()),
                }
            }
        }
    }

    pub fn default_path() -> Self {
        Self::load_or_default(AppConfig::default_config_path())
    }

    pub fn reload(&mut self) {
        match AppConfig::load_or_create_default(&self.path) {
            Ok(config) => {
                self.scenes = config.scenes();
                self.config = config;
                self.reload_error = None;
            }
            Err(error) => {
                self.reload_error = Some(error.to_string());
            }
        }
    }

    pub fn save_config(&mut self, config: AppConfig) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create scene config directory {}",
                    parent.display()
                )
            })?;
        }

        if self.path.exists() {
            fs::copy(&self.path, self.backup_path()).with_context(|| {
                format!(
                    "failed to back up scene config {} before writing",
                    self.path.display()
                )
            })?;
        }

        let contents = serde_json::to_string_pretty(&config)
            .context("failed to serialize GoXLR personal UI scene config")?;
        fs::write(&self.path, contents)
            .with_context(|| format!("failed to write scene config {}", self.path.display()))?;
        self.scenes = config.scenes();
        self.config = config;
        self.reload_error = None;
        Ok(())
    }

    pub fn backup_path(&self) -> PathBuf {
        let mut path = self.path.clone();
        let extension = self
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| format!("{extension}.bak"))
            .unwrap_or_else(|| "bak".to_string());
        path.set_extension(extension);
        path
    }

    pub fn save_audio_routing_rule_for_stream(
        &mut self,
        stream: &AudioStream,
        route: impl Into<String>,
    ) -> Result<()> {
        let app = stream.routing_rule_app_name();
        let route = route.into();
        let mut config = self.config.clone();
        if let Some(existing) = config
            .audio_routing_rules
            .iter_mut()
            .find(|rule| rule.app.eq_ignore_ascii_case(&app))
        {
            existing.app = app;
            existing.route = route;
            existing.enabled = true;
        } else {
            config
                .audio_routing_rules
                .push(AudioRoutingRuleConfig::new(app, route));
        }
        self.save_config(config)
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn scenes(&self) -> Vec<UiScene> {
        self.scenes.clone()
    }

    pub fn scene_names(&self) -> Vec<String> {
        self.scenes
            .iter()
            .map(|scene| scene.name().to_string())
            .collect()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn reload_error(&self) -> Option<&str> {
        self.reload_error.as_deref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalBoolAction {
    Unset,
    SetTrue,
    SetFalse,
}

impl OptionalBoolAction {
    fn from_option(value: Option<bool>) -> Self {
        match value {
            Some(true) => Self::SetTrue,
            Some(false) => Self::SetFalse,
            None => Self::Unset,
        }
    }

    fn to_option(self) -> Option<bool> {
        match self {
            Self::Unset => None,
            Self::SetTrue => Some(true),
            Self::SetFalse => Some(false),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Unset => "Leave unchanged",
            Self::SetTrue => "Set on",
            Self::SetFalse => "Set off",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneEditor {
    config: AppConfig,
    selected_scene: usize,
    save_error: Option<String>,
}

impl SceneEditor {
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            config: config.clone(),
            selected_scene: 0,
            save_error: None,
        }
    }

    pub fn set_selected_scene(&mut self, index: usize) {
        if index < self.config.scenes.len() {
            self.selected_scene = index;
        }
    }

    pub fn add_scene(&mut self) {
        let insert_at = if self.config.scenes.is_empty() {
            0
        } else {
            self.selected_scene
                .saturating_add(1)
                .min(self.config.scenes.len())
        };
        self.config
            .scenes
            .insert(insert_at, SceneConfig::empty("New Scene"));
        self.selected_scene = insert_at;
    }

    pub fn delete_selected_scene(&mut self) {
        if self.config.scenes.is_empty() {
            self.config.scenes.push(SceneConfig::empty("New Scene"));
            self.selected_scene = 0;
            return;
        }

        self.config.scenes.remove(self.selected_scene);
        if self.config.scenes.is_empty() {
            self.config.scenes.push(SceneConfig::empty("New Scene"));
            self.selected_scene = 0;
        } else {
            self.selected_scene = self.selected_scene.min(self.config.scenes.len() - 1);
        }
    }

    pub fn move_selected_scene_up(&mut self) {
        if self.selected_scene > 0 && self.selected_scene < self.config.scenes.len() {
            self.config
                .scenes
                .swap(self.selected_scene, self.selected_scene - 1);
            self.selected_scene -= 1;
        }
    }

    pub fn move_selected_scene_down(&mut self) {
        if self.selected_scene + 1 < self.config.scenes.len() {
            self.config
                .scenes
                .swap(self.selected_scene, self.selected_scene + 1);
            self.selected_scene += 1;
        }
    }

    pub fn selected_scene(&self) -> usize {
        self.selected_scene
    }

    pub fn scene_names(&self) -> Vec<String> {
        self.config
            .scenes
            .iter()
            .map(|scene| scene.name.clone())
            .collect()
    }

    pub fn selected_scene_config(&self) -> Option<&SceneConfig> {
        self.config.scenes.get(self.selected_scene)
    }

    fn selected_scene_config_mut(&mut self) -> Option<&mut SceneConfig> {
        self.config.scenes.get_mut(self.selected_scene)
    }

    pub fn set_scene_name(&mut self, name: impl Into<String>) {
        if let Some(scene) = self.selected_scene_config_mut() {
            scene.name = name.into();
        }
    }

    pub fn set_volume(&mut self, channel: ChannelName, volume: Option<u8>) {
        let volume = volume.map(|value| value.min(100));
        if let Some(scene) = self.selected_scene_config_mut() {
            match channel {
                ChannelName::Headphones => scene.volumes.headphones = volume,
                ChannelName::Music => scene.volumes.music = volume,
                ChannelName::Game => scene.volumes.game = volume,
                ChannelName::Chat => scene.volumes.chat = volume,
                _ => {}
            }
        }
    }

    pub fn set_clip_guard_enabled(&mut self, enabled: Option<bool>) {
        if let Some(scene) = self.selected_scene_config_mut() {
            scene.clip_guard_enabled = enabled;
        }
    }

    pub fn clip_guard_action(&self) -> OptionalBoolAction {
        OptionalBoolAction::from_option(
            self.selected_scene_config()
                .and_then(|scene| scene.clip_guard_enabled),
        )
    }

    pub fn set_clip_guard_action(&mut self, action: OptionalBoolAction) {
        self.set_clip_guard_enabled(action.to_option());
    }

    pub fn set_headphone_limiter_enabled(&mut self, enabled: Option<bool>) {
        if let Some(scene) = self.selected_scene_config_mut() {
            scene.headphone_limiter_enabled = enabled;
        }
    }

    pub fn headphone_limiter_action(&self) -> OptionalBoolAction {
        OptionalBoolAction::from_option(
            self.selected_scene_config()
                .and_then(|scene| scene.headphone_limiter_enabled),
        )
    }

    pub fn set_headphone_limiter_action(&mut self, action: OptionalBoolAction) {
        self.set_headphone_limiter_enabled(action.to_option());
    }

    pub fn set_headphone_eq_enabled(&mut self, enabled: Option<bool>) {
        if let Some(scene) = self.selected_scene_config_mut() {
            scene.headphone_eq_enabled = enabled;
        }
    }

    pub fn headphone_eq_action(&self) -> OptionalBoolAction {
        OptionalBoolAction::from_option(
            self.selected_scene_config()
                .and_then(|scene| scene.headphone_eq_enabled),
        )
    }

    pub fn set_headphone_eq_action(&mut self, action: OptionalBoolAction) {
        self.set_headphone_eq_enabled(action.to_option());
    }

    pub fn set_headphone_eq_profile(&mut self, profile: Option<String>) {
        if let Some(scene) = self.selected_scene_config_mut() {
            scene.headphone_eq_profile = profile.filter(|profile| !profile.trim().is_empty());
        }
    }

    pub fn set_headphone_eq_profile_action_enabled(&mut self, enabled: bool) {
        if !enabled {
            self.set_headphone_eq_profile(None);
        } else if self
            .selected_scene_config()
            .is_some_and(|scene| scene.headphone_eq_profile.is_none())
        {
            self.set_headphone_eq_profile(Some(String::new()));
        }
    }

    pub fn save_to(&mut self, state: &mut AppSceneConfig) -> Result<()> {
        match state.save_config(self.config.clone()) {
            Ok(()) => {
                self.save_error = None;
                Ok(())
            }
            Err(error) => {
                self.save_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    pub fn save_error(&self) -> Option<&str> {
        self.save_error.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoutingRuleEditor {
    config: AppConfig,
    selected_rule: usize,
    save_error: Option<String>,
}

impl RoutingRuleEditor {
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            config: config.clone(),
            selected_rule: 0,
            save_error: None,
        }
    }

    pub fn selected_rule(&self) -> usize {
        self.selected_rule
    }

    pub fn set_selected_rule(&mut self, index: usize) {
        if index < self.config.audio_routing_rules.len() {
            self.selected_rule = index;
        }
    }

    pub fn rules(&self) -> Vec<AudioRoutingRuleConfig> {
        self.config.audio_routing_rules.clone()
    }

    pub fn rule_summaries(&self) -> Vec<String> {
        self.config
            .audio_routing_rules
            .iter()
            .map(|rule| {
                let suffix = if rule.enabled { "" } else { " (disabled)" };
                format!("{} -> {}{}", rule.app, rule.route, suffix)
            })
            .collect()
    }

    pub fn selected_rule_config(&self) -> Option<&AudioRoutingRuleConfig> {
        self.config.audio_routing_rules.get(self.selected_rule)
    }

    fn selected_rule_config_mut(&mut self) -> Option<&mut AudioRoutingRuleConfig> {
        self.config.audio_routing_rules.get_mut(self.selected_rule)
    }

    pub fn add_rule(&mut self) {
        let insert_at = if self.config.audio_routing_rules.is_empty() {
            0
        } else {
            self.selected_rule
                .saturating_add(1)
                .min(self.config.audio_routing_rules.len())
        };
        self.config
            .audio_routing_rules
            .insert(insert_at, AudioRoutingRuleConfig::new("New App", "Music"));
        self.selected_rule = insert_at;
    }

    pub fn delete_selected_rule(&mut self) {
        if self.config.audio_routing_rules.is_empty() {
            self.selected_rule = 0;
            return;
        }
        self.config.audio_routing_rules.remove(self.selected_rule);
        if self.config.audio_routing_rules.is_empty() {
            self.selected_rule = 0;
        } else {
            self.selected_rule = self
                .selected_rule
                .min(self.config.audio_routing_rules.len() - 1);
        }
    }

    pub fn move_selected_rule_up(&mut self) {
        if self.selected_rule > 0 && self.selected_rule < self.config.audio_routing_rules.len() {
            self.config
                .audio_routing_rules
                .swap(self.selected_rule, self.selected_rule - 1);
            self.selected_rule -= 1;
        }
    }

    pub fn move_selected_rule_down(&mut self) {
        if self.selected_rule + 1 < self.config.audio_routing_rules.len() {
            self.config
                .audio_routing_rules
                .swap(self.selected_rule, self.selected_rule + 1);
            self.selected_rule += 1;
        }
    }

    pub fn set_app(&mut self, app: impl Into<String>) {
        if let Some(rule) = self.selected_rule_config_mut() {
            rule.app = app.into();
        }
    }

    pub fn set_route(&mut self, route: impl Into<String>) {
        if let Some(rule) = self.selected_rule_config_mut() {
            rule.route = route.into();
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if let Some(rule) = self.selected_rule_config_mut() {
            rule.enabled = enabled;
        }
    }

    pub fn save_to(&mut self, state: &mut AppSceneConfig) -> Result<()> {
        match state.save_config(self.config.clone()) {
            Ok(()) => {
                self.save_error = None;
                Ok(())
            }
            Err(error) => {
                self.save_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    pub fn save_error(&self) -> Option<&str> {
        self.save_error.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneConfig {
    pub name: String,
    #[serde(default)]
    pub volumes: SceneVolumes,
    pub clip_guard_enabled: Option<bool>,
    pub clip_guard_threshold: Option<u8>,
    pub headphone_limiter_enabled: Option<bool>,
    pub headphone_limiter_threshold: Option<u8>,
    pub headphone_eq_enabled: Option<bool>,
    pub headphone_eq_profile: Option<String>,
    pub mic_type: Option<MicrophoneType>,
    pub mic_gain: Option<u16>,
    pub gate_enabled: Option<bool>,
    pub gate_threshold: Option<i8>,
    pub gate_attenuation: Option<u8>,
    pub gate_attack: Option<GateTimes>,
    pub gate_release: Option<GateTimes>,
    pub compressor_threshold: Option<i8>,
    pub compressor_ratio: Option<CompressorRatio>,
    pub compressor_attack: Option<CompressorAttackTime>,
    pub compressor_release: Option<CompressorReleaseTime>,
    pub compressor_makeup_gain: Option<i8>,
    pub deesser: Option<u8>,
}

impl SceneConfig {
    fn empty(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            volumes: SceneVolumes::default(),
            clip_guard_enabled: None,
            clip_guard_threshold: None,
            headphone_limiter_enabled: None,
            headphone_limiter_threshold: None,
            headphone_eq_enabled: None,
            headphone_eq_profile: None,
            mic_type: None,
            mic_gain: None,
            gate_enabled: None,
            gate_threshold: None,
            gate_attenuation: None,
            gate_attack: None,
            gate_release: None,
            compressor_threshold: None,
            compressor_ratio: None,
            compressor_attack: None,
            compressor_release: None,
            compressor_makeup_gain: None,
            deesser: None,
        }
    }

    fn gaming() -> Self {
        Self {
            name: "Gaming".to_string(),
            volumes: SceneVolumes {
                game: Some(85),
                chat: Some(70),
                music: Some(35),
                headphones: Some(75),
            },
            clip_guard_enabled: Some(true),
            headphone_limiter_enabled: Some(true),
            headphone_eq_enabled: None,
            headphone_eq_profile: None,
            ..Self::empty("")
        }
    }

    fn music() -> Self {
        Self {
            name: "Music".to_string(),
            volumes: SceneVolumes {
                music: Some(85),
                game: Some(30),
                chat: Some(35),
                headphones: Some(80),
            },
            clip_guard_enabled: None,
            headphone_limiter_enabled: Some(true),
            headphone_eq_enabled: Some(true),
            headphone_eq_profile: Some("Music".to_string()),
            ..Self::empty("")
        }
    }

    fn night() -> Self {
        Self {
            name: "Night".to_string(),
            volumes: SceneVolumes {
                music: Some(35),
                game: Some(35),
                chat: Some(45),
                headphones: Some(55),
            },
            clip_guard_enabled: None,
            headphone_limiter_enabled: Some(true),
            headphone_eq_enabled: Some(true),
            headphone_eq_profile: Some("Night".to_string()),
            ..Self::empty("")
        }
    }

    fn call_scene() -> Self {
        Self {
            name: "Call".to_string(),
            volumes: SceneVolumes {
                chat: Some(85),
                music: Some(15),
                game: Some(20),
                headphones: Some(70),
            },
            clip_guard_enabled: Some(true),
            headphone_limiter_enabled: Some(true),
            headphone_eq_enabled: None,
            headphone_eq_profile: None,
            ..Self::empty("")
        }
    }

    fn safe_now() -> Self {
        Self {
            name: "Safe Now".to_string(),
            volumes: SceneVolumes {
                music: Some(0),
                game: Some(0),
                chat: Some(0),
                headphones: Some(50),
            },
            clip_guard_enabled: Some(true),
            headphone_limiter_enabled: Some(true),
            headphone_eq_enabled: None,
            headphone_eq_profile: None,
            ..Self::empty("")
        }
    }

    pub fn to_ui_scene(&self) -> UiScene {
        let mut commands = Vec::new();

        self.volumes.push_commands(&mut commands);
        if let Some(mic_type) = self.mic_type {
            commands.push(PersonalCommand::SetMicrophoneType(mic_type));
            if let Some(gain) = self.mic_gain {
                commands.push(PersonalCommand::SetMicrophoneGain(mic_type, gain));
            }
        }
        if let Some(enabled) = self.gate_enabled {
            commands.push(PersonalCommand::SetGateActive(enabled));
        }
        if let Some(threshold) = self.gate_threshold {
            commands.push(PersonalCommand::SetGateThreshold(threshold));
        }
        if let Some(attenuation) = self.gate_attenuation {
            commands.push(PersonalCommand::SetGateAttenuation(attenuation));
        }
        if let Some(attack) = self.gate_attack {
            commands.push(PersonalCommand::SetGateAttack(attack));
        }
        if let Some(release) = self.gate_release {
            commands.push(PersonalCommand::SetGateRelease(release));
        }
        if let Some(threshold) = self.compressor_threshold {
            commands.push(PersonalCommand::SetCompressorThreshold(threshold));
        }
        if let Some(ratio) = self.compressor_ratio {
            commands.push(PersonalCommand::SetCompressorRatio(ratio));
        }
        if let Some(attack) = self.compressor_attack {
            commands.push(PersonalCommand::SetCompressorAttack(attack));
        }
        if let Some(release) = self.compressor_release {
            commands.push(PersonalCommand::SetCompressorReleaseTime(release));
        }
        if let Some(gain) = self.compressor_makeup_gain {
            commands.push(PersonalCommand::SetCompressorMakeupGain(gain));
        }
        if let Some(deesser) = self.deesser {
            commands.push(PersonalCommand::SetDeesser(deesser));
        }
        if let Some(enabled) = self.clip_guard_enabled {
            commands.push(PersonalCommand::SetClipGuardEnabled(enabled));
        }
        if let Some(threshold) = self.clip_guard_threshold {
            commands.push(PersonalCommand::SetClipGuardThreshold(threshold));
        }
        if let Some(enabled) = self.headphone_limiter_enabled {
            commands.push(PersonalCommand::SetHeadphoneLimiterEnabled(enabled));
        }
        if let Some(threshold) = self.headphone_limiter_threshold {
            commands.push(PersonalCommand::SetHeadphoneLimiterThreshold(threshold));
        }
        if let Some(enabled) = self.headphone_eq_enabled {
            commands.push(PersonalCommand::SetHeadphoneEqEnabled(enabled));
        }
        if let Some(profile) = &self.headphone_eq_profile {
            commands.push(PersonalCommand::LoadHeadphoneEqProfile(profile.clone()));
        }

        UiScene::new(self.name.clone(), commands)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneVolumes {
    pub headphones: Option<u8>,
    pub music: Option<u8>,
    pub game: Option<u8>,
    pub chat: Option<u8>,
}

impl SceneVolumes {
    fn push_commands(&self, commands: &mut Vec<PersonalCommand>) {
        if let Some(volume) = self.headphones {
            commands.push(PersonalCommand::SetVolume(ChannelName::Headphones, volume));
        }
        if let Some(volume) = self.music {
            commands.push(PersonalCommand::SetVolume(ChannelName::Music, volume));
        }
        if let Some(volume) = self.game {
            commands.push(PersonalCommand::SetVolume(ChannelName::Game, volume));
        }
        if let Some(volume) = self.chat {
            commands.push(PersonalCommand::SetVolume(ChannelName::Chat, volume));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelVolume {
    pub channel: ChannelName,
    pub value: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceSelection {
    available_serials: Vec<String>,
    selected_serial: Option<String>,
}

impl DeviceSelection {
    pub fn sync_available_devices(&mut self, mut serials: Vec<String>) {
        serials.sort();
        serials.dedup();
        let selected_is_available = self
            .selected_serial
            .as_ref()
            .is_some_and(|selected| serials.contains(selected));
        if !selected_is_available {
            self.selected_serial = serials.first().cloned();
        }
        self.available_serials = serials;
    }

    pub fn select_serial(&mut self, serial: impl Into<String>) {
        let serial = serial.into();
        if self.available_serials.contains(&serial) {
            self.selected_serial = Some(serial);
        }
    }

    pub fn selected_serial(&self) -> Option<&str> {
        self.selected_serial.as_deref()
    }

    pub fn available_serials(&self) -> Vec<String> {
        self.available_serials.clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioStream {
    pub id: u64,
    pub app_name: Option<String>,
    pub display_name: String,
    pub sink_label: String,
    pub muted: bool,
    pub corked: bool,
    pub volume_percent: Option<String>,
}

impl AudioStream {
    pub fn routing_rule_app_name(&self) -> String {
        self.app_name
            .as_deref()
            .filter(|app| !app.trim().is_empty())
            .unwrap_or(&self.display_name)
            .trim()
            .to_string()
    }

    pub fn volume_percent_value(&self) -> Option<u8> {
        self.volume_percent
            .as_deref()?
            .trim()
            .trim_end_matches('%')
            .parse::<u8>()
            .ok()
            .map(|value| value.min(100))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioRouteTarget {
    pub label: String,
    pub sink_name: String,
}

impl AudioRouteTarget {
    pub fn new(label: impl Into<String>, sink_name: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            sink_name: sink_name.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ActiveAudioStreams {
    pub streams: Vec<AudioStream>,
    pub route_targets: Vec<AudioRouteTarget>,
}

impl ActiveAudioStreams {
    pub fn from_pactl_json(sinks_json: &str, sink_inputs_json: &str) -> Result<Self> {
        let sinks: serde_json::Value =
            serde_json::from_str(sinks_json).context("failed to parse pactl sink JSON")?;
        let sink_inputs: serde_json::Value = serde_json::from_str(sink_inputs_json)
            .context("failed to parse pactl sink-input JSON")?;

        let mut sink_labels = HashMap::new();
        let mut route_targets = Vec::new();
        for sink in sinks.as_array().into_iter().flatten() {
            let Some(index) = sink.get("index").and_then(serde_json::Value::as_u64) else {
                continue;
            };
            let sink_name = sink
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Unknown sink");
            let label = sink
                .get("description")
                .and_then(serde_json::Value::as_str)
                .or_else(|| sink.get("name").and_then(serde_json::Value::as_str))
                .unwrap_or("Unknown sink")
                .to_string();
            sink_labels.insert(index, label);
            if let Some((order, route_label)) = goxlr_route_target_label(sink_name) {
                route_targets.push((order, AudioRouteTarget::new(route_label, sink_name)));
            }
        }
        route_targets.sort_by_key(|(order, _)| *order);
        let route_targets = route_targets
            .into_iter()
            .map(|(_, target)| target)
            .collect::<Vec<_>>();

        let mut streams = Vec::new();
        for input in sink_inputs.as_array().into_iter().flatten() {
            let Some(id) = input.get("index").and_then(serde_json::Value::as_u64) else {
                continue;
            };
            let sink_id = input.get("sink").and_then(serde_json::Value::as_u64);
            let properties = input
                .get("properties")
                .and_then(serde_json::Value::as_object);
            let app_name = properties
                .and_then(|properties| properties.get("application.name"))
                .and_then(serde_json::Value::as_str);
            let media_name = properties
                .and_then(|properties| properties.get("media.name"))
                .and_then(serde_json::Value::as_str);
            let display_name = match (app_name, media_name) {
                (Some(app), Some(media)) if !app.eq_ignore_ascii_case(media) => {
                    format!("{app} — {media}")
                }
                (Some(app), _) => app.to_string(),
                (_, Some(media)) => media.to_string(),
                _ => format!("Playback stream #{id}"),
            };
            let volume_percent = input
                .get("volume")
                .and_then(serde_json::Value::as_object)
                .and_then(|volume| volume.values().next())
                .and_then(|channel| channel.get("value_percent"))
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned);
            let sink_label = sink_id
                .and_then(|id| sink_labels.get(&id).cloned())
                .unwrap_or_else(|| "Unknown output".to_string());

            streams.push(AudioStream {
                id,
                app_name: app_name.map(ToOwned::to_owned),
                display_name,
                sink_label,
                muted: input
                    .get("mute")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                corked: input
                    .get("corked")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                volume_percent,
            });
        }

        streams.sort_by_key(|stream| stream.id);
        Ok(Self {
            streams,
            route_targets,
        })
    }

    pub fn summary(&self) -> String {
        match self.streams.len() {
            0 => "No active playback streams".to_string(),
            1 => "1 playback stream".to_string(),
            count => format!("{count} playback streams"),
        }
    }

    pub fn routing_moves(&self, rules: &[AudioRoutingRule]) -> Vec<UiCommand> {
        let mut moves = Vec::new();
        for stream in &self.streams {
            for rule in rules {
                if !rule.matches_stream(stream) {
                    continue;
                }
                let Some(target) = self
                    .route_targets
                    .iter()
                    .find(|target| target.label.eq_ignore_ascii_case(&rule.route))
                else {
                    continue;
                };
                if stream
                    .sink_label
                    .to_ascii_lowercase()
                    .contains(&target.label.to_ascii_lowercase())
                {
                    continue;
                }
                moves.push(UiCommand::MoveAudioStream {
                    stream_id: stream.id,
                    sink_name: target.sink_name.clone(),
                });
                break;
            }
        }
        moves
    }
}

fn goxlr_route_target_label(sink_name: &str) -> Option<(u8, &'static str)> {
    if !sink_name.contains("GoXLR") {
        return None;
    }

    if sink_name.contains("HiFi__Speaker__sink") {
        Some((0, "System"))
    } else if sink_name.contains("HiFi__Line1__sink") {
        Some((1, "Game"))
    } else if sink_name.contains("HiFi__Line2__sink") {
        Some((2, "Music"))
    } else if sink_name.contains("HiFi__Headphones__sink") {
        Some((3, "Chat"))
    } else if sink_name.contains("HiFi__Line3__sink") {
        Some((4, "Sample"))
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppSnapshot {
    pub connected: bool,
    pub error: Option<String>,
    pub daemon_version: Option<String>,
    pub device_serials: Vec<String>,
    pub device_serial: Option<String>,
    pub device_type: Option<String>,
    pub profile_name: Option<String>,
    pub mic_profile_name: Option<String>,
    pub mic_type: MicrophoneType,
    pub mic_gain: u16,
    pub gate_enabled: bool,
    pub gate_threshold: i8,
    pub gate_attenuation: u8,
    pub gate_attack: GateTimes,
    pub gate_release: GateTimes,
    pub compressor_threshold: i8,
    pub compressor_ratio: CompressorRatio,
    pub compressor_attack: CompressorAttackTime,
    pub compressor_release: CompressorReleaseTime,
    pub compressor_makeup_gain: i8,
    pub deesser: u8,
    pub channel_volumes: Vec<ChannelVolume>,
    pub clip_guard_enabled: bool,
    pub clip_guard_threshold: u8,
    pub headphone_limiter_enabled: bool,
    pub headphone_limiter_threshold: u8,
    pub headphone_eq_enabled: bool,
    pub headphone_eq_backend: Option<String>,
    pub headphone_eq_profile: Option<String>,
    pub active_audio_streams: ActiveAudioStreams,
    pub active_audio_error: Option<String>,
}

impl AppSnapshot {
    pub fn disconnected(error: impl Into<String>) -> Self {
        Self {
            connected: false,
            error: Some(error.into()),
            daemon_version: None,
            device_serials: Vec::new(),
            device_serial: None,
            device_type: None,
            profile_name: None,
            mic_profile_name: None,
            mic_type: MicrophoneType::Dynamic,
            mic_gain: 0,
            gate_enabled: false,
            gate_threshold: -50,
            gate_attenuation: 100,
            gate_attack: GateTimes::Gate10ms,
            gate_release: GateTimes::Gate200ms,
            compressor_threshold: 0,
            compressor_ratio: CompressorRatio::Ratio1_0,
            compressor_attack: CompressorAttackTime::Comp10ms,
            compressor_release: CompressorReleaseTime::Comp100ms,
            compressor_makeup_gain: 0,
            deesser: 0,
            channel_volumes: ControlledChannel::mvp_channels()
                .into_iter()
                .map(|channel| ChannelVolume {
                    channel: channel.channel,
                    value: 0,
                })
                .collect(),
            clip_guard_enabled: false,
            clip_guard_threshold: 0,
            headphone_limiter_enabled: false,
            headphone_limiter_threshold: 0,
            headphone_eq_enabled: false,
            headphone_eq_backend: None,
            headphone_eq_profile: None,
            active_audio_streams: ActiveAudioStreams::default(),
            active_audio_error: None,
        }
    }

    pub fn from_daemon_status(status: &DaemonStatus) -> Self {
        Self::from_daemon_status_for_selected(status, None)
    }

    pub fn from_daemon_status_for_selected(
        status: &DaemonStatus,
        selected_serial: Option<&str>,
    ) -> Self {
        let daemon_version = Some(status.config.daemon_version.clone());
        let mut device_serials = status.mixers.keys().cloned().collect::<Vec<_>>();
        device_serials.sort();
        let serial = selected_serial
            .filter(|selected| status.mixers.contains_key(*selected))
            .map(ToOwned::to_owned)
            .or_else(|| device_serials.first().cloned());
        let Some(serial) = serial else {
            return Self {
                daemon_version,
                device_serials,
                ..Self::disconnected("no GoXLR device connected")
            };
        };
        let Some(mixer) = status.mixers.get(&serial) else {
            return Self {
                daemon_version,
                device_serials,
                ..Self::disconnected("selected GoXLR device is unavailable")
            };
        };

        let settings = &mixer.settings;
        let mic_status = &mixer.mic_status;
        let gate = &mic_status.noise_gate;
        let compressor = &mic_status.compressor;
        let mic_type = mic_status.mic_type;
        Self {
            connected: true,
            error: None,
            daemon_version,
            device_serials,
            device_serial: Some(serial.clone()),
            device_type: Some(device_type_label(mixer.hardware.device_type).to_string()),
            profile_name: Some(mixer.profile_name.clone()),
            mic_profile_name: Some(mixer.mic_profile_name.clone()),
            mic_type,
            mic_gain: mic_status.mic_gains[mic_type],
            gate_enabled: gate.enabled,
            gate_threshold: gate.threshold,
            gate_attenuation: gate.attenuation,
            gate_attack: gate.attack,
            gate_release: gate.release,
            compressor_threshold: compressor.threshold,
            compressor_ratio: compressor.ratio,
            compressor_attack: compressor.attack,
            compressor_release: compressor.release,
            compressor_makeup_gain: compressor.makeup_gain,
            deesser: mixer.levels.deess,
            channel_volumes: ControlledChannel::mvp_channels()
                .into_iter()
                .map(|channel| ChannelVolume {
                    channel: channel.channel,
                    value: mixer.get_channel_volume(channel.channel),
                })
                .collect(),
            clip_guard_enabled: settings.clip_guard_enabled,
            clip_guard_threshold: settings.clip_guard_threshold,
            headphone_limiter_enabled: settings.headphone_limiter_enabled,
            headphone_limiter_threshold: settings.headphone_limiter_threshold,
            headphone_eq_enabled: settings.headphone_eq_enabled,
            headphone_eq_backend: if settings.headphone_eq_backend_name.is_empty() {
                None
            } else {
                Some(settings.headphone_eq_backend_name.clone())
            },
            headphone_eq_profile: settings.headphone_eq_active_profile.clone(),
            active_audio_streams: ActiveAudioStreams::default(),
            active_audio_error: None,
        }
    }

    pub fn status_line(&self) -> String {
        if self.connected {
            let device = self.device_type.as_deref().unwrap_or("GoXLR");
            let serial = self.device_serial.as_deref().unwrap_or("unknown serial");
            format!("Connected: {device} ({serial})")
        } else {
            format!(
                "Disconnected: {}",
                self.error.as_deref().unwrap_or("unknown error")
            )
        }
    }

    pub fn volume_for(&self, channel: ChannelName) -> Option<u8> {
        self.channel_volumes
            .iter()
            .find(|volume| volume.channel == channel)
            .map(|volume| volume.value)
    }
}

fn device_type_label(device_type: DeviceType) -> &'static str {
    match device_type {
        DeviceType::Unknown => "Unknown GoXLR",
        DeviceType::Full => "GoXLR",
        DeviceType::Mini => "GoXLR Mini",
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiCommand {
    Send(PersonalCommand),
    ApplyScene(UiScene),
    ApplyWindow(WindowAction),
    SelectDevice(String),
    MoveAudioStream { stream_id: u64, sink_name: String },
    SetAudioStreamMute { stream_id: u64, muted: bool },
    SetAudioStreamVolume { stream_id: u64, volume_percent: u8 },
    OpenAudioTool(ExternalAudioTool),
    SetAudioRoutingRules(Vec<AudioRoutingRule>),
    Refresh,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppViewMode {
    Mic,
    Effects,
    Full,
    QuickActions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickActions {
    view_mode: AppViewMode,
}

impl Default for QuickActions {
    fn default() -> Self {
        Self {
            view_mode: AppViewMode::QuickActions,
        }
    }
}

impl QuickActions {
    pub fn view_mode(&self) -> AppViewMode {
        self.view_mode
    }

    pub fn set_view_mode(&mut self, view_mode: AppViewMode) {
        self.view_mode = view_mode;
    }

    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            AppViewMode::Mic | AppViewMode::Effects | AppViewMode::Full => {
                AppViewMode::QuickActions
            }
            AppViewMode::QuickActions => AppViewMode::Full,
        };
    }

    pub fn scene_buttons(scenes: &[UiScene]) -> Vec<UiScene> {
        let mut selected = Vec::new();
        if let Some(safe_now) = scenes.iter().find(|scene| scene.name() == "Safe Now") {
            selected.push(safe_now.clone());
        }
        for scene in scenes {
            if selected.len() >= 5 {
                break;
            }
            if scene.name() != "Safe Now" {
                selected.push(scene.clone());
            }
        }
        selected
    }

    pub fn safety_commands() -> Vec<PersonalCommand> {
        vec![
            PersonalCommand::SetClipGuardEnabled(true),
            PersonalCommand::SetHeadphoneLimiterEnabled(true),
            PersonalCommand::SetHeadphoneEqEnabled(true),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    NormalSize,
    MiniSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MiniWindowMode {
    mini: bool,
    always_on_top: bool,
}

impl Default for MiniWindowMode {
    fn default() -> Self {
        Self {
            mini: false,
            always_on_top: false,
        }
    }
}

impl MiniWindowMode {
    pub const NORMAL_SIZE: [f32; 2] = [780.0, 720.0];
    pub const MINI_SIZE: [f32; 2] = [380.0, 420.0];

    pub fn is_mini(&self) -> bool {
        self.mini
    }

    pub fn always_on_top(&self) -> bool {
        self.always_on_top
    }

    pub fn window_action(&self) -> WindowAction {
        if self.mini {
            WindowAction::MiniSize
        } else {
            WindowAction::NormalSize
        }
    }

    pub fn show_mini(&mut self, quick_actions: &mut QuickActions) -> WindowAction {
        self.mini = true;
        self.always_on_top = true;
        quick_actions.set_view_mode(AppViewMode::QuickActions);
        self.window_action()
    }

    pub fn show_full(&mut self, quick_actions: &mut QuickActions) -> WindowAction {
        self.mini = false;
        self.always_on_top = false;
        quick_actions.set_view_mode(AppViewMode::Full);
        self.window_action()
    }

    pub fn toggle_mini_window(&mut self, quick_actions: &mut QuickActions) -> WindowAction {
        if self.mini {
            self.show_full(quick_actions)
        } else {
            self.show_mini(quick_actions)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayAction {
    ShowFull,
    ShowMini,
    SafeNow,
    Gaming,
    Music,
    Refresh,
    Quit,
}

impl TrayAction {
    pub fn id(self) -> &'static str {
        match self {
            TrayAction::ShowFull => "goxlr-personal-ui.show-full",
            TrayAction::ShowMini => "goxlr-personal-ui.show-mini",
            TrayAction::SafeNow => "goxlr-personal-ui.safe-now",
            TrayAction::Gaming => "goxlr-personal-ui.gaming",
            TrayAction::Music => "goxlr-personal-ui.music",
            TrayAction::Refresh => "goxlr-personal-ui.refresh",
            TrayAction::Quit => "goxlr-personal-ui.quit",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "goxlr-personal-ui.show-full" => Some(TrayAction::ShowFull),
            "goxlr-personal-ui.show-mini" => Some(TrayAction::ShowMini),
            "goxlr-personal-ui.safe-now" => Some(TrayAction::SafeNow),
            "goxlr-personal-ui.gaming" => Some(TrayAction::Gaming),
            "goxlr-personal-ui.music" => Some(TrayAction::Music),
            "goxlr-personal-ui.refresh" => Some(TrayAction::Refresh),
            "goxlr-personal-ui.quit" => Some(TrayAction::Quit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayMenuModel {
    items: Vec<(TrayAction, &'static str)>,
}

impl Default for TrayMenuModel {
    fn default() -> Self {
        Self {
            items: vec![
                (TrayAction::ShowFull, "Show full window"),
                (TrayAction::ShowMini, "Show mini window"),
                (TrayAction::SafeNow, "Safe Now"),
                (TrayAction::Gaming, "Gaming"),
                (TrayAction::Music, "Music"),
                (TrayAction::Refresh, "Refresh"),
                (TrayAction::Quit, "Quit"),
            ],
        }
    }
}

impl TrayMenuModel {
    pub fn items(&self) -> Vec<(TrayAction, &'static str)> {
        self.items.clone()
    }

    pub fn handle_action(
        &self,
        action: TrayAction,
        mini_window: &mut MiniWindowMode,
        quick_actions: &mut QuickActions,
    ) -> Vec<UiCommand> {
        match action {
            TrayAction::ShowFull => {
                vec![UiCommand::ApplyWindow(mini_window.show_full(quick_actions))]
            }
            TrayAction::ShowMini => {
                vec![UiCommand::ApplyWindow(mini_window.show_mini(quick_actions))]
            }
            TrayAction::SafeNow => vec![UiCommand::ApplyScene(UiScene::safe_now())],
            TrayAction::Gaming => vec![UiCommand::ApplyScene(UiScene::gaming())],
            TrayAction::Music => vec![UiCommand::ApplyScene(UiScene::music())],
            TrayAction::Refresh => vec![UiCommand::Refresh],
            TrayAction::Quit => vec![UiCommand::Quit],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WorkerEvent {
    Snapshot(AppSnapshot),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingVolumeCommand {
    channel: ChannelName,
    value: u8,
    updated_at: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VolumeDebouncer {
    delay: Duration,
    pending: Vec<PendingVolumeCommand>,
}

impl VolumeDebouncer {
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            pending: Vec::new(),
        }
    }

    pub fn queue(&mut self, channel: ChannelName, value: u8, updated_at: Duration) {
        if let Some(pending) = self
            .pending
            .iter_mut()
            .find(|pending| pending.channel == channel)
        {
            pending.value = value;
            pending.updated_at = updated_at;
        } else {
            self.pending.push(PendingVolumeCommand {
                channel,
                value,
                updated_at,
            });
        }
    }

    pub fn drain_ready(&mut self, now: Duration) -> Vec<UiCommand> {
        let mut ready = Vec::new();
        let mut still_pending = Vec::new();

        for pending in self.pending.drain(..) {
            if now.saturating_sub(pending.updated_at) >= self.delay {
                ready.push(UiCommand::Send(PersonalCommand::SetVolume(
                    pending.channel,
                    pending.value,
                )));
            } else {
                still_pending.push(pending);
            }
        }

        self.pending = still_pending;
        ready
    }
}

pub struct PersonalUiApp {
    snapshot: AppSnapshot,
    commands: Sender<UiCommand>,
    events: Receiver<WorkerEvent>,
    scene_config: AppSceneConfig,
    scene_editor: SceneEditor,
    routing_rule_editor: RoutingRuleEditor,
    show_scene_editor: bool,
    show_routing_rule_editor: bool,
    quick_actions: QuickActions,
    mini_window: MiniWindowMode,
    #[cfg(feature = "system-tray")]
    tray_menu: TrayMenuModel,
    #[cfg(feature = "system-tray")]
    tray: Option<TrayIntegration>,
    device_selection: DeviceSelection,
    pending_volumes: Vec<ChannelVolume>,
    volume_debouncer: VolumeDebouncer,
    started_at: Instant,
    last_repaint: Instant,
}

impl PersonalUiApp {
    pub fn new(commands: Sender<UiCommand>, events: Receiver<WorkerEvent>) -> Self {
        let snapshot = AppSnapshot::disconnected("waiting for daemon status");
        let now = Instant::now();
        let scene_config = AppSceneConfig::default_path();
        let scene_editor = SceneEditor::from_config(scene_config.config());
        let routing_rule_editor = RoutingRuleEditor::from_config(scene_config.config());
        let _ = commands.send(UiCommand::SetAudioRoutingRules(
            scene_config.config().audio_routing_rules(),
        ));
        Self {
            pending_volumes: snapshot.channel_volumes.clone(),
            snapshot,
            commands,
            events,
            scene_config,
            scene_editor,
            routing_rule_editor,
            show_scene_editor: false,
            show_routing_rule_editor: false,
            quick_actions: QuickActions::default(),
            mini_window: MiniWindowMode::default(),
            #[cfg(feature = "system-tray")]
            tray_menu: TrayMenuModel::default(),
            #[cfg(feature = "system-tray")]
            tray: TrayIntegration::new().ok(),
            device_selection: DeviceSelection::default(),
            volume_debouncer: VolumeDebouncer::new(Duration::from_millis(150)),
            started_at: now,
            last_repaint: now,
        }
    }

    fn drain_events(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                WorkerEvent::Snapshot(snapshot) => {
                    self.device_selection
                        .sync_available_devices(snapshot.device_serials.clone());
                    self.pending_volumes = snapshot.channel_volumes.clone();
                    self.snapshot = snapshot;
                }
                WorkerEvent::Error(error) => {
                    self.snapshot = AppSnapshot::disconnected(error);
                    self.pending_volumes = self.snapshot.channel_volumes.clone();
                }
            }
        }
    }

    fn send(&self, command: UiCommand) {
        let _ = self.commands.send(command);
    }

    fn queue_volume(&mut self, channel: ChannelName, value: u8) {
        self.volume_debouncer
            .queue(channel, value, self.started_at.elapsed());
    }

    fn flush_ready_volume_commands(&mut self) {
        for command in self.volume_debouncer.drain_ready(self.started_at.elapsed()) {
            self.send(command);
        }
    }

    fn reload_scenes(&mut self) {
        self.scene_config.reload();
        self.scene_editor = SceneEditor::from_config(self.scene_config.config());
        self.routing_rule_editor = RoutingRuleEditor::from_config(self.scene_config.config());
        self.send(UiCommand::SetAudioRoutingRules(
            self.scene_config.config().audio_routing_rules(),
        ));
    }

    fn save_scene_editor(&mut self) {
        if self.scene_editor.save_to(&mut self.scene_config).is_ok() {
            self.routing_rule_editor = RoutingRuleEditor::from_config(self.scene_config.config());
            self.send(UiCommand::SetAudioRoutingRules(
                self.scene_config.config().audio_routing_rules(),
            ));
        }
    }

    fn save_routing_rule_editor(&mut self) {
        if self
            .routing_rule_editor
            .save_to(&mut self.scene_config)
            .is_ok()
        {
            self.scene_editor = SceneEditor::from_config(self.scene_config.config());
            self.send(UiCommand::SetAudioRoutingRules(
                self.scene_config.config().audio_routing_rules(),
            ));
        }
    }

    fn save_stream_route_rule(&mut self, stream: &AudioStream, route: &str) {
        if self
            .scene_config
            .save_audio_routing_rule_for_stream(stream, route)
            .is_ok()
        {
            self.scene_editor = SceneEditor::from_config(self.scene_config.config());
            self.routing_rule_editor = RoutingRuleEditor::from_config(self.scene_config.config());
            self.send(UiCommand::SetAudioRoutingRules(
                self.scene_config.config().audio_routing_rules(),
            ));
        }
    }

    fn accent() -> egui::Color32 {
        egui::Color32::from_rgb(0, 210, 218)
    }

    fn bg() -> egui::Color32 {
        egui::Color32::from_rgb(30, 38, 34)
    }

    fn panel_bg() -> egui::Color32 {
        egui::Color32::from_rgb(43, 53, 49)
    }

    fn strip_bg() -> egui::Color32 {
        egui::Color32::from_rgb(49, 61, 56)
    }

    fn muted_text() -> egui::Color32 {
        egui::Color32::from_rgb(145, 154, 151)
    }

    fn apply_goxlr_style(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = Self::bg();
        style.visuals.window_fill = Self::panel_bg();
        style.visuals.faint_bg_color = egui::Color32::from_rgb(35, 44, 40);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(48, 59, 55);
        style.visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(226, 232, 230);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(56, 70, 65);
        style.visuals.widgets.hovered.fg_stroke.color = Self::accent();
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(33, 62, 60);
        style.visuals.widgets.active.fg_stroke.color = Self::accent();
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 8.0);
        ctx.set_style(style);
    }

    fn panel_frame() -> egui::Frame {
        egui::Frame::new()
            .fill(Self::panel_bg())
            .stroke(egui::Stroke::new(1.0, Self::accent()))
            .corner_radius(egui::CornerRadius::same(2))
            .inner_margin(egui::Margin::same(12))
    }

    fn soft_panel_frame() -> egui::Frame {
        egui::Frame::new()
            .fill(Self::strip_bg())
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 77, 74)))
            .corner_radius(egui::CornerRadius::same(2))
            .inner_margin(egui::Margin::same(10))
    }

    fn accent_button(label: impl Into<String>) -> egui::Button<'static> {
        egui::Button::new(
            egui::RichText::new(label.into())
                .monospace()
                .color(Self::accent())
                .strong(),
        )
        .fill(egui::Color32::from_rgb(28, 43, 41))
        .stroke(egui::Stroke::new(1.0, Self::accent()))
        .min_size(egui::vec2(96.0, 34.0))
    }

    fn danger_button(label: impl Into<String>) -> egui::Button<'static> {
        egui::Button::new(
            egui::RichText::new(label.into())
                .monospace()
                .color(egui::Color32::from_rgb(255, 128, 116))
                .strong(),
        )
        .fill(egui::Color32::from_rgb(62, 32, 32))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 92, 92)))
        .min_size(egui::vec2(112.0, 36.0))
    }

    fn update_pending_volume_value(&mut self, channel: ChannelName, value: u8) {
        if let Some(pending) = self
            .pending_volumes
            .iter_mut()
            .find(|pending| pending.channel == channel)
        {
            pending.value = value;
        }
        self.queue_volume(channel, value);
    }

    fn render_header_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.heading(
                egui::RichText::new("GoXLR Personal Control")
                    .monospace()
                    .color(egui::Color32::WHITE),
            );
            ui.add_space(12.0);
            let toggle_label = match self.quick_actions.view_mode() {
                AppViewMode::Mic | AppViewMode::Effects | AppViewMode::Full => "Mixer dashboard",
                AppViewMode::QuickActions => "Configuration",
            };
            if ui.add(Self::accent_button(toggle_label)).clicked() {
                self.quick_actions.toggle_view_mode();
            }
            let mini_label = if self.mini_window.is_mini() {
                "Normal window"
            } else {
                "Mini window"
            };
            if ui.add(Self::accent_button(mini_label)).clicked() {
                let action = self.mini_window.toggle_mini_window(&mut self.quick_actions);
                Self::apply_window_action(ctx, action, self.mini_window.always_on_top());
            }
            if ui.add(Self::accent_button("Refresh")).clicked() {
                self.send(UiCommand::Refresh);
            }
            for tool in ExternalAudioTool::daily_helpers() {
                if ui.add(egui::Button::new(tool.label()).small()).clicked() {
                    self.send(UiCommand::OpenAudioTool(tool));
                }
            }
        });
    }

    fn render_scene_panel(&mut self, ui: &mut egui::Ui) {
        Self::panel_frame().show(ui, |ui| {
            ui.set_min_width(245.0);
            ui.label(
                egui::RichText::new("Profiles / Scenes")
                    .monospace()
                    .color(egui::Color32::WHITE)
                    .size(16.0),
            );
            ui.separator();
            if let Some(profile) = &self.snapshot.profile_name {
                ui.label(
                    egui::RichText::new(format!("✓ {profile}"))
                        .monospace()
                        .color(Self::accent()),
                );
            } else {
                ui.label(
                    egui::RichText::new("No profile reported")
                        .monospace()
                        .color(Self::muted_text()),
                );
            }
            if let Some(mic_profile) = &self.snapshot.mic_profile_name {
                ui.label(
                    egui::RichText::new(format!("Mic: {mic_profile}"))
                        .monospace()
                        .color(Self::muted_text()),
                );
            }
            ui.add_space(14.0);
            ui.label(
                egui::RichText::new("Quick scenes")
                    .monospace()
                    .color(Self::muted_text()),
            );
            for scene in QuickActions::scene_buttons(&self.scene_config.scenes()) {
                let is_safe = scene.name().eq_ignore_ascii_case("safe now");
                let response = if is_safe {
                    ui.add(Self::danger_button(scene.name().to_string()))
                } else {
                    ui.add(Self::accent_button(scene.name().to_string()))
                };
                if response.clicked() {
                    self.send(UiCommand::ApplyScene(scene));
                }
            }
            ui.add_space(10.0);
            if let Some(error) = self.scene_config.reload_error() {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!("Scene reload issue: {error}"),
                );
            }
        });
    }

    fn render_status_card(&mut self, ui: &mut egui::Ui) {
        Self::panel_frame().show(ui, |ui| {
            ui.set_min_width(260.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("GOXLR")
                        .monospace()
                        .size(32.0)
                        .strong()
                        .color(egui::Color32::from_rgb(170, 174, 176)),
                );
                ui.label(
                    egui::RichText::new(self.snapshot.status_line())
                        .monospace()
                        .color(if self.snapshot.connected {
                            Self::accent()
                        } else {
                            egui::Color32::YELLOW
                        }),
                );
            });
            ui.separator();
            ui.label(
                egui::RichText::new("Device")
                    .monospace()
                    .color(Self::muted_text()),
            );
            ui.label(
                egui::RichText::new(
                    self.snapshot
                        .device_type
                        .as_deref()
                        .unwrap_or("GoXLR device not reported"),
                )
                .monospace()
                .color(egui::Color32::WHITE),
            );
            if let Some(serial) = &self.snapshot.device_serial {
                ui.label(
                    egui::RichText::new(serial)
                        .monospace()
                        .color(Self::muted_text()),
                );
            }
            ui.add_space(8.0);
            if self.snapshot.device_serials.len() > 1 {
                let mut selected = self
                    .device_selection
                    .selected_serial()
                    .unwrap_or("Select device")
                    .to_string();
                egui::ComboBox::from_label("Device")
                    .selected_text(selected.clone())
                    .show_ui(ui, |ui| {
                        for serial in self.device_selection.available_serials() {
                            ui.selectable_value(&mut selected, serial.clone(), serial);
                        }
                    });
                if self.device_selection.selected_serial() != Some(selected.as_str()) {
                    self.device_selection.select_serial(selected.clone());
                    self.send(UiCommand::SelectDevice(selected));
                }
            }
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                for (label, enabled) in [
                    ("ClipGuard", self.snapshot.clip_guard_enabled),
                    ("Limiter", self.snapshot.headphone_limiter_enabled),
                    ("EQ", self.snapshot.headphone_eq_enabled),
                ] {
                    let color = if enabled {
                        Self::accent()
                    } else {
                        Self::muted_text()
                    };
                    ui.label(
                        egui::RichText::new(format!(
                            "{label}: {}",
                            if enabled { "ON" } else { "OFF" }
                        ))
                        .monospace()
                        .color(color),
                    );
                }
            });
        });
    }

    fn render_active_streams_panel(&mut self, ui: &mut egui::Ui) {
        Self::panel_frame().show(ui, |ui| {
            ui.set_min_width(260.0);
            ui.label(
                egui::RichText::new(DashboardCopy::active_playback_heading())
                    .monospace()
                    .color(egui::Color32::WHITE)
                    .size(16.0),
            );
            ui.label(
                egui::RichText::new(self.snapshot.active_audio_streams.summary())
                    .monospace()
                    .color(Self::muted_text()),
            );
            if let Some(error) = &self.snapshot.active_audio_error {
                ui.colored_label(egui::Color32::YELLOW, format!("pactl: {error}"));
            }
            ui.separator();
            if self.snapshot.active_audio_streams.streams.is_empty() {
                ui.label(
                    egui::RichText::new("Start audio in an app to see its route here.")
                        .monospace()
                        .color(Self::muted_text()),
                );
            }
            let route_targets = self.snapshot.active_audio_streams.route_targets.clone();
            for stream in self.snapshot.active_audio_streams.streams.clone() {
                Self::soft_panel_frame().show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(&stream.display_name)
                            .monospace()
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!("→ {}", stream.sink_label))
                            .monospace()
                            .color(Self::accent()),
                    );
                    let mut flags = Vec::new();
                    if let Some(volume) = &stream.volume_percent {
                        flags.push(format!("vol {volume}"));
                    }
                    if stream.muted {
                        flags.push("muted".to_string());
                    }
                    if stream.corked {
                        flags.push("paused".to_string());
                    }
                    if !flags.is_empty() {
                        ui.label(
                            egui::RichText::new(flags.join(" • "))
                                .monospace()
                                .color(Self::muted_text()),
                        );
                    }
                    ui.horizontal(|ui| {
                        let mute_label = if stream.muted {
                            "Unmute stream"
                        } else {
                            "Mute stream"
                        };
                        if ui.add(egui::Button::new(mute_label).small()).clicked() {
                            self.send(UiCommand::SetAudioStreamMute {
                                stream_id: stream.id,
                                muted: !stream.muted,
                            });
                        }
                        if let Some(mut volume) = stream.volume_percent_value() {
                            ui.label(
                                egui::RichText::new("Volume")
                                    .monospace()
                                    .color(Self::muted_text()),
                            );
                            if ui
                                .add_sized(
                                    egui::vec2(118.0, 18.0),
                                    egui::Slider::new(&mut volume, 0..=100).suffix("%"),
                                )
                                .changed()
                            {
                                self.send(UiCommand::SetAudioStreamVolume {
                                    stream_id: stream.id,
                                    volume_percent: volume,
                                });
                            }
                        }
                    });
                    if !route_targets.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(DashboardCopy::manual_route_label())
                                    .monospace()
                                    .color(Self::muted_text()),
                            );
                            for target in &route_targets {
                                let already_on_target = stream.sink_label.contains(&target.label);
                                if ui
                                    .add_enabled(
                                        !already_on_target,
                                        egui::Button::new(target.label.clone()).small(),
                                    )
                                    .clicked()
                                {
                                    self.send(UiCommand::MoveAudioStream {
                                        stream_id: stream.id,
                                        sink_name: target.sink_name.clone(),
                                    });
                                }
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(DashboardCopy::persistent_route_label())
                                    .monospace()
                                    .color(Self::muted_text()),
                            );
                            for target in &route_targets {
                                if ui
                                    .add(egui::Button::new(target.label.clone()).small())
                                    .on_hover_text(format!(
                                        "Always route {} to {}",
                                        stream.routing_rule_app_name(),
                                        target.label
                                    ))
                                    .clicked()
                                {
                                    self.save_stream_route_rule(&stream, &target.label);
                                    self.send(UiCommand::MoveAudioStream {
                                        stream_id: stream.id,
                                        sink_name: target.sink_name.clone(),
                                    });
                                }
                            }
                        });
                    }
                });
            }
        });
    }

    fn render_channel_strip(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        channel: ChannelName,
        mut value: u8,
    ) {
        Self::soft_panel_frame().show(ui, |ui| {
            ui.set_min_width(94.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(label.to_uppercase())
                        .monospace()
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(6.0);
                let changed = ui
                    .add_sized(
                        egui::vec2(46.0, 190.0),
                        egui::Slider::new(&mut value, 0..=100)
                            .vertical()
                            .show_value(false)
                            .text(""),
                    )
                    .changed();
                if changed {
                    self.update_pending_volume_value(channel, value);
                }
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(format!("{value}%"))
                        .monospace()
                        .size(16.0)
                        .color(Self::accent()),
                );
            });
        });
    }

    fn render_effects_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.heading("Voice Effects");
            ui.separator();
            ui.label("Quick presets for the GoXLR effects bank");
        });
        ui.add_space(8.0);
        ui.label("This is the next practical web-UI parity chunk: fast access to FX on/off, reverb, robot, and hard tune without opening the full browser UI.");
        ui.add_space(12.0);

        egui::Grid::new("effects_quick_presets")
            .num_columns(2)
            .spacing(egui::vec2(12.0, 10.0))
            .show(ui, |ui| {
                for preset in EffectsQuickPreset::daily_presets() {
                    ui.vertical(|ui| {
                        if ui.add(Self::accent_button(preset.name())).clicked() {
                            self.send(UiCommand::ApplyScene(UiScene::new(
                                preset.name(),
                                preset.commands(),
                            )));
                        }
                        ui.label(preset.description());
                    });
                    ui.label(format!("{} commands", preset.commands().len()));
                    ui.end_row();
                }
            });

        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("FX On").clicked() {
                self.send(UiCommand::Send(PersonalCommand::SetFXEnabled(true)));
            }
            if ui.button("FX Off").clicked() {
                self.send(UiCommand::Send(PersonalCommand::SetFXEnabled(false)));
            }
            if ui.button("Robot On").clicked() {
                self.send(UiCommand::Send(PersonalCommand::SetRobotEnabled(true)));
            }
            if ui.button("Hard Tune On").clicked() {
                self.send(UiCommand::Send(PersonalCommand::SetHardTuneEnabled(true)));
            }
        });
    }

    fn render_mic_processing_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            Self::panel_frame().show(ui, |ui| {
                ui.set_min_width(360.0);
                ui.label(
                    egui::RichText::new("MIC SETUP")
                        .monospace()
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.label(format!(
                    "Active mic profile: {}",
                    self.snapshot
                        .mic_profile_name
                        .as_deref()
                        .unwrap_or("unknown")
                ));
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    for mic_type in [
                        MicrophoneType::Dynamic,
                        MicrophoneType::Condenser,
                        MicrophoneType::Jack,
                    ] {
                        let selected = self.snapshot.mic_type == mic_type;
                        let label = format!("{:?}", mic_type);
                        let button = if selected {
                            Self::accent_button(label)
                        } else {
                            egui::Button::new(label).small()
                        };
                        if ui.add(button).clicked() {
                            self.send(UiCommand::Send(PersonalCommand::SetMicrophoneType(
                                mic_type,
                            )));
                        }
                    }
                });
                let mut mic_gain = self.snapshot.mic_gain;
                if ui
                    .add(egui::Slider::new(&mut mic_gain, 0..=72).text("Mic gain"))
                    .changed()
                {
                    self.send(UiCommand::Send(PersonalCommand::SetMicrophoneGain(
                        self.snapshot.mic_type,
                        mic_gain,
                    )));
                }
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.add(Self::accent_button("Save mic profile")).clicked() {
                        self.send(UiCommand::Send(PersonalCommand::SaveMicProfile));
                    }
                    if ui.add(Self::accent_button("Reload settings")).clicked() {
                        self.send(UiCommand::Send(PersonalCommand::ReloadSettings));
                    }
                });
            });

            ui.add_space(12.0);
            Self::panel_frame().show(ui, |ui| {
                ui.set_min_width(360.0);
                ui.label(
                    egui::RichText::new("GATE / DE-ESS")
                        .monospace()
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                if ui
                    .add(Self::accent_button(if self.snapshot.gate_enabled {
                        "Disable gate"
                    } else {
                        "Enable gate"
                    }))
                    .clicked()
                {
                    self.send(UiCommand::Send(PersonalCommand::SetGateActive(
                        !self.snapshot.gate_enabled,
                    )));
                }
                let mut gate_threshold = self.snapshot.gate_threshold;
                if ui
                    .add(egui::Slider::new(&mut gate_threshold, -59..=0).text("Gate threshold dB"))
                    .changed()
                {
                    self.send(UiCommand::Send(PersonalCommand::SetGateThreshold(
                        gate_threshold,
                    )));
                }
                let mut gate_attenuation = self.snapshot.gate_attenuation;
                if ui
                    .add(
                        egui::Slider::new(&mut gate_attenuation, 0..=100)
                            .text("Gate attenuation %"),
                    )
                    .changed()
                {
                    self.send(UiCommand::Send(PersonalCommand::SetGateAttenuation(
                        gate_attenuation,
                    )));
                }
                let mut deesser = self.snapshot.deesser;
                if ui
                    .add(egui::Slider::new(&mut deesser, 0..=100).text("De-esser %"))
                    .changed()
                {
                    self.send(UiCommand::Send(PersonalCommand::SetDeesser(deesser)));
                }
            });

            ui.add_space(12.0);
            Self::panel_frame().show(ui, |ui| {
                ui.set_min_width(360.0);
                ui.label(
                    egui::RichText::new("COMPRESSOR / SAFETY")
                        .monospace()
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                let mut compressor_threshold = self.snapshot.compressor_threshold;
                if ui
                    .add(
                        egui::Slider::new(&mut compressor_threshold, -40..=0)
                            .text("Compressor threshold dB"),
                    )
                    .changed()
                {
                    self.send(UiCommand::Send(PersonalCommand::SetCompressorThreshold(
                        compressor_threshold,
                    )));
                }
                let mut makeup_gain = self.snapshot.compressor_makeup_gain;
                if ui
                    .add(egui::Slider::new(&mut makeup_gain, 0..=24).text("Makeup gain dB"))
                    .changed()
                {
                    self.send(UiCommand::Send(PersonalCommand::SetCompressorMakeupGain(
                        makeup_gain,
                    )));
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label("Ratio:");
                    for ratio in [
                        CompressorRatio::Ratio2_0,
                        CompressorRatio::Ratio4_0,
                        CompressorRatio::Ratio8_0,
                    ] {
                        if ui.button(format!("{:?}", ratio)).clicked() {
                            self.send(UiCommand::Send(PersonalCommand::SetCompressorRatio(ratio)));
                        }
                    }
                });
                let mut clip_threshold = self.snapshot.clip_guard_threshold;
                if ui
                    .add(
                        egui::Slider::new(&mut clip_threshold, 0..=100).text("ClipGuard threshold"),
                    )
                    .changed()
                {
                    self.send(UiCommand::Send(PersonalCommand::SetClipGuardThreshold(
                        clip_threshold,
                    )));
                }
                let mut limiter_threshold = self.snapshot.headphone_limiter_threshold;
                if ui
                    .add(
                        egui::Slider::new(&mut limiter_threshold, 0..=100)
                            .text("Limiter threshold"),
                    )
                    .changed()
                {
                    self.send(UiCommand::Send(
                        PersonalCommand::SetHeadphoneLimiterThreshold(limiter_threshold),
                    ));
                }
            });
        });
    }

    fn render_mixer_dashboard(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            self.render_scene_panel(ui);
            ui.add_space(18.0);
            Self::panel_frame().show(ui, |ui| {
                ui.set_min_width(560.0);
                ui.label(
                    egui::RichText::new("MIXER")
                        .monospace()
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let channel_labels = ControlledChannel::mvp_channels();
                    for volume in self.pending_volumes.clone() {
                        let label = channel_labels
                            .iter()
                            .find(|channel| channel.channel == volume.channel)
                            .map(|channel| channel.label)
                            .unwrap_or("Channel");
                        self.render_channel_strip(ui, label, volume.channel, volume.value);
                    }
                });
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.add(Self::accent_button("Enable ClipGuard")).clicked() {
                        self.send(UiCommand::Send(PersonalCommand::SetClipGuardEnabled(true)));
                    }
                    if ui.add(Self::accent_button("Enable limiter")).clicked() {
                        self.send(UiCommand::Send(
                            PersonalCommand::SetHeadphoneLimiterEnabled(true),
                        ));
                    }
                    if ui.add(Self::accent_button("Enable EQ")).clicked() {
                        self.send(UiCommand::Send(PersonalCommand::SetHeadphoneEqEnabled(
                            true,
                        )));
                    }
                });
            });
            ui.add_space(18.0);
            ui.vertical(|ui| {
                self.render_status_card(ui);
                ui.add_space(12.0);
                self.render_active_streams_panel(ui);
            });
        });
    }

    fn apply_window_action(ctx: &egui::Context, action: WindowAction, always_on_top: bool) {
        let size = match action {
            WindowAction::MiniSize => MiniWindowMode::MINI_SIZE,
            WindowAction::NormalSize => MiniWindowMode::NORMAL_SIZE,
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            size[0], size[1],
        )));
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if always_on_top {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        }));
    }

    #[cfg(feature = "system-tray")]
    fn handle_local_command(&mut self, ctx: &egui::Context, command: UiCommand) {
        match command {
            UiCommand::ApplyWindow(action) => {
                Self::apply_window_action(ctx, action, self.mini_window.always_on_top());
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            UiCommand::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            other => self.send(other),
        }
    }

    #[cfg(feature = "system-tray")]
    fn handle_tray_action(&mut self, ctx: &egui::Context, action: TrayAction) {
        for command in
            self.tray_menu
                .handle_action(action, &mut self.mini_window, &mut self.quick_actions)
        {
            self.handle_local_command(ctx, command);
        }
    }

    #[cfg(feature = "system-tray")]
    fn drain_tray_events(&mut self, ctx: &egui::Context) {
        if let Some(tray) = &self.tray {
            let actions = tray.drain_actions();
            for action in actions {
                self.handle_tray_action(ctx, action);
            }
        }
    }
}

impl eframe::App for PersonalUiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        #[cfg(feature = "system-tray")]
        self.drain_tray_events(ctx);

        if self.last_repaint.elapsed() > Duration::from_millis(500) {
            self.send(UiCommand::Refresh);
            self.last_repaint = Instant::now();
        }

        Self::apply_goxlr_style(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Self::bg()).inner_margin(egui::Margin::same(14)))
            .show(ctx, |ui| {
            self.render_header_controls(ui, ctx);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for (label, selected) in [
                    ("Mic", self.quick_actions.view_mode() == AppViewMode::Mic),
                    ("Effects", self.quick_actions.view_mode() == AppViewMode::Effects),
                    (DashboardCopy::mixer_tab(), self.quick_actions.view_mode() == AppViewMode::QuickActions),
                    (DashboardCopy::configuration_tab(), self.quick_actions.view_mode() == AppViewMode::Full),
                    ("Lighting", false),
                    ("Routing", false),
                    ("System", false),
                ] {
                    let text = egui::RichText::new(label)
                        .monospace()
                        .color(if selected { Self::accent() } else { egui::Color32::WHITE });
                    let button = egui::Button::new(text)
                        .fill(if selected { egui::Color32::from_rgb(35, 52, 50) } else { Self::bg() })
                        .stroke(egui::Stroke::new(1.0, if selected { Self::accent() } else { egui::Color32::from_rgb(56, 66, 62) }))
                        .min_size(egui::vec2(138.0, 34.0));
                    if ui.add(button).clicked() {
                        match label {
                            "Mic" => self.quick_actions.set_view_mode(AppViewMode::Mic),
                            "Effects" => self.quick_actions.set_view_mode(AppViewMode::Effects),
                            label if label == DashboardCopy::mixer_tab() => self.quick_actions.set_view_mode(AppViewMode::QuickActions),
                            label if label == DashboardCopy::configuration_tab() => self.quick_actions.set_view_mode(AppViewMode::Full),
                            _ => {}
                        }
                    }
                }
            });

            if self.quick_actions.view_mode() == AppViewMode::QuickActions {
                self.render_mixer_dashboard(ui);
                return;
            }

            if self.quick_actions.view_mode() == AppViewMode::Mic {
                self.render_mic_processing_page(ui);
                return;
            }

            if self.quick_actions.view_mode() == AppViewMode::Effects {
                self.render_effects_page(ui);
                return;
            }

            if let Some(profile) = &self.snapshot.profile_name {
                ui.label(format!("Profile: {profile}"));
            }
            if let Some(mic_profile) = &self.snapshot.mic_profile_name {
                ui.label(format!("Mic profile: {mic_profile}"));
            }

            ui.add_space(12.0);
            ui.heading("Scenes");
            ui.label(format!("Scene config: {}", self.scene_config.path().display()));
            if let Some(error) = self.scene_config.reload_error() {
                ui.colored_label(egui::Color32::YELLOW, format!("Using previous scenes: {error}"));
            }
            ui.horizontal_wrapped(|ui| {
                for scene in self.scene_config.scenes() {
                    if ui.button(scene.name()).clicked() {
                        self.send(UiCommand::ApplyScene(scene));
                    }
                }
                if ui.button("Reload scenes").clicked() {
                    self.reload_scenes();
                }
                if ui.button("Edit scenes").clicked() {
                    self.show_scene_editor = !self.show_scene_editor;
                }
                if ui.button("Edit routing rules").clicked() {
                    self.show_routing_rule_editor = !self.show_routing_rule_editor;
                }
                if ui.button("Refresh").clicked() {
                    self.send(UiCommand::Refresh);
                }
            });

            if self.show_scene_editor {
                ui.add_space(8.0);
                ui.group(|ui| {
                    ui.heading("Scene editor");
                    let scene_names = self.scene_editor.scene_names();
                    if !scene_names.is_empty() {
                        let selected = self.scene_editor.selected_scene();
                        egui::ComboBox::from_label("Scene")
                            .selected_text(
                                scene_names
                                    .get(selected)
                                    .cloned()
                                    .unwrap_or_else(|| "Unknown".to_string()),
                            )
                            .show_ui(ui, |ui| {
                                for (index, name) in scene_names.iter().enumerate() {
                                    if ui.selectable_label(index == selected, name).clicked() {
                                        self.scene_editor.set_selected_scene(index);
                                    }
                                }
                            });

                        ui.horizontal(|ui| {
                            if ui.button("Add scene").clicked() {
                                self.scene_editor.add_scene();
                            }
                            if ui.button("Delete scene").clicked() {
                                self.scene_editor.delete_selected_scene();
                            }
                            if ui
                                .add_enabled(selected > 0, egui::Button::new("Move up"))
                                .clicked()
                            {
                                self.scene_editor.move_selected_scene_up();
                            }
                            if ui
                                .add_enabled(
                                    selected + 1 < scene_names.len(),
                                    egui::Button::new("Move down"),
                                )
                                .clicked()
                            {
                                self.scene_editor.move_selected_scene_down();
                            }
                        });

                        if let Some(scene) = self.scene_editor.selected_scene_config().cloned() {
                            let mut name = scene.name.clone();
                            if ui.text_edit_singleline(&mut name).changed() {
                                self.scene_editor.set_scene_name(name);
                            }

                            ui.label("Volumes");
                            for (label, channel, current) in [
                                ("Headphones", ChannelName::Headphones, scene.volumes.headphones),
                                ("Music", ChannelName::Music, scene.volumes.music),
                                ("Game", ChannelName::Game, scene.volumes.game),
                                ("Chat", ChannelName::Chat, scene.volumes.chat),
                            ] {
                                ui.horizontal(|ui| {
                                    let mut enabled = current.is_some();
                                    if ui.checkbox(&mut enabled, label).changed() && !enabled {
                                        self.scene_editor.set_volume(channel, None);
                                    }
                                    let mut value = current.unwrap_or(0);
                                    if ui
                                        .add_enabled(
                                            enabled,
                                            egui::Slider::new(&mut value, 0..=100).text("%"),
                                        )
                                        .changed()
                                    {
                                        self.scene_editor.set_volume(channel, Some(value));
                                    }
                                    if enabled && current.is_none() {
                                        self.scene_editor.set_volume(channel, Some(value));
                                    }
                                });
                            }

                            for (label, action, setter) in [
                                (
                                    "ClipGuard",
                                    scene.clip_guard_enabled,
                                    SceneEditor::set_clip_guard_action as fn(&mut SceneEditor, OptionalBoolAction),
                                ),
                                (
                                    "Headphone limiter",
                                    scene.headphone_limiter_enabled,
                                    SceneEditor::set_headphone_limiter_action,
                                ),
                                (
                                    "Headphone EQ",
                                    scene.headphone_eq_enabled,
                                    SceneEditor::set_headphone_eq_action,
                                ),
                            ] {
                                let mut selected_action = OptionalBoolAction::from_option(action);
                                egui::ComboBox::from_label(label)
                                    .selected_text(selected_action.label())
                                    .show_ui(ui, |ui| {
                                        for option in [
                                            OptionalBoolAction::Unset,
                                            OptionalBoolAction::SetTrue,
                                            OptionalBoolAction::SetFalse,
                                        ] {
                                            ui.selectable_value(
                                                &mut selected_action,
                                                option,
                                                option.label(),
                                            );
                                        }
                                    });
                                if selected_action != OptionalBoolAction::from_option(action) {
                                    setter(&mut self.scene_editor, selected_action);
                                }
                            }

                            let mut eq_profile_enabled = scene.headphone_eq_profile.is_some();
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut eq_profile_enabled, "Load EQ profile").changed() {
                                    self.scene_editor
                                        .set_headphone_eq_profile_action_enabled(eq_profile_enabled);
                                }
                                let mut eq_profile = scene.headphone_eq_profile.clone().unwrap_or_default();
                                if ui
                                    .add_enabled(
                                        eq_profile_enabled,
                                        egui::TextEdit::singleline(&mut eq_profile).hint_text("EQ profile"),
                                    )
                                    .changed()
                                {
                                    self.scene_editor
                                        .set_headphone_eq_profile(Some(eq_profile.clone()));
                                }
                            });

                            if ui.button("Save scenes").clicked() {
                                self.save_scene_editor();
                            }
                            if let Some(error) = self.scene_editor.save_error() {
                                ui.colored_label(egui::Color32::YELLOW, format!("Save failed: {error}"));
                            }
                        }
                    }
                });
            }

            if self.show_routing_rule_editor {
                ui.add_space(8.0);
                ui.group(|ui| {
                    ui.heading("Audio routing rule editor");
                    ui.label("Rules match active playback stream names and move them to GoXLR routes.");
                    let summaries = self.routing_rule_editor.rule_summaries();
                    if summaries.is_empty() {
                        ui.label("No routing rules. Add one or leave the list empty to disable auto-routing.");
                    } else {
                        let selected = self.routing_rule_editor.selected_rule();
                        egui::ComboBox::from_label("Rule")
                            .selected_text(
                                summaries
                                    .get(selected)
                                    .cloned()
                                    .unwrap_or_else(|| "Select rule".to_string()),
                            )
                            .show_ui(ui, |ui| {
                                for (index, summary) in summaries.iter().enumerate() {
                                    if ui.selectable_label(index == selected, summary).clicked() {
                                        self.routing_rule_editor.set_selected_rule(index);
                                    }
                                }
                            });
                    }

                    let selected = self.routing_rule_editor.selected_rule();
                    ui.horizontal(|ui| {
                        if ui.button("Add rule").clicked() {
                            self.routing_rule_editor.add_rule();
                        }
                        if ui.button("Delete rule").clicked() {
                            self.routing_rule_editor.delete_selected_rule();
                        }
                        if ui
                            .add_enabled(selected > 0, egui::Button::new("Move up"))
                            .clicked()
                        {
                            self.routing_rule_editor.move_selected_rule_up();
                        }
                        if ui
                            .add_enabled(
                                selected + 1 < self.routing_rule_editor.rule_summaries().len(),
                                egui::Button::new("Move down"),
                            )
                            .clicked()
                        {
                            self.routing_rule_editor.move_selected_rule_down();
                        }
                    });

                    if let Some(rule) = self.routing_rule_editor.selected_rule_config().cloned() {
                        let mut enabled = rule.enabled;
                        if ui.checkbox(&mut enabled, "Enabled").changed() {
                            self.routing_rule_editor.set_enabled(enabled);
                        }

                        ui.horizontal(|ui| {
                            ui.label("App contains");
                            let mut app = rule.app.clone();
                            if ui.text_edit_singleline(&mut app).changed() {
                                self.routing_rule_editor.set_app(app);
                            }
                        });

                        let mut route = rule.route.clone();
                        egui::ComboBox::from_label("Route")
                            .selected_text(route.clone())
                            .show_ui(ui, |ui| {
                                for option in ["System", "Game", "Music", "Chat", "Sample"] {
                                    ui.selectable_value(&mut route, option.to_string(), option);
                                }
                            });
                        if route != rule.route {
                            self.routing_rule_editor.set_route(route);
                        }
                    }

                    if ui.button("Save routing rules").clicked() {
                        self.save_routing_rule_editor();
                    }
                    if let Some(error) = self.routing_rule_editor.save_error() {
                        ui.colored_label(egui::Color32::YELLOW, format!("Save failed: {error}"));
                    }
                });
            }

            ui.add_space(12.0);
            ui.heading("Volumes");
            let channel_labels = ControlledChannel::mvp_channels();
            for volume in self.pending_volumes.clone() {
                let label = channel_labels
                    .iter()
                    .find(|channel| channel.channel == volume.channel)
                    .map(|channel| channel.label)
                    .unwrap_or("Channel");
                let mut value = volume.value;
                if ui
                    .add(egui::Slider::new(&mut value, 0..=100).text(label))
                    .changed()
                {
                    if let Some(pending) = self
                        .pending_volumes
                        .iter_mut()
                        .find(|pending| pending.channel == volume.channel)
                    {
                        pending.value = value;
                    }
                    self.queue_volume(volume.channel, value);
                }
            }

            ui.add_space(12.0);
            ui.heading("Safety / EQ");
            let mut clip_guard = self.snapshot.clip_guard_enabled;
            if ui.checkbox(&mut clip_guard, "ClipGuard").changed() {
                self.send(UiCommand::Send(PersonalCommand::SetClipGuardEnabled(
                    clip_guard,
                )));
            }

            let mut limiter = self.snapshot.headphone_limiter_enabled;
            if ui.checkbox(&mut limiter, "Headphone limiter").changed() {
                self.send(UiCommand::Send(PersonalCommand::SetHeadphoneLimiterEnabled(
                    limiter,
                )));
            }

            let mut eq_enabled = self.snapshot.headphone_eq_enabled;
            if ui.checkbox(&mut eq_enabled, "Headphone EQ").changed() {
                self.send(UiCommand::Send(PersonalCommand::SetHeadphoneEqEnabled(
                    eq_enabled,
                )));
            }

            let backend = self
                .snapshot
                .headphone_eq_backend
                .as_deref()
                .unwrap_or("backend not reported");
            ui.label(format!("EQ backend: {backend}"));
            if let Some(profile) = &self.snapshot.headphone_eq_profile {
                ui.label(format!("EQ profile: {profile}"));
            }
        });

        self.flush_ready_volume_commands();
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

pub fn spawn_ipc_worker(commands: Receiver<UiCommand>, events: Sender<WorkerEvent>) {
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = events.send(WorkerEvent::Error(format!(
                    "failed to start async runtime: {error}"
                )));
                return;
            }
        };

        runtime.block_on(async move {
            if let Err(error) = ipc_worker_loop(commands, events.clone()).await {
                let _ = events.send(WorkerEvent::Error(error.to_string()));
            }
        });
    });
}

async fn ipc_worker_loop(commands: Receiver<UiCommand>, events: Sender<WorkerEvent>) -> Result<()> {
    let mut client = connect_ipc().await?;
    let mut selected_serial: Option<String> = None;
    let mut routing_rules: Vec<AudioRoutingRule> = Vec::new();
    poll_and_publish(
        &mut client,
        &events,
        selected_serial.as_deref(),
        &routing_rules,
    )
    .await;

    loop {
        match commands.recv_timeout(Duration::from_millis(500)) {
            Ok(UiCommand::Send(command)) => {
                let serial = active_serial(client.status(), selected_serial.as_deref())?;
                client.command(&serial, command.into()).await?;
                poll_and_publish(
                    &mut client,
                    &events,
                    selected_serial.as_deref(),
                    &routing_rules,
                )
                .await;
            }
            Ok(UiCommand::ApplyScene(scene)) => {
                let serial = active_serial(client.status(), selected_serial.as_deref())?;
                for command in scene.commands() {
                    client.command(&serial, command.into()).await?;
                }
                poll_and_publish(
                    &mut client,
                    &events,
                    selected_serial.as_deref(),
                    &routing_rules,
                )
                .await;
            }
            Ok(UiCommand::SelectDevice(serial)) => {
                selected_serial = Some(serial);
                poll_and_publish(
                    &mut client,
                    &events,
                    selected_serial.as_deref(),
                    &routing_rules,
                )
                .await;
            }
            Ok(UiCommand::MoveAudioStream {
                stream_id,
                sink_name,
            }) => {
                if let Err(error) = move_audio_stream(stream_id, &sink_name) {
                    let _ = events.send(WorkerEvent::Error(error.to_string()));
                }
                poll_and_publish(
                    &mut client,
                    &events,
                    selected_serial.as_deref(),
                    &routing_rules,
                )
                .await;
            }
            Ok(UiCommand::SetAudioStreamMute { stream_id, muted }) => {
                if let Err(error) = set_audio_stream_mute(stream_id, muted) {
                    let _ = events.send(WorkerEvent::Error(error.to_string()));
                }
                poll_and_publish(
                    &mut client,
                    &events,
                    selected_serial.as_deref(),
                    &routing_rules,
                )
                .await;
            }
            Ok(UiCommand::SetAudioStreamVolume {
                stream_id,
                volume_percent,
            }) => {
                if let Err(error) = set_audio_stream_volume(stream_id, volume_percent) {
                    let _ = events.send(WorkerEvent::Error(error.to_string()));
                }
                poll_and_publish(
                    &mut client,
                    &events,
                    selected_serial.as_deref(),
                    &routing_rules,
                )
                .await;
            }
            Ok(UiCommand::OpenAudioTool(tool)) => {
                if let Err(error) = open_audio_tool(tool) {
                    let _ = events.send(WorkerEvent::Error(error.to_string()));
                }
            }
            Ok(UiCommand::SetAudioRoutingRules(rules)) => {
                routing_rules = rules;
                poll_and_publish(
                    &mut client,
                    &events,
                    selected_serial.as_deref(),
                    &routing_rules,
                )
                .await;
            }
            Ok(UiCommand::Refresh) => {
                poll_and_publish(
                    &mut client,
                    &events,
                    selected_serial.as_deref(),
                    &routing_rules,
                )
                .await;
            }
            Ok(UiCommand::ApplyWindow(_)) => {}
            Ok(UiCommand::Quit) => return Ok(()),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                poll_and_publish(
                    &mut client,
                    &events,
                    selected_serial.as_deref(),
                    &routing_rules,
                )
                .await;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

pub fn ipc_socket_path_candidates() -> Vec<String> {
    let mut candidates = vec![ipc_socket_path()];

    #[cfg(target_family = "unix")]
    {
        let fallback = "/tmp/goxlr.socket".to_string();
        if !candidates.contains(&fallback) {
            candidates.push(fallback);
        }
    }

    candidates
}

async fn connect_ipc() -> Result<IPCClient> {
    let mut errors = Vec::new();

    for socket_path in ipc_socket_path_candidates() {
        let path = if cfg!(windows) {
            socket_path.as_str().to_ns_name::<GenericNamespaced>()
        } else {
            socket_path.as_str().to_fs_name::<GenericFilePath>()
        }
        .with_context(|| format!("unable to process IPC socket path {socket_path}"))?;

        match LocalSocketStream::connect(path).await {
            Ok(connection) => {
                let socket: Socket<DaemonResponse, DaemonRequest> = Socket::new(connection);
                return Ok(IPCClient::new(socket));
            }
            Err(error) => errors.push(format!("{socket_path}: {error}")),
        }
    }

    anyhow::bail!(
        "unable to connect to the GoXLR daemon IPC socket; tried {}",
        errors.join(", ")
    )
}

fn move_audio_stream(stream_id: u64, sink_name: &str) -> Result<()> {
    let output = Command::new("pactl")
        .arg("move-sink-input")
        .arg(stream_id.to_string())
        .arg(sink_name)
        .output()
        .context("failed to run pactl move-sink-input")?;

    if !output.status.success() {
        anyhow::bail!(
            "pactl move-sink-input failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(())
}

fn set_audio_stream_mute(stream_id: u64, muted: bool) -> Result<()> {
    let output = Command::new("pactl")
        .arg("set-sink-input-mute")
        .arg(stream_id.to_string())
        .arg(if muted { "1" } else { "0" })
        .output()
        .context("failed to run pactl set-sink-input-mute")?;

    if !output.status.success() {
        anyhow::bail!(
            "pactl set-sink-input-mute failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(())
}

fn set_audio_stream_volume(stream_id: u64, volume_percent: u8) -> Result<()> {
    let output = Command::new("pactl")
        .arg("set-sink-input-volume")
        .arg(stream_id.to_string())
        .arg(format!("{}%", volume_percent.min(100)))
        .output()
        .context("failed to run pactl set-sink-input-volume")?;

    if !output.status.success() {
        anyhow::bail!(
            "pactl set-sink-input-volume failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(())
}

fn open_audio_tool(tool: ExternalAudioTool) -> Result<()> {
    Command::new(tool.command())
        .spawn()
        .with_context(|| format!("failed to launch {}", tool.command()))?;
    Ok(())
}

fn read_active_audio_streams() -> Result<ActiveAudioStreams> {
    let sinks = pactl_json(["list", "sinks"])?;
    let sink_inputs = pactl_json(["list", "sink-inputs"])?;
    ActiveAudioStreams::from_pactl_json(&sinks, &sink_inputs)
}

fn pactl_json<const N: usize>(args: [&str; N]) -> Result<String> {
    let output = Command::new("pactl")
        .arg("--format=json")
        .args(args)
        .output()
        .context("failed to run pactl; is PulseAudio/PipeWire available?")?;

    if !output.status.success() {
        anyhow::bail!(
            "pactl failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    String::from_utf8(output.stdout).context("pactl returned non-UTF8 JSON")
}

async fn poll_and_publish(
    client: &mut IPCClient,
    events: &Sender<WorkerEvent>,
    selected_serial: Option<&str>,
    routing_rules: &[AudioRoutingRule],
) {
    match client.poll_status().await {
        Ok(()) => {
            let mut snapshot =
                AppSnapshot::from_daemon_status_for_selected(client.status(), selected_serial);
            match read_active_audio_streams() {
                Ok(streams) => {
                    for move_command in streams.routing_moves(routing_rules) {
                        if let UiCommand::MoveAudioStream {
                            stream_id,
                            sink_name,
                        } = move_command
                        {
                            if let Err(error) = move_audio_stream(stream_id, &sink_name) {
                                let _ = events.send(WorkerEvent::Error(error.to_string()));
                            }
                        }
                    }
                    snapshot.active_audio_streams = streams;
                }
                Err(error) => snapshot.active_audio_error = Some(error.to_string()),
            }
            let _ = events.send(WorkerEvent::Snapshot(snapshot));
        }
        Err(error) => {
            let _ = events.send(WorkerEvent::Error(error.to_string()));
        }
    }
}

fn active_serial(status: &DaemonStatus, selected_serial: Option<&str>) -> Result<String> {
    if let Some(selected) = selected_serial {
        if status.mixers.contains_key(selected) {
            return Ok(selected.to_string());
        }
    }

    let mut serials = status.mixers.keys().cloned().collect::<Vec<_>>();
    serials.sort();
    serials
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no GoXLR device connected"))
}

#[cfg(feature = "system-tray")]
struct TrayIntegration {
    _handle: ksni::blocking::Handle<GoXlrTray>,
    actions: Receiver<TrayAction>,
}

#[cfg(feature = "system-tray")]
impl TrayIntegration {
    fn new() -> Result<Self> {
        use ksni::blocking::TrayMethods;

        let (actions_tx, actions_rx) = std::sync::mpsc::channel();
        let handle = GoXlrTray {
            actions: actions_tx,
        }
        .assume_sni_available(true)
        .spawn()
        .context("failed to start Linux StatusNotifier tray")?;

        Ok(Self {
            _handle: handle,
            actions: actions_rx,
        })
    }

    fn drain_actions(&self) -> Vec<TrayAction> {
        let mut actions = Vec::new();
        while let Ok(action) = self.actions.try_recv() {
            actions.push(action);
        }
        actions
    }
}

#[cfg(feature = "system-tray")]
struct GoXlrTray {
    actions: Sender<TrayAction>,
}

#[cfg(feature = "system-tray")]
impl GoXlrTray {
    fn send(&self, action: TrayAction) {
        let _ = self.actions.send(action);
    }
}

#[cfg(feature = "system-tray")]
impl ksni::Tray for GoXlrTray {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "goxlr-personal-ui".to_string()
    }

    fn title(&self) -> String {
        "GoXLR Personal Control".to_string()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Hardware
    }

    fn icon_name(&self) -> String {
        "audio-card".to_string()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "GoXLR Personal Control".to_string(),
            description: "Quick GoXLR scene and window actions".to_string(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayAction::ShowMini);
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayAction::SafeNow);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{StandardItem, SubMenu};

        vec![
            StandardItem {
                label: "Show full window".to_string(),
                activate: Box::new(|tray: &mut Self| tray.send(TrayAction::ShowFull)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Show mini window".to_string(),
                activate: Box::new(|tray: &mut Self| tray.send(TrayAction::ShowMini)),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            SubMenu {
                label: "Scenes".to_string(),
                submenu: vec![
                    StandardItem {
                        label: "Safe Now".to_string(),
                        activate: Box::new(|tray: &mut Self| tray.send(TrayAction::SafeNow)),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: "Gaming".to_string(),
                        activate: Box::new(|tray: &mut Self| tray.send(TrayAction::Gaming)),
                        ..Default::default()
                    }
                    .into(),
                    StandardItem {
                        label: "Music".to_string(),
                        activate: Box::new(|tray: &mut Self| tray.send(TrayAction::Music)),
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Refresh".to_string(),
                activate: Box::new(|tray: &mut Self| tray.send(TrayAction::Refresh)),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Quit".to_string(),
                activate: Box::new(|tray: &mut Self| tray.send(TrayAction::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn run_native() -> eframe::Result<()> {
    let (command_tx, command_rx) = std::sync::mpsc::channel();
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    spawn_ipc_worker(command_rx, event_tx);

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "GoXLR Personal Control",
        options,
        Box::new(move |_creation_context| {
            Ok(Box::new(PersonalUiApp::new(command_tx.clone(), event_rx)))
        }),
    )
}
