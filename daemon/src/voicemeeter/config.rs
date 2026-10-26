use anyhow::{Context, Result};
use goxlr_types::{Button, ChannelName, EffectBankPresets, FaderName, SampleBank};
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Sub-directory of the GoXLR profile directory where the Voicemeeter profiles live, every
/// GoXLR profile `<name>.goxlr` gets a matching `Voicemeeter/<name>.json`.
const PROFILE_SUBDIR: &str = "Voicemeeter";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VmProfile {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub faders: Vec<FaderLink>,

    #[serde(default)]
    pub buttons: Vec<ButtonLink>,
}

impl Default for VmProfile {
    fn default() -> Self {
        Self {
            enabled: true,
            faders: vec![],
            buttons: vec![],
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Both,
    ToVoicemeeter,
    FromVoicemeeter,
}

impl Direction {
    pub fn sends_to_vm(self) -> bool {
        self != Direction::FromVoicemeeter
    }

    pub fn accepts_from_vm(self) -> bool {
        self != Direction::ToVoicemeeter
    }
}

/// A volume on the GoXLR side, either a channel directly, or 'whatever is on Fader X'
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VolumeSource {
    Channel(ChannelName),
    Fader(FaderName),
}

/// A boolean state on the GoXLR side
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ButtonSource {
    FaderMute(FaderName),
    CoughMute,
    FxEnabled,
    Megaphone,
    Robot,
    HardTune,
    EffectPreset(EffectBankPresets),
    SamplerBank(SampleBank),

    /// Physical button held down, this is read-only on the GoXLR side
    ButtonHeld(Button),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FaderLink {
    pub source: VolumeSource,

    /// Voicemeeter parameter, eg. `Strip[0].Gain`
    pub target: String,

    #[serde(default)]
    pub direction: Direction,

    /// Voicemeeter value when the GoXLR is at 0%
    #[serde(default = "default_min_db")]
    pub min_db: f32,

    /// Voicemeeter value when the GoXLR is at 100%
    #[serde(default)]
    pub max_db: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ButtonLink {
    pub source: ButtonSource,

    /// Voicemeeter parameter, eg. `Strip[0].Mute`, `Bus[1].Mono` or `MacroButton[3].State`
    pub target: String,

    #[serde(default)]
    pub direction: Direction,

    #[serde(default)]
    pub invert: bool,
}

fn default_true() -> bool {
    true
}

fn default_min_db() -> f32 {
    -60.0
}

pub fn profile_directory(goxlr_profiles: &Path) -> PathBuf {
    goxlr_profiles.join(PROFILE_SUBDIR)
}

pub fn profile_path(goxlr_profiles: &Path, name: &str) -> PathBuf {
    profile_directory(goxlr_profiles).join(format!("{name}.json"))
}

/// Loads the Voicemeeter profile matching a GoXLR profile, a missing file is an empty profile.
pub fn load_profile(goxlr_profiles: &Path, name: &str) -> VmProfile {
    let path = profile_path(goxlr_profiles, name);
    if !path.exists() {
        debug!("No Voicemeeter profile at {:?}, using defaults", path);
        return VmProfile::default();
    }

    match fs::read_to_string(&path)
        .map_err(anyhow::Error::from)
        .and_then(|text| serde_json::from_str(&text).map_err(anyhow::Error::from))
    {
        Ok(profile) => profile,
        Err(e) => {
            warn!("Unable to read Voicemeeter profile {:?}: {}", path, e);
            VmProfile::default()
        }
    }
}

pub fn save_profile(goxlr_profiles: &Path, name: &str, profile: &VmProfile) -> Result<()> {
    let dir = profile_directory(goxlr_profiles);
    fs::create_dir_all(&dir).with_context(|| format!("Unable to create {dir:?}"))?;

    let path = profile_path(goxlr_profiles, name);
    let text = serde_json::to_string_pretty(profile)?;
    fs::write(&path, text).with_context(|| format!("Unable to write {path:?}"))?;
    debug!("Saved Voicemeeter profile to {:?}", path);
    Ok(())
}

pub fn delete_profile(goxlr_profiles: &Path, name: &str) -> Result<()> {
    let path = profile_path(goxlr_profiles, name);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("Unable to delete {path:?}"))?;
    }
    Ok(())
}
