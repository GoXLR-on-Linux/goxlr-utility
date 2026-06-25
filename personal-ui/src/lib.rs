use std::collections::HashMap;
use std::fs;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use goxlr_ipc::client::Client;
use goxlr_ipc::clients::ipc::ipc_client::IPCClient;
use goxlr_ipc::clients::ipc::ipc_socket::Socket;
use goxlr_ipc::{
    DaemonRequest, DaemonResponse, DaemonStatus, GoXLRCommand, Sampler, Submixes, ipc_socket_path,
};
use goxlr_types::{
    AnimationMode, Button, ButtonColourGroups, ButtonColourOffStyle, ChannelName,
    CompressorAttackTime, CompressorRatio, CompressorReleaseTime, DeviceType, EchoStyle,
    EffectBankPresets, EncoderColourTargets, EqFrequencies, FaderDisplayStyle, FaderName,
    GateTimes, GenderStyle, HardTuneSource, HardTuneStyle, InputDevice, MegaphoneStyle,
    MicrophoneType, MiniEqFrequencies, Mix, MuteFunction, MuteState, OutputDevice, PitchStyle,
    ReverbStyle, RobotRange, RobotStyle, SampleBank, SampleButtons, SamplePlayOrder,
    SamplePlaybackMode, SamplerColourTargets, SimpleColourTargets, SubMixChannelName, VodMode,
    WaterfallDirection,
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
pub struct ContentLayoutPolicy;

impl ContentLayoutPolicy {
    pub fn main_content_scroll_enabled() -> bool {
        Self::main_content_vertical_scroll_enabled()
    }

    pub fn main_content_vertical_scroll_enabled() -> bool {
        true
    }

    pub fn main_content_horizontal_scroll_enabled() -> bool {
        false
    }

    pub fn scroll_area_id() -> &'static str {
        "personal_ui_main_content_scroll"
    }

    pub fn bounded_panel_allocates_before_frame() -> bool {
        true
    }

    pub fn bounded_panel_avoids_sentinel_height_allocation() -> bool {
        true
    }

    pub fn bounded_panel_outer_min_height() -> f32 {
        0.0
    }

    pub fn min_action_button_width() -> f32 {
        112.0
    }

    pub fn wide_action_button_width() -> f32 {
        148.0
    }

    pub fn slider_width() -> f32 {
        180.0
    }

    pub fn max_content_width() -> f32 {
        1320.0
    }

    pub fn content_width_for_available_width(available_width: f32) -> f32 {
        available_width.min(Self::max_content_width()).max(360.0)
    }

    pub fn page_body_centers_in_wide_windows() -> bool {
        true
    }

    pub fn wide_window_side_margin(available_width: f32) -> f32 {
        ((available_width - Self::content_width_for_available_width(available_width)) / 2.0)
            .max(0.0)
    }

    pub fn desktop_panel_gap() -> f32 {
        14.0
    }

    pub fn wrapped_rows_top_align() -> bool {
        true
    }

    pub fn section_header_width() -> f32 {
        960.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemLayoutPolicy;

impl SystemLayoutPolicy {
    pub fn panel_width() -> f32 {
        390.0
    }

    pub fn button_width() -> f32 {
        150.0
    }

    pub fn uses_wrapped_cards() -> bool {
        true
    }

    pub fn destructive_actions_are_omitted_from_daily_controls() -> bool {
        true
    }

    pub fn uses_guarded_main_profile_actions() -> bool {
        true
    }

    pub fn profile_panel_width() -> f32 {
        420.0
    }

    pub fn profile_button_width() -> f32 {
        150.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticsLayoutPolicy;

impl DiagnosticsLayoutPolicy {
    pub fn panel_width() -> f32 {
        460.0
    }

    pub fn detail_panel_width() -> f32 {
        640.0
    }

    pub fn button_width() -> f32 {
        150.0
    }

    pub fn uses_read_only_status_cards() -> bool {
        true
    }

    pub fn shows_ipc_socket_candidates() -> bool {
        true
    }

    pub fn shows_read_only_log_viewer() -> bool {
        true
    }

    pub fn log_panel_width() -> f32 {
        640.0
    }

    pub fn log_row_height() -> f32 {
        46.0
    }

    pub fn log_row_limit() -> usize {
        12
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticsStatusSeverity {
    Ok,
    Warning,
    Info,
}

impl DiagnosticsStatusSeverity {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warning => "Warning",
            Self::Info => "Info",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsStatusRow {
    label: String,
    value: String,
    severity: DiagnosticsStatusSeverity,
}

impl DiagnosticsStatusRow {
    pub fn new(
        label: impl Into<String>,
        value: impl Into<String>,
        severity: DiagnosticsStatusSeverity,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            severity,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn severity(&self) -> DiagnosticsStatusSeverity {
        self.severity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticsLogFilter {
    All,
    WarningsOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsLogEntry {
    timestamp: String,
    severity: DiagnosticsStatusSeverity,
    category: String,
    message: String,
}

impl DiagnosticsLogEntry {
    pub fn new(
        timestamp: impl Into<String>,
        severity: DiagnosticsStatusSeverity,
        category: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: timestamp.into(),
            severity,
            category: category.into(),
            message: message.into(),
        }
    }

    pub fn timestamp(&self) -> &str {
        &self.timestamp
    }

    pub fn severity(&self) -> DiagnosticsStatusSeverity {
        self.severity
    }

    pub fn category(&self) -> &str {
        &self.category
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn is_read_only(&self) -> bool {
        true
    }

    pub fn filtered_rows(entries: &[Self], filter: DiagnosticsLogFilter) -> Vec<Self> {
        entries
            .iter()
            .filter(|entry| match filter {
                DiagnosticsLogFilter::All => true,
                DiagnosticsLogFilter::WarningsOnly => {
                    entry.severity == DiagnosticsStatusSeverity::Warning
                }
            })
            .cloned()
            .collect()
    }

    pub fn recent_rows(entries: &[Self], limit: usize, filter: DiagnosticsLogFilter) -> Vec<Self> {
        let mut rows = Self::filtered_rows(entries, filter);
        if rows.len() > limit {
            rows.drain(0..rows.len() - limit);
        }
        rows
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AboutLayoutPolicy;

impl AboutLayoutPolicy {
    pub fn panel_width() -> f32 {
        540.0
    }

    pub fn panel_height() -> f32 {
        132.0
    }

    pub fn status_badge_width() -> f32 {
        92.0
    }

    pub fn uses_read_only_summary_cards() -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImplementedParityStatus {
    Implemented,
    Partial,
}

impl ImplementedParityStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Implemented => "Implemented",
            Self::Partial => "Partial",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplementedParityItem {
    label: &'static str,
    status: ImplementedParityStatus,
    description: &'static str,
}

impl ImplementedParityItem {
    pub fn new(
        label: &'static str,
        status: ImplementedParityStatus,
        description: &'static str,
    ) -> Self {
        Self {
            label,
            status,
            description,
        }
    }

    pub fn current_items() -> Vec<Self> {
        vec![
            Self::new(
                "Mixer",
                ImplementedParityStatus::Implemented,
                "Dashboard, scenes, fader assignment, mute targets, scribble presets, monitor mix, and first-pass submix controls.",
            ),
            Self::new(
                "Routing",
                ImplementedParityStatus::Implemented,
                "Typed router matrix, live route-state badges, app-match routing rules, and visual routing-rule diff with safe apply actions.",
            ),
            Self::new(
                "Mic",
                ImplementedParityStatus::Partial,
                "Mic type/gain, gate, de-ess, compressor, EQ, safety controls, guarded mic profiles, and setup guidance; true live meter remains pending.",
            ),
            Self::new(
                "Effects",
                ImplementedParityStatus::Partial,
                "Quick presets, amounts, styles, expanded advanced DSP defaults, and guarded preset actions; arbitrary full editor/browser remains pending.",
            ),
            Self::new(
                "Lighting",
                ImplementedParityStatus::Partial,
                "Quick themes, colour editor, animation controls, and guarded colour-only profile load; dynamic profile browser remains pending.",
            ),
            Self::new(
                "Headphone EQ",
                ImplementedParityStatus::Partial,
                "Dedicated EQ page, compact 10-band editor, preamp, enable, and guarded profile slot controls.",
            ),
            Self::new(
                "Sampler",
                ImplementedParityStatus::Partial,
                "Bank selector, compact pad cards, playback/order controls, workflow settings, conservative trim reset, guarded typed-path add/remove, a simple audio-file browser, and live per-index sample rows; waveform/bulk editing remains deferred.",
            ),
            Self::new(
                "Profiles",
                ImplementedParityStatus::Partial,
                "guarded named-slot workflows exist for main, mic, effects, headphone EQ, and lighting; full dynamic browser remains pending.",
            ),
            Self::new(
                "Diagnostics",
                ImplementedParityStatus::Partial,
                "Read-only status, socket candidates, connection/device/profile visibility, plus recent in-app IPC event log; external daemon log tailing remains pending.",
            ),
        ]
    }

    pub fn label(&self) -> &str {
        self.label
    }

    pub fn status_label(&self) -> &'static str {
        self.status.label()
    }

    pub fn status(&self) -> ImplementedParityStatus {
        self.status
    }

    pub fn description(&self) -> &str {
        self.description
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightingProfileAction {
    label: String,
    description: &'static str,
    command: PersonalCommand,
}

impl LightingProfileAction {
    pub fn new(label: String, description: &'static str, command: PersonalCommand) -> Self {
        Self {
            label,
            description,
            command,
        }
    }

    pub fn guarded_daily_actions(profile: &str) -> Vec<Self> {
        vec![Self::new(
            format!("Load {profile} lighting"),
            "Load only lighting colours from the named profile without changing audio routing, mix, mic, or effects settings.",
            PersonalCommand::LoadProfileColours(profile.to_string()),
        )]
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn description(&self) -> &'static str {
        self.description
    }

    pub fn command(&self) -> PersonalCommand {
        self.command.clone()
    }

    pub fn requires_confirmation(&self) -> bool {
        true
    }

    pub fn command_if_confirmed(&self, confirmed: bool) -> Option<PersonalCommand> {
        confirmed.then(|| self.command())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MainProfileAction {
    label: String,
    description: &'static str,
    command: PersonalCommand,
}

impl MainProfileAction {
    pub fn new(label: String, description: &'static str, command: PersonalCommand) -> Self {
        Self {
            label,
            description,
            command,
        }
    }

    pub fn guarded_daily_actions(profile: &str) -> Vec<Self> {
        let profile_name = profile.to_string();
        vec![
            Self::new(
                format!("Load {profile}"),
                "Load the named full GoXLR profile, including hardware settings.",
                PersonalCommand::LoadProfile(profile_name.clone(), true),
            ),
            Self::new(
                "Save active".to_string(),
                "Overwrite the currently active full GoXLR profile with current settings.",
                PersonalCommand::SaveProfile,
            ),
            Self::new(
                format!("Save as {profile}"),
                "Save current full GoXLR settings into the named personal profile slot.",
                PersonalCommand::SaveProfileAs(profile_name.clone()),
            ),
            Self::new(
                format!("Create {profile}"),
                "Create the named personal full-profile slot if it does not already exist.",
                PersonalCommand::NewProfile(profile_name.clone()),
            ),
            Self::new(
                format!("Delete {profile}"),
                "Delete the named personal full-profile slot.",
                PersonalCommand::DeleteProfile(profile_name),
            ),
        ]
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn description(&self) -> &'static str {
        self.description
    }

    pub fn command(&self) -> PersonalCommand {
        self.command.clone()
    }

    pub fn requires_confirmation(&self) -> bool {
        true
    }

    pub fn command_if_confirmed(&self, confirmed: bool) -> Option<PersonalCommand> {
        confirmed.then(|| self.command())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileBrowserKind {
    Main,
    LightingColours,
    Mic,
    EffectsPreset,
    HeadphoneEq,
}

impl ProfileBrowserKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::Main => "Profile browser",
            Self::LightingColours => "Lighting profile browser",
            Self::Mic => "Mic profile browser",
            Self::EffectsPreset => "Effect preset browser",
            Self::HeadphoneEq => "Headphone EQ profile browser",
        }
    }

    pub fn empty_hint(self) -> &'static str {
        match self {
            Self::Main => "No .goxlr profiles found.",
            Self::LightingColours => "No .goxlr profiles found for lighting-only load.",
            Self::Mic => "No .goxlrMicProfile files found.",
            Self::EffectsPreset => "No .preset files found.",
            Self::HeadphoneEq => "No headphone EQ profiles found.",
        }
    }

    fn file_suffix(self) -> &'static str {
        match self {
            Self::Main | Self::LightingColours => ".goxlr",
            Self::Mic => ".goxlrMicProfile",
            Self::EffectsPreset => ".preset",
            Self::HeadphoneEq => ".goxlrHeadphoneProfile",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileBrowserAction {
    label: &'static str,
    command: PersonalCommand,
    requires_confirmation: bool,
}

impl ProfileBrowserAction {
    fn new(label: &'static str, command: PersonalCommand, requires_confirmation: bool) -> Self {
        Self {
            label,
            command,
            requires_confirmation,
        }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn command(&self) -> PersonalCommand {
        self.command.clone()
    }

    pub fn requires_confirmation(&self) -> bool {
        self.requires_confirmation
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileBrowserRow {
    kind: ProfileBrowserKind,
    name: String,
    active: bool,
}

impl ProfileBrowserRow {
    fn new(kind: ProfileBrowserKind, name: String, active: bool) -> Self {
        Self { kind, name, active }
    }

    pub fn kind(&self) -> ProfileBrowserKind {
        self.kind
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn actions(&self) -> Vec<ProfileBrowserAction> {
        let name = self.name.clone();
        match self.kind {
            ProfileBrowserKind::Main => vec![
                ProfileBrowserAction::new(
                    "Load",
                    PersonalCommand::LoadProfile(name.clone(), true),
                    true,
                ),
                ProfileBrowserAction::new(
                    "Load lighting",
                    PersonalCommand::LoadProfileColours(name.clone()),
                    true,
                ),
                ProfileBrowserAction::new(
                    "Save as",
                    PersonalCommand::SaveProfileAs(name.clone()),
                    true,
                ),
                ProfileBrowserAction::new("Delete", PersonalCommand::DeleteProfile(name), true),
            ],
            ProfileBrowserKind::LightingColours => vec![ProfileBrowserAction::new(
                "Load lighting",
                PersonalCommand::LoadProfileColours(name),
                true,
            )],
            ProfileBrowserKind::Mic => vec![
                ProfileBrowserAction::new(
                    "Load",
                    PersonalCommand::LoadMicProfile(name.clone(), true),
                    true,
                ),
                ProfileBrowserAction::new(
                    "Save as",
                    PersonalCommand::SaveMicProfileAs(name.clone()),
                    true,
                ),
                ProfileBrowserAction::new("Delete", PersonalCommand::DeleteMicProfile(name), true),
            ],
            ProfileBrowserKind::EffectsPreset => vec![
                ProfileBrowserAction::new(
                    "Load",
                    PersonalCommand::LoadEffectPreset(name.clone()),
                    true,
                ),
                ProfileBrowserAction::new(
                    "Rename active",
                    PersonalCommand::RenameActiveEffectPreset(name),
                    true,
                ),
                ProfileBrowserAction::new(
                    "Save active",
                    PersonalCommand::SaveActiveEffectPreset,
                    true,
                ),
            ],
            ProfileBrowserKind::HeadphoneEq => vec![
                ProfileBrowserAction::new(
                    "Load",
                    PersonalCommand::LoadHeadphoneEqProfile(name.clone()),
                    true,
                ),
                ProfileBrowserAction::new(
                    "Save as",
                    PersonalCommand::SaveHeadphoneEqProfile(name.clone()),
                    true,
                ),
                ProfileBrowserAction::new(
                    "Delete",
                    PersonalCommand::DeleteHeadphoneEqProfile(name),
                    true,
                ),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfileBrowser {
    kind: ProfileBrowserKind,
    rows: Vec<ProfileBrowserRow>,
}

impl ProfileBrowser {
    pub fn from_names(
        kind: ProfileBrowserKind,
        active_name: Option<&str>,
        mut names: Vec<String>,
    ) -> Self {
        names.sort_by_key(|name| name.to_ascii_lowercase());
        names.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        let rows = names
            .into_iter()
            .map(|name| {
                let active = active_name.is_some_and(|active| active.eq_ignore_ascii_case(&name));
                ProfileBrowserRow::new(kind, name, active)
            })
            .collect();
        Self { kind, rows }
    }

    pub fn from_directory(
        kind: ProfileBrowserKind,
        active_name: Option<&str>,
        path: Option<&Path>,
    ) -> Self {
        let Some(path) = path else {
            return Self::from_names(kind, active_name, Vec::new());
        };
        let suffix = kind.file_suffix();
        let mut names = Vec::new();
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if !file_type.is_file() {
                    continue;
                }
                let Some(file_name) = entry.file_name().to_str().map(str::to_string) else {
                    continue;
                };
                if let Some(name) = file_name.strip_suffix(suffix) {
                    if name.trim().is_empty() {
                        continue;
                    }
                    names.push(name.to_string());
                }
            }
        }
        Self::from_names(kind, active_name, names)
    }

    pub fn title(&self) -> &'static str {
        self.kind.title()
    }

    pub fn kind(&self) -> ProfileBrowserKind {
        self.kind
    }

    pub fn rows(&self) -> &[ProfileBrowserRow] {
        &self.rows
    }

    pub fn empty_hint(&self) -> &'static str {
        self.kind.empty_hint()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemSettingsAction {
    label: &'static str,
    description: &'static str,
    command: PersonalCommand,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemSettingsSnapshot {
    mute_hold_duration_ms: u16,
    vc_mute_also_mute_chat_mic: bool,
    monitor_with_fx_enabled: bool,
    lock_faders_enabled: bool,
    vod_mode: VodMode,
}

impl SystemSettingsSnapshot {
    pub fn new(
        mute_hold_duration_ms: u16,
        vc_mute_also_mute_chat_mic: bool,
        monitor_with_fx_enabled: bool,
        lock_faders_enabled: bool,
        vod_mode: VodMode,
    ) -> Self {
        Self {
            mute_hold_duration_ms,
            vc_mute_also_mute_chat_mic,
            monitor_with_fx_enabled,
            lock_faders_enabled,
            vod_mode,
        }
    }

    pub fn mute_hold_duration_ms(&self) -> u16 {
        self.mute_hold_duration_ms
    }

    pub fn vc_mute_also_mute_chat_mic(&self) -> bool {
        self.vc_mute_also_mute_chat_mic
    }

    pub fn monitor_with_fx_enabled(&self) -> bool {
        self.monitor_with_fx_enabled
    }

    pub fn lock_faders_enabled(&self) -> bool {
        self.lock_faders_enabled
    }

    pub fn vod_mode(&self) -> VodMode {
        self.vod_mode
    }

    pub fn rows(&self) -> Vec<SystemSettingsStatusRow> {
        vec![
            SystemSettingsStatusRow::new(
                "Mute hold",
                format!("{} ms", self.mute_hold_duration_ms),
                "Current daemon-reported hold time for mute-style buttons.",
            ),
            SystemSettingsStatusRow::new(
                "VC mute links Chat Mic",
                on_off(self.vc_mute_also_mute_chat_mic),
                "Whether muting voice chat also mutes the Chat Mic output.",
            ),
            SystemSettingsStatusRow::new(
                "Monitor with FX",
                on_off(self.monitor_with_fx_enabled),
                "Whether direct microphone monitoring includes voice effects.",
            ),
            SystemSettingsStatusRow::new(
                "Lock faders",
                on_off(self.lock_faders_enabled),
                "Whether hardware faders are locked against movement.",
            ),
            SystemSettingsStatusRow::new(
                "VOD mode",
                format!("{:?}", self.vod_mode),
                "Current VOD routing mode reported by daemon settings.",
            ),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemSettingsStatusRow {
    label: &'static str,
    value: String,
    description: &'static str,
}

impl SystemSettingsStatusRow {
    pub fn new(label: &'static str, value: impl Into<String>, description: &'static str) -> Self {
        Self {
            label,
            value: value.into(),
            description,
        }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn description(&self) -> &'static str {
        self.description
    }
}

fn on_off(enabled: bool) -> String {
    if enabled { "On" } else { "Off" }.to_string()
}

impl SystemSettingsAction {
    pub fn new(label: &'static str, description: &'static str, command: PersonalCommand) -> Self {
        Self {
            label,
            description,
            command,
        }
    }

    pub fn daily_controls() -> Vec<Self> {
        vec![
            Self::new(
                "Hold 250ms",
                "Short cough-button hold duration for fast push-to-mute use.",
                PersonalCommand::SetMuteHoldDuration(250),
            ),
            Self::new(
                "Hold 500ms",
                "Balanced cough-button hold duration for daily use.",
                PersonalCommand::SetMuteHoldDuration(500),
            ),
            Self::new(
                "Hold 1000ms",
                "Longer hold duration to avoid accidental mute changes.",
                PersonalCommand::SetMuteHoldDuration(1000),
            ),
            Self::new(
                "Cough hold mode",
                "Require holding the cough button while muting.",
                PersonalCommand::SetCoughIsHold(true),
            ),
            Self::new(
                "Cough toggle mode",
                "Make the cough button toggle mute state with each press.",
                PersonalCommand::SetCoughIsHold(false),
            ),
            Self::new(
                "Cough mutes VC",
                "Route the cough button to mute the voice-chat output.",
                PersonalCommand::SetCoughMuteFunction(MuteFunction::ToVoiceChat),
            ),
            Self::new(
                "Cough mutes stream",
                "Route the cough button to mute only the stream mix.",
                PersonalCommand::SetCoughMuteFunction(MuteFunction::ToStream),
            ),
            Self::new(
                "Cough mutes all",
                "Route the cough button to mute all microphone destinations.",
                PersonalCommand::SetCoughMuteFunction(MuteFunction::All),
            ),
            Self::new(
                "Cough mutes phones",
                "Route the cough button to mute the headphone monitor.",
                PersonalCommand::SetCoughMuteFunction(MuteFunction::ToPhones),
            ),
            Self::new(
                "VC mutes chat mic",
                "Also mute Chat Mic output when the voice-chat mute is active.",
                PersonalCommand::SetVCMuteAlsoMuteCM(true),
            ),
            Self::new(
                "VC leaves chat mic",
                "Keep Chat Mic independent from the voice-chat mute.",
                PersonalCommand::SetVCMuteAlsoMuteCM(false),
            ),
            Self::new(
                "Monitor FX on",
                "Hear voice effects in the monitor/headphone feed.",
                PersonalCommand::SetMonitorWithFx(true),
            ),
            Self::new(
                "Monitor FX off",
                "Monitor the dry microphone without voice effects.",
                PersonalCommand::SetMonitorWithFx(false),
            ),
            Self::new(
                "Lock faders",
                "Prevent hardware fader moves from changing levels.",
                PersonalCommand::SetLockFaders(true),
            ),
            Self::new(
                "Unlock faders",
                "Allow normal hardware fader level changes.",
                PersonalCommand::SetLockFaders(false),
            ),
            Self::new(
                "VOD routable",
                "Use the normal routable VOD output mode.",
                PersonalCommand::SetVodMode(VodMode::Routable),
            ),
            Self::new(
                "VOD no music",
                "Exclude Music from the VOD mix for stream-safe archives.",
                PersonalCommand::SetVodMode(VodMode::StreamNoMusic),
            ),
            Self::new(
                "Reload settings",
                "Reload daemon/device settings after manual config changes.",
                PersonalCommand::ReloadSettings,
            ),
        ]
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn description(&self) -> &'static str {
        self.description
    }

    pub fn command(&self) -> PersonalCommand {
        self.command.clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightingLayoutPolicy;

impl LightingLayoutPolicy {
    pub fn quick_theme_target_columns() -> usize {
        4
    }

    pub fn quick_theme_card_width() -> f32 {
        150.0
    }

    pub fn quick_theme_card_height() -> f32 {
        116.0
    }

    pub fn quick_theme_card_width_for_available_width(available_width: f32) -> f32 {
        let columns = Self::quick_theme_target_columns() as f32;
        let total_gap = Self::panel_gap() * (columns - 1.0);
        let frame_margin_allowance = 18.0;
        let fitted = ((available_width - total_gap) / columns) - frame_margin_allowance;
        fitted.clamp(132.0, Self::quick_theme_card_width())
    }

    pub fn compact_editor_panel_width() -> f32 {
        320.0
    }

    pub fn editor_intro_width() -> f32 {
        960.0
    }

    pub fn uses_dense_editor_flow() -> bool {
        true
    }

    pub fn theme_row_stays_compact_in_wide_windows() -> bool {
        true
    }

    pub fn animation_control_grid_columns() -> usize {
        2
    }

    pub fn wide_editor_panel_width() -> f32 {
        360.0
    }

    pub fn balanced_editor_row_width() -> f32 {
        Self::wide_editor_panel_width() * 3.0 + Self::panel_gap() * 2.0
    }

    pub fn profile_panel_width() -> f32 {
        420.0
    }

    pub fn profile_button_width() -> f32 {
        170.0
    }

    pub fn uses_guarded_profile_colour_actions() -> bool {
        true
    }

    pub fn panel_gap() -> f32 {
        8.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectsLayoutPolicy;

impl EffectsLayoutPolicy {
    pub fn quick_preset_target_columns() -> usize {
        4
    }

    pub fn quick_preset_card_width() -> f32 {
        180.0
    }

    pub fn quick_preset_card_height() -> f32 {
        112.0
    }

    pub fn quick_preset_inner_width() -> f32 {
        Self::quick_preset_card_width() - 24.0
    }

    pub fn quick_preset_inner_height() -> f32 {
        Self::quick_preset_card_height() - 24.0
    }

    pub fn quick_preset_row_cross_align() -> egui::Align {
        egui::Align::Min
    }

    pub fn quick_preset_cards_share_height() -> bool {
        true
    }

    pub fn quick_preset_command_label_min_width() -> f32 {
        72.0
    }

    pub fn quick_preset_card_width_for_available_width(available_width: f32) -> f32 {
        let columns = Self::quick_preset_target_columns() as f32;
        let total_gap = Self::detail_panel_gap() * (columns - 1.0);
        let fitted = (available_width - total_gap) / columns;
        fitted.clamp(168.0, Self::quick_preset_card_width())
    }

    pub fn amount_panel_width() -> f32 {
        340.0
    }

    pub fn style_panel_width() -> f32 {
        700.0
    }

    pub fn style_group_card_width() -> f32 {
        170.0
    }

    pub fn style_button_min_width() -> f32 {
        64.0
    }

    pub fn detail_panel_gap() -> f32 {
        8.0
    }

    pub fn preset_management_panel_width() -> f32 {
        520.0
    }

    pub fn preset_management_button_width() -> f32 {
        150.0
    }

    pub fn advanced_slider_width() -> f32 {
        180.0
    }

    pub fn uses_advanced_reverb_sliders() -> bool {
        true
    }

    pub fn uses_advanced_echo_sliders() -> bool {
        true
    }

    pub fn uses_advanced_pitch_sliders() -> bool {
        true
    }

    pub fn uses_advanced_megaphone_sliders() -> bool {
        true
    }

    pub fn uses_advanced_robot_sliders() -> bool {
        true
    }

    pub fn uses_advanced_hard_tune_sliders() -> bool {
        true
    }

    pub fn uses_guarded_preset_management() -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicLayoutPolicy;

impl MicLayoutPolicy {
    pub fn panel_width() -> f32 {
        360.0
    }

    pub fn eq_panel_width() -> f32 {
        720.0
    }

    pub fn profile_panel_width() -> f32 {
        360.0
    }

    pub fn panel_gap() -> f32 {
        8.0
    }

    pub fn slider_width() -> f32 {
        190.0
    }

    pub fn eq_slider_width() -> f32 {
        120.0
    }

    pub fn uses_wrapped_panels() -> bool {
        true
    }

    pub fn setup_guide_panel_width() -> f32 {
        420.0
    }

    pub fn uses_setup_guidance_cards() -> bool {
        true
    }

    pub fn meter_placeholder_is_read_only() -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicSetupGuideStep {
    label: &'static str,
    description: &'static str,
}

impl MicSetupGuideStep {
    pub fn daily_steps() -> Vec<Self> {
        vec![
            Self {
                label: "1. Pick mic type",
                description: "Choose Dynamic, Condenser, or Jack before setting gain so the gain command targets the correct preamp mode.",
            },
            Self {
                label: "2. Set gain before processing",
                description: "Raise mic gain until normal speech has healthy peaks, then leave headroom before adding gate or compressor changes.",
            },
            Self {
                label: "3. Close the gate gently",
                description: "Use Gate threshold first, then attenuation, attack, and release so room noise closes without chopping word starts.",
            },
            Self {
                label: "4. Add compression last",
                description: "Set threshold and ratio for consistency, then use makeup gain after compression rather than chasing loudness with preamp gain.",
            },
        ]
    }

    pub fn live_meter_status_note() -> &'static str {
        "Live mic metering is not exposed in the current IPC snapshot; this setup guide is read-only until a reliable level source is available."
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn description(&self) -> &'static str {
        self.description
    }

    pub fn command(&self) -> Option<PersonalCommand> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MicEqBandControl {
    Mini {
        label: &'static str,
        band: MiniEqFrequencies,
    },
    Full {
        label: &'static str,
        band: EqFrequencies,
    },
}

impl MicEqBandControl {
    pub fn mini_bands() -> Vec<Self> {
        vec![
            Self::Mini {
                label: "90 Hz",
                band: MiniEqFrequencies::Equalizer90Hz,
            },
            Self::Mini {
                label: "250 Hz",
                band: MiniEqFrequencies::Equalizer250Hz,
            },
            Self::Mini {
                label: "500 Hz",
                band: MiniEqFrequencies::Equalizer500Hz,
            },
            Self::Mini {
                label: "1 kHz",
                band: MiniEqFrequencies::Equalizer1KHz,
            },
            Self::Mini {
                label: "3 kHz",
                band: MiniEqFrequencies::Equalizer3KHz,
            },
            Self::Mini {
                label: "8 kHz",
                band: MiniEqFrequencies::Equalizer8KHz,
            },
        ]
    }

    pub fn full_bands() -> Vec<Self> {
        vec![
            Self::Full {
                label: "31 Hz",
                band: EqFrequencies::Equalizer31Hz,
            },
            Self::Full {
                label: "63 Hz",
                band: EqFrequencies::Equalizer63Hz,
            },
            Self::Full {
                label: "125 Hz",
                band: EqFrequencies::Equalizer125Hz,
            },
            Self::Full {
                label: "250 Hz",
                band: EqFrequencies::Equalizer250Hz,
            },
            Self::Full {
                label: "500 Hz",
                band: EqFrequencies::Equalizer500Hz,
            },
            Self::Full {
                label: "1 kHz",
                band: EqFrequencies::Equalizer1KHz,
            },
            Self::Full {
                label: "2 kHz",
                band: EqFrequencies::Equalizer2KHz,
            },
            Self::Full {
                label: "4 kHz",
                band: EqFrequencies::Equalizer4KHz,
            },
            Self::Full {
                label: "8 kHz",
                band: EqFrequencies::Equalizer8KHz,
            },
            Self::Full {
                label: "16 kHz",
                band: EqFrequencies::Equalizer16KHz,
            },
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Mini { label, .. } | Self::Full { label, .. } => label,
        }
    }

    pub fn default_frequency_hz(self) -> f32 {
        match self {
            Self::Mini { band, .. } => match band {
                MiniEqFrequencies::Equalizer90Hz => 90.0,
                MiniEqFrequencies::Equalizer250Hz => 250.0,
                MiniEqFrequencies::Equalizer500Hz => 500.0,
                MiniEqFrequencies::Equalizer1KHz => 1000.0,
                MiniEqFrequencies::Equalizer3KHz => 3000.0,
                MiniEqFrequencies::Equalizer8KHz => 8000.0,
            },
            Self::Full { band, .. } => match band {
                EqFrequencies::Equalizer31Hz => 31.0,
                EqFrequencies::Equalizer63Hz => 63.0,
                EqFrequencies::Equalizer125Hz => 125.0,
                EqFrequencies::Equalizer250Hz => 250.0,
                EqFrequencies::Equalizer500Hz => 500.0,
                EqFrequencies::Equalizer1KHz => 1000.0,
                EqFrequencies::Equalizer2KHz => 2000.0,
                EqFrequencies::Equalizer4KHz => 4000.0,
                EqFrequencies::Equalizer8KHz => 8000.0,
                EqFrequencies::Equalizer16KHz => 16_000.0,
            },
        }
    }

    pub fn gain_command(self, gain: i8) -> PersonalCommand {
        match self {
            Self::Mini { band, .. } => PersonalCommand::SetEqMiniGain(band, gain),
            Self::Full { band, .. } => PersonalCommand::SetEqGain(band, gain),
        }
    }

    pub fn frequency_command(self, frequency: f32) -> PersonalCommand {
        match self {
            Self::Mini { band, .. } => PersonalCommand::SetEqMiniFreq(band, frequency),
            Self::Full { band, .. } => PersonalCommand::SetEqFreq(band, frequency),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MicProfileAction {
    label: String,
    command: PersonalCommand,
    requires_confirmation: bool,
}

impl MicProfileAction {
    pub fn guarded_daily_actions(profile: &str) -> Vec<Self> {
        vec![
            Self::new(
                format!("Load {profile}"),
                PersonalCommand::LoadMicProfile(profile.to_string(), true),
                true,
            ),
            Self::new(
                "Save current".to_string(),
                PersonalCommand::SaveMicProfile,
                true,
            ),
            Self::new(
                format!("Save as {profile}"),
                PersonalCommand::SaveMicProfileAs(profile.to_string()),
                true,
            ),
            Self::new(
                format!("Delete {profile}"),
                PersonalCommand::DeleteMicProfile(profile.to_string()),
                true,
            ),
        ]
    }

    fn new(label: String, command: PersonalCommand, requires_confirmation: bool) -> Self {
        Self {
            label,
            command,
            requires_confirmation,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn command(&self) -> PersonalCommand {
        self.command.clone()
    }
    pub fn requires_confirmation(&self) -> bool {
        self.requires_confirmation
    }

    pub fn command_if_confirmed(&self, confirmed: bool) -> Option<PersonalCommand> {
        if self.requires_confirmation && !confirmed {
            None
        } else {
            Some(self.command())
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectPresetAction {
    label: String,
    command: PersonalCommand,
    requires_confirmation: bool,
}

impl EffectPresetAction {
    pub fn guarded_daily_actions(preset: &str) -> Vec<Self> {
        vec![
            Self::new(
                format!("Load {preset}"),
                PersonalCommand::LoadEffectPreset(preset.to_string()),
                true,
            ),
            Self::new(
                format!("Rename to {preset}"),
                PersonalCommand::RenameActiveEffectPreset(preset.to_string()),
                true,
            ),
            Self::new(
                "Save active".to_string(),
                PersonalCommand::SaveActiveEffectPreset,
                true,
            ),
        ]
    }

    fn new(label: String, command: PersonalCommand, requires_confirmation: bool) -> Self {
        Self {
            label,
            command,
            requires_confirmation,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn command(&self) -> PersonalCommand {
        self.command.clone()
    }

    pub fn requires_confirmation(&self) -> bool {
        self.requires_confirmation
    }

    pub fn command_if_confirmed(&self, confirmed: bool) -> Option<PersonalCommand> {
        if self.requires_confirmation && !confirmed {
            None
        } else {
            Some(self.command())
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectsAdvancedControl {
    label: &'static str,
    default_command: PersonalCommand,
}

impl EffectsAdvancedControl {
    pub fn daily_controls() -> Vec<Self> {
        vec![
            Self::new("Reverb decay", PersonalCommand::SetReverbDecay(1500)),
            Self::new(
                "Reverb early level",
                PersonalCommand::SetReverbEarlyLevel(0),
            ),
            Self::new("Reverb tail level", PersonalCommand::SetReverbTailLevel(0)),
            Self::new("Reverb pre-delay", PersonalCommand::SetReverbPreDelay(25)),
            Self::new("Reverb low colour", PersonalCommand::SetReverbLowColour(0)),
            Self::new(
                "Reverb high colour",
                PersonalCommand::SetReverbHighColour(0),
            ),
            Self::new(
                "Reverb high factor",
                PersonalCommand::SetReverbHighFactor(0),
            ),
            Self::new("Reverb diffuse", PersonalCommand::SetReverbDiffuse(0)),
            Self::new("Reverb mod speed", PersonalCommand::SetReverbModSpeed(0)),
            Self::new("Reverb mod depth", PersonalCommand::SetReverbModDepth(0)),
            Self::new("Echo feedback", PersonalCommand::SetEchoFeedback(35)),
            Self::new("Echo tempo", PersonalCommand::SetEchoTempo(120)),
            Self::new("Echo left delay", PersonalCommand::SetEchoDelayLeft(250)),
            Self::new("Echo right delay", PersonalCommand::SetEchoDelayRight(375)),
            Self::new(
                "Echo left feedback",
                PersonalCommand::SetEchoFeedbackLeft(35),
            ),
            Self::new(
                "Echo right feedback",
                PersonalCommand::SetEchoFeedbackRight(35),
            ),
            Self::new("Echo cross L→R", PersonalCommand::SetEchoFeedbackXFBLtoR(0)),
            Self::new("Echo cross R→L", PersonalCommand::SetEchoFeedbackXFBRtoL(0)),
            Self::new("Pitch character", PersonalCommand::SetPitchCharacter(50)),
            Self::new(
                "Megaphone post gain",
                PersonalCommand::SetMegaphonePostGain(0),
            ),
            Self::new("Robot threshold", PersonalCommand::SetRobotThreshold(-36)),
            Self::new(
                "Robot low gain",
                PersonalCommand::SetRobotGain(RobotRange::Low, 0),
            ),
            Self::new(
                "Robot mid frequency",
                PersonalCommand::SetRobotFreq(RobotRange::Medium, 60),
            ),
            Self::new(
                "Robot high width",
                PersonalCommand::SetRobotWidth(RobotRange::High, 50),
            ),
            Self::new("Robot waveform", PersonalCommand::SetRobotWaveform(0)),
            Self::new("Robot pulse width", PersonalCommand::SetRobotPulseWidth(50)),
            Self::new("Robot dry mix", PersonalCommand::SetRobotDryMix(0)),
            Self::new("Hard Tune amount", PersonalCommand::SetHardTuneAmount(50)),
            Self::new("Hard Tune rate", PersonalCommand::SetHardTuneRate(50)),
            Self::new("Hard Tune window", PersonalCommand::SetHardTuneWindow(200)),
            Self::new(
                "Hard Tune source",
                PersonalCommand::SetHardTuneSource(HardTuneSource::Music),
            ),
        ]
    }

    fn new(label: &'static str, default_command: PersonalCommand) -> Self {
        Self {
            label,
            default_command,
        }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }
    pub fn command_for_default(&self) -> PersonalCommand {
        self.default_command.clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadphoneEqLayoutPolicy;

impl HeadphoneEqLayoutPolicy {
    pub fn panel_width() -> f32 {
        640.0
    }
    pub fn grid_columns() -> usize {
        5
    }
    pub fn grid_rows_for_band_count(band_count: usize) -> usize {
        band_count.div_ceil(Self::grid_columns())
    }
    pub fn uses_fixed_grid_rows() -> bool {
        true
    }
    pub fn uses_equal_height_band_cards() -> bool {
        true
    }
    pub fn band_card_width() -> f32 {
        112.0
    }
    pub fn band_card_height() -> f32 {
        126.0
    }
    pub fn band_card_gap() -> f32 {
        10.0
    }
    pub fn uses_guarded_profile_actions() -> bool {
        true
    }
    pub fn profile_panel_width() -> f32 {
        Self::panel_width()
    }
    pub fn profile_button_width() -> f32 {
        150.0
    }
    pub fn uses_listening_presets() -> bool {
        true
    }
    pub fn preset_panel_width() -> f32 {
        Self::panel_width()
    }
    pub fn preset_button_width() -> f32 {
        144.0
    }
    pub fn preset_card_height() -> f32 {
        104.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeadphoneListeningPreset {
    name: &'static str,
    description: &'static str,
    headphone_volume: u8,
    limiter_threshold: u8,
    preamp_db: f32,
    band_gains: [f32; 10],
    safety_preset: bool,
}

impl HeadphoneListeningPreset {
    pub fn daily_presets() -> Vec<Self> {
        vec![
            Self::new(
                "Neutral Base",
                "Flat, gain-staged hardware EQ with limiter protection for judging source audio.",
                72,
                85,
                -3.0,
                [0.0; 10],
                false,
            ),
            Self::new(
                "Music Detail",
                "Slight low/high lift with conservative preamp headroom for enjoyable music listening.",
                76,
                88,
                -4.0,
                [1.5, 0.5, 0.0, -0.5, 0.0, 0.5, 1.0, 1.5, 1.0, 0.5],
                false,
            ),
            Self::new(
                "Game Imaging",
                "Reduced rumble and boosted presence/air to make positional cues easier to hear.",
                74,
                86,
                -5.0,
                [-3.0, -2.0, -1.0, 0.0, 0.5, 1.0, 2.0, 3.0, 2.0, 0.5],
                false,
            ),
            Self::new(
                "Night Safe",
                "Lower output, lower limiter ceiling, and darker EQ for fatigue-safe late listening.",
                55,
                72,
                -6.0,
                [-1.0, -1.0, -0.5, 0.0, 0.0, 0.0, -0.5, -1.0, -2.0, -3.0],
                true,
            ),
        ]
    }

    fn new(
        name: &'static str,
        description: &'static str,
        headphone_volume: u8,
        limiter_threshold: u8,
        preamp_db: f32,
        band_gains: [f32; 10],
        safety_preset: bool,
    ) -> Self {
        Self {
            name,
            description,
            headphone_volume,
            limiter_threshold,
            preamp_db,
            band_gains,
            safety_preset,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn description(&self) -> &'static str {
        self.description
    }
    pub fn is_safety_preset(&self) -> bool {
        self.safety_preset
    }
    pub fn commands(&self) -> Vec<PersonalCommand> {
        let mut commands = vec![
            PersonalCommand::SetMonitorMix(OutputDevice::Headphones),
            PersonalCommand::SetVolume(ChannelName::Headphones, self.headphone_volume),
            PersonalCommand::SetHeadphoneLimiterEnabled(true),
            PersonalCommand::SetHeadphoneLimiterThreshold(self.limiter_threshold),
            PersonalCommand::SetHeadphoneEqEnabled(true),
            PersonalCommand::SetHeadphoneEqPreamp(self.preamp_db),
        ];
        for (index, gain) in self.band_gains.iter().copied().enumerate() {
            commands.push(PersonalCommand::SetHeadphoneEqBandGain(index as u8, gain));
        }
        for band in HeadphoneEqBandControl::ten_band_editor() {
            commands.push(band.frequency_command(band.default_frequency_hz()));
            commands.push(band.q_command(0.9));
        }
        commands
    }
    pub fn to_scene(&self) -> UiScene {
        UiScene::new(self.name, self.commands())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadphoneAudioStep {
    label: &'static str,
    description: &'static str,
}

impl HeadphoneAudioStep {
    pub fn recommended_steps() -> Vec<Self> {
        vec![
            Self::new(
                "1. Route intentionally",
                "Confirm the app/source is routed to GoXLR and the monitored output is Headphones before tuning.",
            ),
            Self::new(
                "2. Gain-stage first",
                "Set headphone volume for comfort, then use negative EQ preamp headroom before boosting bands.",
            ),
            Self::new(
                "3. Enable limiter",
                "Keep the hardware headphone limiter on; raise the threshold only after checking loud material.",
            ),
            Self::new(
                "4. Tune by purpose",
                "Use Neutral for judgement, Music for enjoyment, Game for cues, and Night Safe for fatigue control.",
            ),
            Self::new(
                "5. Save after listening",
                "After real listening, save the chosen EQ as a named Headphone EQ profile for recall.",
            ),
        ]
    }

    const fn new(label: &'static str, description: &'static str) -> Self {
        Self { label, description }
    }
    pub fn label(&self) -> &'static str {
        self.label
    }
    pub fn description(&self) -> &'static str {
        self.description
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeadphoneEqProfileAction {
    label: String,
    command: PersonalCommand,
    requires_confirmation: bool,
}

impl HeadphoneEqProfileAction {
    pub fn guarded_daily_actions(profile: &str) -> Vec<Self> {
        vec![
            Self::new(
                format!("Load {profile}"),
                PersonalCommand::LoadHeadphoneEqProfile(profile.to_string()),
                true,
            ),
            Self::new(
                format!("Save as {profile}"),
                PersonalCommand::SaveHeadphoneEqProfile(profile.to_string()),
                true,
            ),
            Self::new(
                format!("Delete {profile}"),
                PersonalCommand::DeleteHeadphoneEqProfile(profile.to_string()),
                true,
            ),
        ]
    }

    fn new(label: String, command: PersonalCommand, requires_confirmation: bool) -> Self {
        Self {
            label,
            command,
            requires_confirmation,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn command(&self) -> PersonalCommand {
        self.command.clone()
    }
    pub fn requires_confirmation(&self) -> bool {
        self.requires_confirmation
    }
    pub fn command_if_confirmed(&self, confirmed: bool) -> Option<PersonalCommand> {
        if self.requires_confirmation && !confirmed {
            None
        } else {
            Some(self.command())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadphoneEqBandControl {
    index: u8,
    label: &'static str,
    default_frequency_hz: f32,
}

impl HeadphoneEqBandControl {
    pub fn ten_band_editor() -> Vec<Self> {
        [
            ("31 Hz", 31.0),
            ("63 Hz", 63.0),
            ("125 Hz", 125.0),
            ("250 Hz", 250.0),
            ("500 Hz", 500.0),
            ("1 kHz", 1000.0),
            ("2 kHz", 2000.0),
            ("4 kHz", 4000.0),
            ("8 kHz", 8000.0),
            ("16 kHz", 16_000.0),
        ]
        .iter()
        .enumerate()
        .map(|(index, (label, default_frequency_hz))| Self {
            index: index as u8,
            label,
            default_frequency_hz: *default_frequency_hz,
        })
        .collect()
    }

    pub fn index(self) -> u8 {
        self.index
    }
    pub fn label(self) -> &'static str {
        self.label
    }
    pub fn default_frequency_hz(self) -> f32 {
        self.default_frequency_hz
    }
    pub fn gain_command(self, gain: f32) -> PersonalCommand {
        PersonalCommand::SetHeadphoneEqBandGain(self.index, gain)
    }
    pub fn frequency_command(self, frequency: f32) -> PersonalCommand {
        PersonalCommand::SetHeadphoneEqBandFrequency(self.index, frequency)
    }
    pub fn q_command(self, q: f32) -> PersonalCommand {
        PersonalCommand::SetHeadphoneEqBandQ(self.index, q)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SamplerLayoutPolicy;

impl SamplerLayoutPolicy {
    pub fn panel_width() -> f32 {
        360.0
    }
    pub fn uses_bank_button_cards() -> bool {
        true
    }
    pub fn uses_two_by_two_slot_grid() -> bool {
        true
    }
    pub fn bank_slot_columns() -> usize {
        2
    }
    pub fn bank_slot_rows() -> usize {
        2
    }
    pub fn bank_slot_card_width() -> f32 {
        156.0
    }
    pub fn bank_slot_card_height() -> f32 {
        132.0
    }
    pub fn bank_slot_gap() -> f32 {
        8.0
    }
    pub fn exposes_file_import_controls() -> bool {
        true
    }

    pub fn file_workflow_panel_width() -> f32 {
        420.0
    }

    pub fn file_workflow_button_width() -> f32 {
        120.0
    }

    pub fn sample_browser_panel_width() -> f32 {
        420.0
    }

    pub fn sample_browser_row_button_width() -> f32 {
        84.0
    }

    pub fn exposes_custom_trim_editor() -> bool {
        true
    }

    pub fn custom_trim_slider_width() -> f32 {
        150.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SamplerAction {
    label: &'static str,
    command: PersonalCommand,
}

impl SamplerAction {
    pub fn daily_bank_actions(bank: SampleBank, button: SampleButtons) -> Vec<Self> {
        vec![
            Self::new("Select bank", PersonalCommand::SetActiveSamplerBank(bank)),
            Self::new(
                "Play / stop",
                PersonalCommand::SetSamplerFunction(bank, button, SamplePlaybackMode::PlayStop),
            ),
            Self::new(
                "Random order",
                PersonalCommand::SetSamplerOrder(bank, button, SamplePlayOrder::Random),
            ),
            Self::new("Play next", PersonalCommand::PlayNextSample(bank, button)),
            Self::new("Stop", PersonalCommand::StopSamplePlayback(bank, button)),
        ]
    }

    fn new(label: &'static str, command: PersonalCommand) -> Self {
        Self { label, command }
    }
    pub fn label(&self) -> &'static str {
        self.label
    }
    pub fn command(&self) -> PersonalCommand {
        self.command.clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SamplerWorkflowSetting {
    label: &'static str,
    description: &'static str,
    command: PersonalCommand,
}

impl SamplerWorkflowSetting {
    pub fn safe_settings() -> Vec<Self> {
        vec![
            Self::new(
                "Clear process error",
                "Reset the daemon's last sample-processing error after reviewing it.",
                PersonalCommand::ClearSampleProcessError,
            ),
            Self::new(
                "Reset on clear: on",
                "Return sampler buttons to a reset state when cleared.",
                PersonalCommand::SetSamplerResetOnClear(true),
            ),
            Self::new(
                "Reset on clear: off",
                "Keep sampler state stable when clearing sample slots.",
                PersonalCommand::SetSamplerResetOnClear(false),
            ),
            Self::new(
                "Fade 250 ms",
                "Use a short fade for sampler playback changes.",
                PersonalCommand::SetSamplerFadeDuration(250),
            ),
        ]
    }

    fn new(label: &'static str, description: &'static str, command: PersonalCommand) -> Self {
        Self {
            label,
            description,
            command,
        }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn description(&self) -> &'static str {
        self.description
    }

    pub fn command(&self) -> PersonalCommand {
        self.command.clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SampleTrimAction {
    label: &'static str,
    command: PersonalCommand,
}

impl SampleTrimAction {
    pub fn safe_trim_actions(
        bank: SampleBank,
        button: SampleButtons,
        sample_index: usize,
    ) -> Vec<Self> {
        vec![
            Self::new(
                "Start 0%",
                PersonalCommand::SetSampleStartPercent(bank, button, sample_index, 0.0),
            ),
            Self::new(
                "Start 25%",
                PersonalCommand::SetSampleStartPercent(bank, button, sample_index, 25.0),
            ),
            Self::new(
                "Start 50%",
                PersonalCommand::SetSampleStartPercent(bank, button, sample_index, 50.0),
            ),
            Self::new(
                "Stop 50%",
                PersonalCommand::SetSampleStopPercent(bank, button, sample_index, 50.0),
            ),
            Self::new(
                "Stop 75%",
                PersonalCommand::SetSampleStopPercent(bank, button, sample_index, 75.0),
            ),
            Self::new(
                "Stop 100%",
                PersonalCommand::SetSampleStopPercent(bank, button, sample_index, 100.0),
            ),
        ]
    }

    fn new(label: &'static str, command: PersonalCommand) -> Self {
        Self { label, command }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn command(&self) -> PersonalCommand {
        self.command.clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SampleTrimEditor {
    bank: SampleBank,
    button: SampleButtons,
    sample_index: usize,
    start_pct: f32,
    stop_pct: f32,
}

impl SampleTrimEditor {
    pub fn new(
        bank: SampleBank,
        button: SampleButtons,
        sample_index: usize,
        start_pct: f32,
        stop_pct: f32,
    ) -> Self {
        Self {
            bank,
            button,
            sample_index,
            start_pct: start_pct.clamp(0.0, 100.0),
            stop_pct: stop_pct.clamp(0.0, 100.0),
        }
    }

    pub fn start_pct(&self) -> f32 {
        self.start_pct
    }

    pub fn stop_pct(&self) -> f32 {
        self.stop_pct
    }

    pub fn start_label(&self) -> String {
        format!("Start {:.1}%", self.start_pct)
    }

    pub fn stop_label(&self) -> String {
        format!("Stop {:.1}%", self.stop_pct)
    }

    pub fn clamp_percent(&self, value: f32) -> f32 {
        value.clamp(0.0, 100.0)
    }

    pub fn start_command(&self, percent: f32) -> PersonalCommand {
        PersonalCommand::SetSampleStartPercent(
            self.bank,
            self.button,
            self.sample_index,
            self.clamp_percent(percent),
        )
    }

    pub fn stop_command(&self, percent: f32) -> PersonalCommand {
        PersonalCommand::SetSampleStopPercent(
            self.bank,
            self.button,
            self.sample_index,
            self.clamp_percent(percent),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SamplerLoadedSample {
    index: usize,
    name: String,
    start_pct: f32,
    stop_pct: f32,
}

impl SamplerLoadedSample {
    pub fn new(index: usize, name: impl Into<String>, start_pct: f32, stop_pct: f32) -> Self {
        Self {
            index,
            name: name.into(),
            start_pct,
            stop_pct,
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn start_pct(&self) -> f32 {
        self.start_pct
    }

    pub fn stop_pct(&self) -> f32 {
        self.stop_pct
    }

    pub fn trim_label(&self) -> String {
        format!("{:.0}%–{:.0}%", self.start_pct, self.stop_pct)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SamplerSlotSnapshot {
    bank: SampleBank,
    button: SampleButtons,
    function: SamplePlaybackMode,
    order: SamplePlayOrder,
    is_playing: bool,
    is_recording: bool,
    samples: Vec<SamplerLoadedSample>,
}

impl SamplerSlotSnapshot {
    pub fn new(
        bank: SampleBank,
        button: SampleButtons,
        function: SamplePlaybackMode,
        order: SamplePlayOrder,
        is_playing: bool,
        is_recording: bool,
        samples: Vec<SamplerLoadedSample>,
    ) -> Self {
        Self {
            bank,
            button,
            function,
            order,
            is_playing,
            is_recording,
            samples,
        }
    }

    pub fn from_sampler(sampler: &Sampler) -> Vec<Self> {
        let mut rows = Vec::new();
        for bank in [SampleBank::A, SampleBank::B, SampleBank::C] {
            let Some(buttons) = sampler.banks.get(&bank) else {
                continue;
            };
            for button in [
                SampleButtons::TopLeft,
                SampleButtons::TopRight,
                SampleButtons::BottomLeft,
                SampleButtons::BottomRight,
            ] {
                let Some(slot) = buttons.get(&button) else {
                    continue;
                };
                rows.push(Self::new(
                    bank,
                    button,
                    slot.function,
                    slot.order,
                    slot.is_playing,
                    slot.is_recording,
                    slot.samples
                        .iter()
                        .enumerate()
                        .map(|(index, sample)| {
                            SamplerLoadedSample::new(
                                index,
                                sample.name.clone(),
                                sample.start_pct,
                                sample.stop_pct,
                            )
                        })
                        .collect(),
                ));
            }
        }
        rows
    }

    pub fn bank(&self) -> SampleBank {
        self.bank
    }

    pub fn button(&self) -> SampleButtons {
        self.button
    }

    pub fn function(&self) -> SamplePlaybackMode {
        self.function
    }

    pub fn order(&self) -> SamplePlayOrder {
        self.order
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording
    }

    pub fn samples(&self) -> &[SamplerLoadedSample] {
        &self.samples
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    pub fn status_label(&self) -> &'static str {
        if self.is_recording {
            "Recording"
        } else if self.is_playing {
            "Playing"
        } else if self.samples.is_empty() {
            "Empty"
        } else {
            "Ready"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamplerSampleRow {
    display_name: String,
    path: String,
}

impl SamplerSampleRow {
    pub fn new(display_name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            display_name: display_name.into(),
            path: path.into(),
        }
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamplerSampleBrowser {
    root: PathBuf,
    rows: Vec<SamplerSampleRow>,
}

impl SamplerSampleBrowser {
    pub fn from_directory(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let mut rows = fs::read_dir(&root)
            .ok()
            .into_iter()
            .flat_map(|entries| entries.filter_map(Result::ok))
            .filter_map(|entry| {
                let path = entry.path();
                if !path.is_file() || !Self::is_supported_audio_file(&path) {
                    return None;
                }
                let display_name = path.file_name()?.to_string_lossy().to_string();
                Some(SamplerSampleRow::new(
                    display_name,
                    path.to_string_lossy().to_string(),
                ))
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| left.display_name.cmp(&right.display_name));
        Self { root, rows }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn rows(&self) -> &[SamplerSampleRow] {
        &self.rows
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn supported_audio_extensions() -> &'static [&'static str] {
        &["wav", "mp3", "flac", "ogg", "aiff", "aif", "aac", "m4a"]
    }

    pub fn is_supported_audio_file(path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                Self::supported_audio_extensions()
                    .iter()
                    .any(|supported| extension.eq_ignore_ascii_case(supported))
            })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SamplerFileAction {
    label: String,
    description: &'static str,
    command: PersonalCommand,
    requires_confirmation: bool,
}

impl SamplerFileAction {
    pub fn add_from_path(
        bank: SampleBank,
        button: SampleButtons,
        sample_path: &str,
    ) -> Option<Self> {
        let trimmed = sample_path.trim();
        if trimmed.is_empty() || !SamplerSampleBrowser::is_supported_audio_file(Path::new(trimmed))
        {
            return None;
        }
        Some(Self::new(
            "Add file",
            "Import the typed audio file path into this sampler slot.",
            PersonalCommand::AddSample(bank, button, trimmed.to_string()),
            true,
        ))
    }

    pub fn remove_first(bank: SampleBank, button: SampleButtons) -> Self {
        Self::remove_by_index(bank, button, 0)
    }

    pub fn remove_by_index(bank: SampleBank, button: SampleButtons, sample_index: usize) -> Self {
        Self::new(
            format!("Remove #{}", sample_index + 1),
            "Remove this sample from this slot; use only after checking the live slot list.",
            PersonalCommand::RemoveSampleByIndex(bank, button, sample_index),
            true,
        )
    }

    pub fn play_by_index(bank: SampleBank, button: SampleButtons, sample_index: usize) -> Self {
        Self::new(
            format!("Play #{}", sample_index + 1),
            "Play this sample from this slot for quick verification.",
            PersonalCommand::PlaySampleByIndex(bank, button, sample_index),
            false,
        )
    }

    pub fn play_first(bank: SampleBank, button: SampleButtons) -> Self {
        Self::play_by_index(bank, button, 0)
    }

    fn new(
        label: impl Into<String>,
        description: &'static str,
        command: PersonalCommand,
        requires_confirmation: bool,
    ) -> Self {
        Self {
            label: label.into(),
            description,
            command,
            requires_confirmation,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn description(&self) -> &'static str {
        self.description
    }

    pub fn command(&self) -> PersonalCommand {
        self.command.clone()
    }

    pub fn requires_confirmation(&self) -> bool {
        self.requires_confirmation
    }

    pub fn command_if_confirmed(&self, confirmed: bool) -> Option<PersonalCommand> {
        if self.requires_confirmation && !confirmed {
            None
        } else {
            Some(self.command())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixerLayoutPolicy;

impl MixerLayoutPolicy {
    pub fn panel_width() -> f32 {
        640.0
    }

    pub fn assignment_panel_width() -> f32 {
        640.0
    }

    pub fn assignment_button_width() -> f32 {
        84.0
    }

    pub fn fader_mute_state_button_width() -> f32 {
        92.0
    }

    pub fn assignment_card_width() -> f32 {
        294.0
    }

    pub fn assignment_card_height() -> f32 {
        292.0
    }

    pub fn assignment_card_gap() -> f32 {
        8.0
    }

    pub fn assignment_cards_per_row() -> usize {
        2
    }

    pub fn scribble_panel_width() -> f32 {
        640.0
    }

    pub fn scribble_button_width() -> f32 {
        118.0
    }

    pub fn monitor_mix_panel_width() -> f32 {
        640.0
    }

    pub fn monitor_mix_button_width() -> f32 {
        118.0
    }

    pub fn submix_panel_width() -> f32 {
        640.0
    }

    pub fn submix_button_width() -> f32 {
        96.0
    }

    pub fn submix_slider_width() -> f32 {
        220.0
    }

    pub fn channel_strip_width() -> f32 {
        94.0
    }

    pub fn channel_strip_height() -> f32 {
        270.0
    }

    pub fn channel_slider_height() -> f32 {
        190.0
    }

    pub fn panel_gap() -> f32 {
        12.0
    }

    pub fn top_row_height() -> f32 {
        520.0
    }

    pub fn status_row_height() -> f32 {
        320.0
    }

    pub fn detail_row_height() -> f32 {
        660.0
    }

    pub fn overview_panel_height() -> f32 {
        Self::top_row_height()
    }

    pub fn detail_panel_height() -> f32 {
        Self::detail_row_height()
    }

    pub fn uses_equal_height_within_rows() -> bool {
        true
    }

    pub fn uses_wrapped_dashboard_panels() -> bool {
        false
    }

    pub fn uses_fader_assignment_editor() -> bool {
        true
    }

    pub fn uses_compact_fader_assignment_cards() -> bool {
        true
    }

    pub fn uses_scribble_strip_editor() -> bool {
        true
    }

    pub fn uses_monitor_mix_selector() -> bool {
        true
    }

    pub fn uses_submix_controls() -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaderAssignmentControl {
    fader: FaderName,
    label: &'static str,
    default_channel: ChannelName,
}

impl FaderAssignmentControl {
    pub fn daily_controls() -> Vec<Self> {
        vec![
            Self::new(FaderName::A, "Fader A", ChannelName::Mic),
            Self::new(FaderName::B, "Fader B", ChannelName::Chat),
            Self::new(FaderName::C, "Fader C", ChannelName::Music),
            Self::new(FaderName::D, "Fader D", ChannelName::Game),
        ]
    }

    pub fn daily_channels() -> Vec<ChannelName> {
        vec![
            ChannelName::Mic,
            ChannelName::Chat,
            ChannelName::Music,
            ChannelName::Game,
            ChannelName::Console,
            ChannelName::LineIn,
            ChannelName::System,
            ChannelName::Sample,
        ]
    }

    fn new(fader: FaderName, label: &'static str, default_channel: ChannelName) -> Self {
        Self {
            fader,
            label,
            default_channel,
        }
    }

    pub fn fader(&self) -> FaderName {
        self.fader
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn description(&self) -> &'static str {
        "Assign a GoXLR channel to this hardware fader."
    }

    pub fn default_channel(&self) -> ChannelName {
        self.default_channel
    }

    pub fn assign_command(&self, channel: ChannelName) -> PersonalCommand {
        PersonalCommand::SetFader(self.fader, channel)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaderMuteFunctionControl {
    fader: FaderName,
    label: &'static str,
    default_function: MuteFunction,
}

impl FaderMuteFunctionControl {
    pub fn daily_controls() -> Vec<Self> {
        vec![
            Self::new(FaderName::A, "Fader A mute", MuteFunction::ToStream),
            Self::new(FaderName::B, "Fader B mute", MuteFunction::ToStream),
            Self::new(FaderName::C, "Fader C mute", MuteFunction::ToStream),
            Self::new(FaderName::D, "Fader D mute", MuteFunction::ToStream),
        ]
    }

    pub fn daily_functions() -> Vec<MuteFunction> {
        vec![
            MuteFunction::All,
            MuteFunction::ToStream,
            MuteFunction::ToVoiceChat,
            MuteFunction::ToPhones,
        ]
    }

    fn new(fader: FaderName, label: &'static str, default_function: MuteFunction) -> Self {
        Self {
            fader,
            label,
            default_function,
        }
    }

    pub fn fader(&self) -> FaderName {
        self.fader
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn description(&self) -> &'static str {
        "Choose where this fader mute button sends silence."
    }

    pub fn default_function(&self) -> MuteFunction {
        self.default_function
    }

    pub fn function_command(&self, function: MuteFunction) -> PersonalCommand {
        PersonalCommand::SetFaderMuteFunction(self.fader, function)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaderMuteStateControl {
    fader: FaderName,
    label: &'static str,
    default_state: MuteState,
}

impl FaderMuteStateControl {
    pub fn daily_controls() -> Vec<Self> {
        vec![
            Self::new(FaderName::A, "Fader A state", MuteState::Unmuted),
            Self::new(FaderName::B, "Fader B state", MuteState::Unmuted),
            Self::new(FaderName::C, "Fader C state", MuteState::Unmuted),
            Self::new(FaderName::D, "Fader D state", MuteState::Unmuted),
        ]
    }

    pub fn daily_states() -> Vec<MuteState> {
        vec![
            MuteState::Unmuted,
            MuteState::MutedToX,
            MuteState::MutedToAll,
        ]
    }

    fn new(fader: FaderName, label: &'static str, default_state: MuteState) -> Self {
        Self {
            fader,
            label,
            default_state,
        }
    }

    pub fn fader(&self) -> FaderName {
        self.fader
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn description(&self) -> &'static str {
        "Directly set the current mute state for this hardware fader."
    }

    pub fn default_state(&self) -> MuteState {
        self.default_state
    }

    pub fn state_label(state: MuteState) -> &'static str {
        match state {
            MuteState::Unmuted => "Unmute",
            MuteState::MutedToX => "Mute target",
            MuteState::MutedToAll => "Mute all",
        }
    }

    pub fn state_command(&self, state: MuteState) -> PersonalCommand {
        PersonalCommand::SetFaderMuteState(self.fader, state)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorMixControl {
    output: OutputDevice,
    label: &'static str,
}

impl MonitorMixControl {
    pub fn daily_controls() -> Vec<Self> {
        vec![
            Self::new(OutputDevice::Headphones, "Headphones"),
            Self::new(OutputDevice::BroadcastMix, "Broadcast"),
            Self::new(OutputDevice::ChatMic, "Chat Mic"),
            Self::new(OutputDevice::LineOut, "Line Out"),
        ]
    }

    fn new(output: OutputDevice, label: &'static str) -> Self {
        Self { output, label }
    }

    pub fn output(&self) -> OutputDevice {
        self.output
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn description(&self) -> &'static str {
        "Choose which GoXLR output mix is monitored in headphones."
    }

    pub fn command(&self) -> PersonalCommand {
        PersonalCommand::SetMonitorMix(self.output)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmixChannelControl {
    channel: ChannelName,
    label: &'static str,
}

impl SubmixChannelControl {
    pub fn daily_controls() -> Vec<Self> {
        vec![
            Self::new(ChannelName::Mic, "Mic"),
            Self::new(ChannelName::Chat, "Chat"),
            Self::new(ChannelName::Music, "Music"),
            Self::new(ChannelName::Game, "Game"),
            Self::new(ChannelName::Console, "Console"),
            Self::new(ChannelName::LineIn, "Line In"),
            Self::new(ChannelName::System, "System"),
            Self::new(ChannelName::Sample, "Sample"),
        ]
    }

    fn new(channel: ChannelName, label: &'static str) -> Self {
        Self { channel, label }
    }

    pub fn channel(&self) -> ChannelName {
        self.channel
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn description(&self) -> &'static str {
        "Set a conservative submix volume preset or link this channel to the main mix."
    }

    pub fn volume_presets(&self) -> Vec<u8> {
        vec![0, 50, 100]
    }

    pub fn percent_to_raw_volume(volume_percent: u8) -> u8 {
        ((255 * volume_percent as u16) / 100) as u8
    }

    pub fn volume_command(&self, volume_percent: u8) -> PersonalCommand {
        PersonalCommand::SetSubMixVolume(self.channel, Self::percent_to_raw_volume(volume_percent))
    }

    pub fn link_command(&self, linked: bool) -> PersonalCommand {
        PersonalCommand::SetSubMixLinked(self.channel, linked)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmixVolumeSlider {
    channel: ChannelName,
    label: &'static str,
}

impl SubmixVolumeSlider {
    pub fn daily_sliders() -> Vec<Self> {
        SubmixChannelControl::daily_controls()
            .into_iter()
            .map(|control| {
                Self::new(
                    control.channel(),
                    match control.channel() {
                        ChannelName::Mic => "Mic volume",
                        ChannelName::Chat => "Chat volume",
                        ChannelName::Music => "Music volume",
                        ChannelName::Game => "Game volume",
                        ChannelName::Console => "Console volume",
                        ChannelName::LineIn => "Line In volume",
                        ChannelName::System => "System volume",
                        ChannelName::Sample => "Sample volume",
                        _ => "Submix volume",
                    },
                )
            })
            .collect()
    }

    fn new(channel: ChannelName, label: &'static str) -> Self {
        Self { channel, label }
    }

    pub fn channel(&self) -> ChannelName {
        self.channel
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn range(&self) -> RangeInclusive<u8> {
        0..=100
    }

    pub fn value_from_snapshot(&self, snapshot: Option<&SubmixChannelSnapshot>) -> u8 {
        snapshot.map_or(50, SubmixChannelSnapshot::volume_percent)
    }

    pub fn command_for_percent(&self, percent: u16) -> PersonalCommand {
        let percent = percent.min(100) as u8;
        PersonalCommand::SetSubMixVolume(
            self.channel,
            SubmixChannelControl::percent_to_raw_volume(percent),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmixOutputMixControl {
    output: OutputDevice,
    label: &'static str,
}

impl SubmixOutputMixControl {
    pub fn daily_controls() -> Vec<Self> {
        vec![
            Self::new(OutputDevice::Headphones, "Headphones"),
            Self::new(OutputDevice::BroadcastMix, "Broadcast"),
            Self::new(OutputDevice::ChatMic, "Chat Mic"),
            Self::new(OutputDevice::LineOut, "Line Out"),
        ]
    }

    fn new(output: OutputDevice, label: &'static str) -> Self {
        Self { output, label }
    }

    pub fn output(&self) -> OutputDevice {
        self.output
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn mixes(&self) -> Vec<Mix> {
        vec![Mix::A, Mix::B]
    }

    pub fn mix_command(&self, mix: Mix) -> PersonalCommand {
        PersonalCommand::SetSubMixOutputMix(self.output, mix)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubmixChannelSnapshot {
    channel: ChannelName,
    label: String,
    volume: u8,
    linked: bool,
    ratio: f64,
}

impl SubmixChannelSnapshot {
    pub fn new(
        channel: ChannelName,
        label: impl Into<String>,
        volume: u8,
        linked: bool,
        ratio: f64,
    ) -> Self {
        Self {
            channel,
            label: label.into(),
            volume,
            linked,
            ratio,
        }
    }

    pub fn from_submixes(submixes: &Submixes) -> Vec<Self> {
        SubmixChannelControl::daily_controls()
            .into_iter()
            .map(|control| {
                let state = &submixes.inputs[channel_to_submix_channel(control.channel())];
                Self::new(
                    control.channel(),
                    control.label(),
                    state.volume,
                    state.linked,
                    state.ratio,
                )
            })
            .collect()
    }

    pub fn channel(&self) -> ChannelName {
        self.channel
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn volume(&self) -> u8 {
        self.volume
    }

    pub fn volume_percent(&self) -> u8 {
        ((self.volume as u16 * 100) / 255) as u8
    }

    pub fn linked(&self) -> bool {
        self.linked
    }

    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    pub fn state_label(&self) -> String {
        format!(
            "{}% {}",
            self.volume_percent(),
            if self.linked { "linked" } else { "unlinked" }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmixOutputSnapshot {
    output: OutputDevice,
    label: String,
    mix: Mix,
}

impl SubmixOutputSnapshot {
    pub fn new(output: OutputDevice, label: impl Into<String>, mix: Mix) -> Self {
        Self {
            output,
            label: label.into(),
            mix,
        }
    }

    pub fn from_submixes(submixes: &Submixes) -> Vec<Self> {
        SubmixOutputMixControl::daily_controls()
            .into_iter()
            .map(|control| {
                Self::new(
                    control.output(),
                    control.label(),
                    submixes.outputs[control.output()],
                )
            })
            .collect()
    }

    pub fn output(&self) -> OutputDevice {
        self.output
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn mix(&self) -> Mix {
        self.mix
    }

    pub fn state_label(&self) -> String {
        format!("Mix {}", self.mix)
    }
}

fn channel_to_submix_channel(channel: ChannelName) -> SubMixChannelName {
    match channel {
        ChannelName::Mic => SubMixChannelName::Mic,
        ChannelName::Chat => SubMixChannelName::Chat,
        ChannelName::Music => SubMixChannelName::Music,
        ChannelName::Game => SubMixChannelName::Game,
        ChannelName::Console => SubMixChannelName::Console,
        ChannelName::LineIn => SubMixChannelName::LineIn,
        ChannelName::System => SubMixChannelName::System,
        ChannelName::Sample => SubMixChannelName::Sample,
        _ => SubMixChannelName::Mic,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HardwareScribbleControl {
    fader: FaderName,
    label: &'static str,
    default_text: &'static str,
    default_number: &'static str,
}

impl HardwareScribbleControl {
    pub fn daily_controls() -> Vec<Self> {
        vec![
            Self::new(FaderName::A, "Fader A scribble", "Mic", "1"),
            Self::new(FaderName::B, "Fader B scribble", "Chat", "2"),
            Self::new(FaderName::C, "Fader C scribble", "Music", "3"),
            Self::new(FaderName::D, "Fader D scribble", "Game", "4"),
        ]
    }

    pub fn daily_icon_presets() -> Vec<Option<&'static str>> {
        vec![
            None,
            Some("mic.png"),
            Some("music.png"),
            Some("person.png"),
            Some("scale.png"),
        ]
    }

    fn new(
        fader: FaderName,
        label: &'static str,
        default_text: &'static str,
        default_number: &'static str,
    ) -> Self {
        Self {
            fader,
            label,
            default_text,
            default_number,
        }
    }

    pub fn fader(&self) -> FaderName {
        self.fader
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn description(&self) -> &'static str {
        "Set the hardware scribble-strip icon, label, number, or invert state."
    }

    pub fn default_text(&self) -> &'static str {
        self.default_text
    }

    pub fn default_number(&self) -> &'static str {
        self.default_number
    }

    pub fn icon_command(&self, icon: Option<&str>) -> PersonalCommand {
        PersonalCommand::SetScribbleIcon(self.fader, icon.map(str::to_string))
    }

    pub fn text_command(&self, text: &str) -> PersonalCommand {
        PersonalCommand::SetScribbleText(self.fader, text.to_string())
    }

    pub fn number_command(&self, number: &str) -> PersonalCommand {
        PersonalCommand::SetScribbleNumber(self.fader, number.to_string())
    }

    pub fn invert_command(&self, inverted: bool) -> PersonalCommand {
        PersonalCommand::SetScribbleInvert(self.fader, inverted)
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
    SetFader(FaderName, ChannelName),
    SetFaderMuteFunction(FaderName, MuteFunction),
    SetFaderMuteState(FaderName, MuteState),
    SetScribbleIcon(FaderName, Option<String>),
    SetScribbleText(FaderName, String),
    SetScribbleNumber(FaderName, String),
    SetScribbleInvert(FaderName, bool),
    SetRouter(InputDevice, OutputDevice, bool),
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
    SetHeadphoneEqPreamp(f32),
    SetHeadphoneEqBandGain(u8, f32),
    SetHeadphoneEqBandFrequency(u8, f32),
    SetHeadphoneEqBandQ(u8, f32),
    LoadHeadphoneEqProfile(String),
    SaveHeadphoneEqProfile(String),
    DeleteHeadphoneEqProfile(String),
    SetEqMiniGain(MiniEqFrequencies, i8),
    SetEqMiniFreq(MiniEqFrequencies, f32),
    SetEqGain(EqFrequencies, i8),
    SetEqFreq(EqFrequencies, f32),
    LoadMicProfile(String, bool),
    SaveMicProfileAs(String),
    DeleteMicProfile(String),
    SetActiveEffectPreset(EffectBankPresets),
    LoadEffectPreset(String),
    RenameActiveEffectPreset(String),
    SaveActiveEffectPreset,
    SetFXEnabled(bool),
    SetReverbStyle(ReverbStyle),
    SetReverbAmount(u8),
    SetReverbDecay(u16),
    SetReverbEarlyLevel(i8),
    SetReverbTailLevel(i8),
    SetReverbPreDelay(u8),
    SetReverbLowColour(i8),
    SetReverbHighColour(i8),
    SetReverbHighFactor(i8),
    SetReverbDiffuse(i8),
    SetReverbModSpeed(i8),
    SetReverbModDepth(i8),
    SetEchoStyle(EchoStyle),
    SetEchoAmount(u8),
    SetEchoFeedback(u8),
    SetEchoTempo(u16),
    SetEchoDelayLeft(u16),
    SetEchoDelayRight(u16),
    SetEchoFeedbackLeft(u8),
    SetEchoFeedbackRight(u8),
    SetEchoFeedbackXFBLtoR(u8),
    SetEchoFeedbackXFBRtoL(u8),
    SetPitchStyle(PitchStyle),
    SetPitchAmount(i8),
    SetPitchCharacter(u8),
    SetGenderStyle(GenderStyle),
    SetGenderAmount(i8),
    SetMegaphoneEnabled(bool),
    SetMegaphoneStyle(MegaphoneStyle),
    SetMegaphoneAmount(u8),
    SetMegaphonePostGain(i8),
    SetRobotEnabled(bool),
    SetRobotStyle(RobotStyle),
    SetRobotGain(RobotRange, i8),
    SetRobotFreq(RobotRange, u8),
    SetRobotWidth(RobotRange, u8),
    SetRobotWaveform(u8),
    SetRobotPulseWidth(u8),
    SetRobotThreshold(i8),
    SetRobotDryMix(i8),
    SetHardTuneEnabled(bool),
    SetHardTuneStyle(HardTuneStyle),
    SetHardTuneAmount(u8),
    SetHardTuneRate(u8),
    SetHardTuneWindow(u16),
    SetHardTuneSource(HardTuneSource),
    SetAnimationMode(AnimationMode),
    SetAnimationMod1(u8),
    SetAnimationMod2(u8),
    SetAnimationWaterfall(WaterfallDirection),
    SetGlobalColour(String),
    SetFaderColours(FaderName, String, String),
    SetFaderDisplayStyle(FaderName, FaderDisplayStyle),
    SetAllFaderColours(String, String),
    SetAllFaderDisplayStyle(FaderDisplayStyle),
    SetButtonColours(Button, String, Option<String>),
    SetButtonOffStyle(Button, ButtonColourOffStyle),
    SetButtonGroupColours(ButtonColourGroups, String, Option<String>),
    SetButtonGroupOffStyle(ButtonColourGroups, ButtonColourOffStyle),
    SetSimpleColour(SimpleColourTargets, String),
    SetEncoderColour(EncoderColourTargets, String, String, String),
    SetSampleColour(SamplerColourTargets, String, String, String),
    SetSampleOffStyle(SamplerColourTargets, ButtonColourOffStyle),
    SetMuteHoldDuration(u16),
    SetCoughIsHold(bool),
    SetCoughMuteFunction(MuteFunction),
    SetVCMuteAlsoMuteCM(bool),
    SetMonitorWithFx(bool),
    SetMonitorMix(OutputDevice),
    SetSubMixEnabled(bool),
    SetSubMixVolume(ChannelName, u8),
    SetSubMixLinked(ChannelName, bool),
    SetSubMixOutputMix(OutputDevice, Mix),
    SetLockFaders(bool),
    SetVodMode(VodMode),
    SetActiveSamplerBank(SampleBank),
    SetSamplerFunction(SampleBank, SampleButtons, SamplePlaybackMode),
    SetSamplerOrder(SampleBank, SampleButtons, SamplePlayOrder),
    ClearSampleProcessError,
    SetSamplerResetOnClear(bool),
    SetSamplerFadeDuration(u32),
    SetSampleStartPercent(SampleBank, SampleButtons, usize, f32),
    SetSampleStopPercent(SampleBank, SampleButtons, usize, f32),
    AddSample(SampleBank, SampleButtons, String),
    RemoveSampleByIndex(SampleBank, SampleButtons, usize),
    PlaySampleByIndex(SampleBank, SampleButtons, usize),
    PlayNextSample(SampleBank, SampleButtons),
    StopSamplePlayback(SampleBank, SampleButtons),
    LoadProfileColours(String),
    NewProfile(String),
    LoadProfile(String, bool),
    SaveProfile,
    SaveProfileAs(String),
    DeleteProfile(String),
    SaveMicProfile,
    ReloadSettings,
}

impl From<PersonalCommand> for GoXLRCommand {
    fn from(value: PersonalCommand) -> Self {
        match value {
            PersonalCommand::SetVolume(channel, volume) => GoXLRCommand::SetVolume(channel, volume),
            PersonalCommand::SetFader(fader, channel) => GoXLRCommand::SetFader(fader, channel),
            PersonalCommand::SetFaderMuteFunction(fader, function) => {
                GoXLRCommand::SetFaderMuteFunction(fader, function)
            }
            PersonalCommand::SetFaderMuteState(fader, state) => {
                GoXLRCommand::SetFaderMuteState(fader, state)
            }
            PersonalCommand::SetScribbleIcon(fader, icon) => {
                GoXLRCommand::SetScribbleIcon(fader, icon)
            }
            PersonalCommand::SetScribbleText(fader, text) => {
                GoXLRCommand::SetScribbleText(fader, text)
            }
            PersonalCommand::SetScribbleNumber(fader, number) => {
                GoXLRCommand::SetScribbleNumber(fader, number)
            }
            PersonalCommand::SetScribbleInvert(fader, inverted) => {
                GoXLRCommand::SetScribbleInvert(fader, inverted)
            }
            PersonalCommand::SetRouter(input, output, enabled) => {
                GoXLRCommand::SetRouter(input, output, enabled)
            }
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
            // Upstream `GoXLRCommand` spells this `SetDeeser` (one `s`). Local enum keeps the
            // correct `SetDeesser` spelling; this is the bridge.
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
            PersonalCommand::SetHeadphoneEqPreamp(preamp) => {
                GoXLRCommand::SetHeadphoneEqPreamp(preamp)
            }
            PersonalCommand::SetHeadphoneEqBandGain(index, gain) => {
                GoXLRCommand::SetHeadphoneEqBandGain(index, gain)
            }
            PersonalCommand::SetHeadphoneEqBandFrequency(index, frequency) => {
                GoXLRCommand::SetHeadphoneEqBandFrequency(index, frequency)
            }
            PersonalCommand::SetHeadphoneEqBandQ(index, q) => {
                GoXLRCommand::SetHeadphoneEqBandQ(index, q)
            }
            PersonalCommand::LoadHeadphoneEqProfile(profile) => {
                GoXLRCommand::LoadHeadphoneEqProfile(profile)
            }
            PersonalCommand::SaveHeadphoneEqProfile(profile) => {
                GoXLRCommand::SaveHeadphoneEqProfile(profile)
            }
            PersonalCommand::DeleteHeadphoneEqProfile(profile) => {
                GoXLRCommand::DeleteHeadphoneEqProfile(profile)
            }
            PersonalCommand::SetEqMiniGain(band, gain) => GoXLRCommand::SetEqMiniGain(band, gain),
            PersonalCommand::SetEqMiniFreq(band, frequency) => {
                GoXLRCommand::SetEqMiniFreq(band, frequency)
            }
            PersonalCommand::SetEqGain(band, gain) => GoXLRCommand::SetEqGain(band, gain),
            PersonalCommand::SetEqFreq(band, frequency) => GoXLRCommand::SetEqFreq(band, frequency),
            PersonalCommand::LoadMicProfile(profile, load_hardware) => {
                GoXLRCommand::LoadMicProfile(profile, load_hardware)
            }
            PersonalCommand::SaveMicProfileAs(profile) => GoXLRCommand::SaveMicProfileAs(profile),
            PersonalCommand::DeleteMicProfile(profile) => GoXLRCommand::DeleteMicProfile(profile),
            PersonalCommand::SetActiveEffectPreset(preset) => {
                GoXLRCommand::SetActiveEffectPreset(preset)
            }
            PersonalCommand::LoadEffectPreset(preset) => GoXLRCommand::LoadEffectPreset(preset),
            PersonalCommand::RenameActiveEffectPreset(preset) => {
                GoXLRCommand::RenameActivePreset(preset)
            }
            PersonalCommand::SaveActiveEffectPreset => GoXLRCommand::SaveActivePreset(),
            PersonalCommand::SetFXEnabled(enabled) => GoXLRCommand::SetFXEnabled(enabled),
            PersonalCommand::SetReverbStyle(style) => GoXLRCommand::SetReverbStyle(style),
            PersonalCommand::SetReverbAmount(amount) => GoXLRCommand::SetReverbAmount(amount),
            PersonalCommand::SetReverbDecay(value) => GoXLRCommand::SetReverbDecay(value),
            PersonalCommand::SetReverbEarlyLevel(value) => GoXLRCommand::SetReverbEarlyLevel(value),
            PersonalCommand::SetReverbTailLevel(value) => GoXLRCommand::SetReverbTailLevel(value),
            PersonalCommand::SetReverbPreDelay(value) => GoXLRCommand::SetReverbPreDelay(value),
            PersonalCommand::SetReverbLowColour(value) => GoXLRCommand::SetReverbLowColour(value),
            PersonalCommand::SetReverbHighColour(value) => GoXLRCommand::SetReverbHighColour(value),
            PersonalCommand::SetReverbHighFactor(value) => GoXLRCommand::SetReverbHighFactor(value),
            PersonalCommand::SetReverbDiffuse(value) => GoXLRCommand::SetReverbDiffuse(value),
            PersonalCommand::SetReverbModSpeed(value) => GoXLRCommand::SetReverbModSpeed(value),
            PersonalCommand::SetReverbModDepth(value) => GoXLRCommand::SetReverbModDepth(value),
            PersonalCommand::SetEchoStyle(style) => GoXLRCommand::SetEchoStyle(style),
            PersonalCommand::SetEchoAmount(amount) => GoXLRCommand::SetEchoAmount(amount),
            PersonalCommand::SetEchoFeedback(value) => GoXLRCommand::SetEchoFeedback(value),
            PersonalCommand::SetEchoTempo(value) => GoXLRCommand::SetEchoTempo(value),
            PersonalCommand::SetEchoDelayLeft(value) => GoXLRCommand::SetEchoDelayLeft(value),
            PersonalCommand::SetEchoDelayRight(value) => GoXLRCommand::SetEchoDelayRight(value),
            PersonalCommand::SetEchoFeedbackLeft(value) => GoXLRCommand::SetEchoFeedbackLeft(value),
            PersonalCommand::SetEchoFeedbackRight(value) => {
                GoXLRCommand::SetEchoFeedbackRight(value)
            }
            PersonalCommand::SetEchoFeedbackXFBLtoR(value) => {
                GoXLRCommand::SetEchoFeedbackXFBLtoR(value)
            }
            PersonalCommand::SetEchoFeedbackXFBRtoL(value) => {
                GoXLRCommand::SetEchoFeedbackXFBRtoL(value)
            }
            PersonalCommand::SetPitchStyle(style) => GoXLRCommand::SetPitchStyle(style),
            PersonalCommand::SetPitchAmount(amount) => GoXLRCommand::SetPitchAmount(amount),
            PersonalCommand::SetPitchCharacter(value) => GoXLRCommand::SetPitchCharacter(value),
            PersonalCommand::SetGenderStyle(style) => GoXLRCommand::SetGenderStyle(style),
            PersonalCommand::SetGenderAmount(amount) => GoXLRCommand::SetGenderAmount(amount),
            PersonalCommand::SetMegaphoneEnabled(enabled) => {
                GoXLRCommand::SetMegaphoneEnabled(enabled)
            }
            PersonalCommand::SetMegaphoneStyle(style) => GoXLRCommand::SetMegaphoneStyle(style),
            PersonalCommand::SetMegaphoneAmount(amount) => GoXLRCommand::SetMegaphoneAmount(amount),
            PersonalCommand::SetMegaphonePostGain(value) => {
                GoXLRCommand::SetMegaphonePostGain(value)
            }
            PersonalCommand::SetRobotEnabled(enabled) => GoXLRCommand::SetRobotEnabled(enabled),
            PersonalCommand::SetRobotStyle(style) => GoXLRCommand::SetRobotStyle(style),
            PersonalCommand::SetRobotGain(range, value) => GoXLRCommand::SetRobotGain(range, value),
            PersonalCommand::SetRobotFreq(range, value) => GoXLRCommand::SetRobotFreq(range, value),
            PersonalCommand::SetRobotWidth(range, value) => {
                GoXLRCommand::SetRobotWidth(range, value)
            }
            PersonalCommand::SetRobotWaveform(value) => GoXLRCommand::SetRobotWaveform(value),
            PersonalCommand::SetRobotPulseWidth(value) => GoXLRCommand::SetRobotPulseWidth(value),
            PersonalCommand::SetRobotThreshold(value) => GoXLRCommand::SetRobotThreshold(value),
            PersonalCommand::SetRobotDryMix(value) => GoXLRCommand::SetRobotDryMix(value),
            PersonalCommand::SetHardTuneEnabled(enabled) => {
                GoXLRCommand::SetHardTuneEnabled(enabled)
            }
            PersonalCommand::SetHardTuneStyle(style) => GoXLRCommand::SetHardTuneStyle(style),
            PersonalCommand::SetHardTuneAmount(value) => GoXLRCommand::SetHardTuneAmount(value),
            PersonalCommand::SetHardTuneRate(value) => GoXLRCommand::SetHardTuneRate(value),
            PersonalCommand::SetHardTuneWindow(value) => GoXLRCommand::SetHardTuneWindow(value),
            PersonalCommand::SetHardTuneSource(source) => GoXLRCommand::SetHardTuneSource(source),
            PersonalCommand::SetAnimationMode(mode) => GoXLRCommand::SetAnimationMode(mode),
            PersonalCommand::SetAnimationMod1(value) => GoXLRCommand::SetAnimationMod1(value),
            PersonalCommand::SetAnimationMod2(value) => GoXLRCommand::SetAnimationMod2(value),
            PersonalCommand::SetAnimationWaterfall(direction) => {
                GoXLRCommand::SetAnimationWaterfall(direction)
            }
            PersonalCommand::SetGlobalColour(colour) => GoXLRCommand::SetGlobalColour(colour),
            PersonalCommand::SetFaderColours(fader, top, bottom) => {
                GoXLRCommand::SetFaderColours(fader, top, bottom)
            }
            PersonalCommand::SetFaderDisplayStyle(fader, style) => {
                GoXLRCommand::SetFaderDisplayStyle(fader, style)
            }
            PersonalCommand::SetAllFaderColours(top, bottom) => {
                GoXLRCommand::SetAllFaderColours(top, bottom)
            }
            PersonalCommand::SetAllFaderDisplayStyle(style) => {
                GoXLRCommand::SetAllFaderDisplayStyle(style)
            }
            PersonalCommand::SetButtonColours(button, colour_one, colour_two) => {
                GoXLRCommand::SetButtonColours(button, colour_one, colour_two)
            }
            PersonalCommand::SetButtonOffStyle(button, off_style) => {
                GoXLRCommand::SetButtonOffStyle(button, off_style)
            }
            PersonalCommand::SetButtonGroupColours(group, colour_one, colour_two) => {
                GoXLRCommand::SetButtonGroupColours(group, colour_one, colour_two)
            }
            PersonalCommand::SetButtonGroupOffStyle(group, off_style) => {
                GoXLRCommand::SetButtonGroupOffStyle(group, off_style)
            }
            PersonalCommand::SetSimpleColour(target, colour) => {
                GoXLRCommand::SetSimpleColour(target, colour)
            }
            PersonalCommand::SetEncoderColour(target, colour_one, colour_two, colour_three) => {
                GoXLRCommand::SetEncoderColour(target, colour_one, colour_two, colour_three)
            }
            PersonalCommand::SetSampleColour(target, colour_one, colour_two, colour_three) => {
                GoXLRCommand::SetSampleColour(target, colour_one, colour_two, colour_three)
            }
            PersonalCommand::SetSampleOffStyle(target, off_style) => {
                GoXLRCommand::SetSampleOffStyle(target, off_style)
            }
            PersonalCommand::SetMuteHoldDuration(duration_ms) => {
                GoXLRCommand::SetMuteHoldDuration(duration_ms)
            }
            PersonalCommand::SetCoughIsHold(enabled) => GoXLRCommand::SetCoughIsHold(enabled),
            PersonalCommand::SetCoughMuteFunction(function) => {
                GoXLRCommand::SetCoughMuteFunction(function)
            }
            PersonalCommand::SetVCMuteAlsoMuteCM(enabled) => {
                GoXLRCommand::SetVCMuteAlsoMuteCM(enabled)
            }
            PersonalCommand::SetMonitorWithFx(enabled) => GoXLRCommand::SetMonitorWithFx(enabled),
            PersonalCommand::SetMonitorMix(output) => GoXLRCommand::SetMonitorMix(output),
            PersonalCommand::SetSubMixEnabled(enabled) => GoXLRCommand::SetSubMixEnabled(enabled),
            PersonalCommand::SetSubMixVolume(channel, volume) => {
                GoXLRCommand::SetSubMixVolume(channel, volume)
            }
            PersonalCommand::SetSubMixLinked(channel, linked) => {
                GoXLRCommand::SetSubMixLinked(channel, linked)
            }
            PersonalCommand::SetSubMixOutputMix(output, mix) => {
                GoXLRCommand::SetSubMixOutputMix(output, mix)
            }
            PersonalCommand::SetLockFaders(enabled) => GoXLRCommand::SetLockFaders(enabled),
            PersonalCommand::SetVodMode(mode) => GoXLRCommand::SetVodMode(mode),
            PersonalCommand::SetActiveSamplerBank(bank) => GoXLRCommand::SetActiveSamplerBank(bank),
            PersonalCommand::SetSamplerFunction(bank, button, mode) => {
                GoXLRCommand::SetSamplerFunction(bank, button, mode)
            }
            PersonalCommand::SetSamplerOrder(bank, button, order) => {
                GoXLRCommand::SetSamplerOrder(bank, button, order)
            }
            PersonalCommand::ClearSampleProcessError => GoXLRCommand::ClearSampleProcessError(),
            PersonalCommand::SetSamplerResetOnClear(enabled) => {
                GoXLRCommand::SetSamplerResetOnClear(enabled)
            }
            PersonalCommand::SetSamplerFadeDuration(duration_ms) => {
                GoXLRCommand::SetSamplerFadeDuration(duration_ms)
            }
            PersonalCommand::SetSampleStartPercent(bank, button, index, percent) => {
                GoXLRCommand::SetSampleStartPercent(bank, button, index, percent)
            }
            PersonalCommand::SetSampleStopPercent(bank, button, index, percent) => {
                GoXLRCommand::SetSampleStopPercent(bank, button, index, percent)
            }
            PersonalCommand::AddSample(bank, button, sample_path) => {
                GoXLRCommand::AddSample(bank, button, sample_path)
            }
            PersonalCommand::RemoveSampleByIndex(bank, button, index) => {
                GoXLRCommand::RemoveSampleByIndex(bank, button, index)
            }
            PersonalCommand::PlaySampleByIndex(bank, button, index) => {
                GoXLRCommand::PlaySampleByIndex(bank, button, index)
            }
            PersonalCommand::PlayNextSample(bank, button) => {
                GoXLRCommand::PlayNextSample(bank, button)
            }
            PersonalCommand::StopSamplePlayback(bank, button) => {
                GoXLRCommand::StopSamplePlayback(bank, button)
            }
            PersonalCommand::LoadProfileColours(profile) => {
                GoXLRCommand::LoadProfileColours(profile)
            }
            PersonalCommand::NewProfile(profile) => GoXLRCommand::NewProfile(profile),
            PersonalCommand::LoadProfile(profile, load_hardware) => {
                GoXLRCommand::LoadProfile(profile, load_hardware)
            }
            PersonalCommand::SaveProfile => GoXLRCommand::SaveProfile(),
            PersonalCommand::SaveProfileAs(profile) => GoXLRCommand::SaveProfileAs(profile),
            PersonalCommand::DeleteProfile(profile) => GoXLRCommand::DeleteProfile(profile),
            PersonalCommand::SaveMicProfile => GoXLRCommand::SaveMicProfile(),
            PersonalCommand::ReloadSettings => GoXLRCommand::ReloadSettings(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingMatrixRoute {
    input: InputDevice,
    output: OutputDevice,
    enabled: bool,
}

impl RoutingMatrixRoute {
    pub fn new(input: InputDevice, output: OutputDevice, enabled: bool) -> Self {
        Self {
            input,
            output,
            enabled,
        }
    }

    pub fn input(&self) -> InputDevice {
        self.input
    }

    pub fn output(&self) -> OutputDevice {
        self.output
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingMatrixLayoutPolicy;

impl RoutingMatrixLayoutPolicy {
    pub fn cell_width() -> f32 {
        74.0
    }

    pub fn cell_height() -> f32 {
        40.0
    }

    pub fn badge_width() -> f32 {
        50.0
    }

    pub fn badge_height() -> f32 {
        15.0
    }

    pub fn button_width() -> f32 {
        28.0
    }

    pub fn button_height() -> f32 {
        15.0
    }

    pub fn grid_column_gap() -> f32 {
        4.0
    }

    pub fn grid_row_gap() -> f32 {
        1.0
    }

    pub fn badge_text_size() -> f32 {
        9.0
    }

    pub fn badge_uses_available_height() -> bool {
        false
    }

    pub fn uses_compact_action_labels() -> bool {
        true
    }

    pub fn matrix_width_for_model() -> f32 {
        let output_columns = RoutingMatrixModel::outputs().len() as f32;
        let input_label_column_width = 68.0;
        input_label_column_width
            + output_columns * Self::cell_width()
            + output_columns * Self::grid_column_gap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingStateBadge {
    label: &'static str,
    fill: egui::Color32,
    stroke: egui::Color32,
    text: egui::Color32,
}

impl RoutingStateBadge {
    pub fn for_state(enabled: Option<bool>) -> Self {
        match enabled {
            Some(true) => Self {
                label: "Active",
                fill: egui::Color32::from_rgb(24, 72, 44),
                stroke: egui::Color32::from_rgb(80, 220, 120),
                text: egui::Color32::from_rgb(150, 255, 185),
            },
            Some(false) => Self {
                label: "Off",
                fill: egui::Color32::from_rgb(43, 48, 48),
                stroke: egui::Color32::from_rgb(120, 128, 128),
                text: egui::Color32::from_rgb(190, 198, 198),
            },
            None => Self {
                label: "Unknown",
                fill: egui::Color32::from_rgb(74, 58, 22),
                stroke: egui::Color32::from_rgb(230, 188, 70),
                text: egui::Color32::from_rgb(255, 220, 130),
            },
        }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn fill(&self) -> egui::Color32 {
        self.fill
    }

    pub fn stroke(&self) -> egui::Color32 {
        self.stroke
    }

    pub fn text(&self) -> egui::Color32 {
        self.text
    }

    pub fn min_width() -> f32 {
        52.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingMatrixCell {
    input: InputDevice,
    output: OutputDevice,
}

impl RoutingMatrixCell {
    pub fn new(input: InputDevice, output: OutputDevice) -> Self {
        Self { input, output }
    }

    pub fn input(&self) -> InputDevice {
        self.input
    }

    pub fn output(&self) -> OutputDevice {
        self.output
    }

    pub fn input_label(&self) -> &'static str {
        routing_input_label(self.input)
    }

    pub fn output_label(&self) -> &'static str {
        routing_output_label(self.output)
    }

    pub fn command_for_enabled(&self, enabled: bool) -> PersonalCommand {
        PersonalCommand::SetRouter(self.input, self.output, enabled)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoutingPreset {
    name: &'static str,
    description: &'static str,
    commands: Vec<PersonalCommand>,
}

impl RoutingPreset {
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
                "Broadcast Mix",
                "Send desktop sources and mic to headphones and broadcast output.",
                vec![
                    PersonalCommand::SetRouter(
                        InputDevice::Microphone,
                        OutputDevice::Headphones,
                        true,
                    ),
                    PersonalCommand::SetRouter(
                        InputDevice::Microphone,
                        OutputDevice::BroadcastMix,
                        true,
                    ),
                    PersonalCommand::SetRouter(InputDevice::Chat, OutputDevice::Headphones, true),
                    PersonalCommand::SetRouter(InputDevice::Chat, OutputDevice::BroadcastMix, true),
                    PersonalCommand::SetRouter(InputDevice::Music, OutputDevice::Headphones, true),
                    PersonalCommand::SetRouter(
                        InputDevice::Music,
                        OutputDevice::BroadcastMix,
                        true,
                    ),
                    PersonalCommand::SetRouter(InputDevice::Game, OutputDevice::Headphones, true),
                    PersonalCommand::SetRouter(InputDevice::Game, OutputDevice::BroadcastMix, true),
                    PersonalCommand::SetRouter(InputDevice::System, OutputDevice::Headphones, true),
                    PersonalCommand::SetRouter(
                        InputDevice::System,
                        OutputDevice::BroadcastMix,
                        true,
                    ),
                ],
            ),
            Self::new(
                "Chat Mic Only",
                "Keep microphone routed to chat mic while avoiding desktop audio bleed.",
                vec![
                    PersonalCommand::SetRouter(
                        InputDevice::Microphone,
                        OutputDevice::ChatMic,
                        true,
                    ),
                    PersonalCommand::SetRouter(InputDevice::Music, OutputDevice::ChatMic, false),
                    PersonalCommand::SetRouter(InputDevice::Game, OutputDevice::ChatMic, false),
                    PersonalCommand::SetRouter(InputDevice::System, OutputDevice::ChatMic, false),
                    PersonalCommand::SetRouter(InputDevice::Samples, OutputDevice::ChatMic, false),
                ],
            ),
            Self::new(
                "Line Out Safe",
                "Disable common sources from line out before changing external gear.",
                vec![
                    PersonalCommand::SetRouter(
                        InputDevice::Microphone,
                        OutputDevice::LineOut,
                        false,
                    ),
                    PersonalCommand::SetRouter(InputDevice::Chat, OutputDevice::LineOut, false),
                    PersonalCommand::SetRouter(InputDevice::Music, OutputDevice::LineOut, false),
                    PersonalCommand::SetRouter(InputDevice::Game, OutputDevice::LineOut, false),
                    PersonalCommand::SetRouter(InputDevice::System, OutputDevice::LineOut, false),
                    PersonalCommand::SetRouter(InputDevice::Samples, OutputDevice::LineOut, false),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingMatrixModel;

impl RoutingMatrixModel {
    pub fn inputs() -> Vec<InputDevice> {
        vec![
            InputDevice::Microphone,
            InputDevice::Chat,
            InputDevice::Music,
            InputDevice::Game,
            InputDevice::Console,
            InputDevice::LineIn,
            InputDevice::System,
            InputDevice::Samples,
        ]
    }

    pub fn outputs() -> Vec<OutputDevice> {
        vec![
            OutputDevice::Headphones,
            OutputDevice::BroadcastMix,
            OutputDevice::ChatMic,
            OutputDevice::Sampler,
            OutputDevice::LineOut,
        ]
    }

    pub fn cells() -> Vec<RoutingMatrixCell> {
        Self::inputs()
            .into_iter()
            .flat_map(|input| {
                Self::outputs()
                    .into_iter()
                    .map(move |output| RoutingMatrixCell::new(input, output))
            })
            .collect()
    }

    pub fn cell(input: InputDevice, output: OutputDevice) -> RoutingMatrixCell {
        RoutingMatrixCell::new(input, output)
    }
}

fn routing_input_label(input: InputDevice) -> &'static str {
    match input {
        InputDevice::Microphone => "Mic",
        InputDevice::Chat => "Chat",
        InputDevice::Music => "Music",
        InputDevice::Game => "Game",
        InputDevice::Console => "Console",
        InputDevice::LineIn => "Line In",
        InputDevice::System => "System",
        InputDevice::Samples => "Samples",
    }
}

fn routing_output_label(output: OutputDevice) -> &'static str {
    match output {
        OutputDevice::Headphones => "Headphones",
        OutputDevice::BroadcastMix => "Broadcast",
        OutputDevice::ChatMic => "Chat Mic",
        OutputDevice::Sampler => "Sampler",
        OutputDevice::LineOut => "Line Out",
        OutputDevice::StreamMix2 => "Stream Mix 2",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectsReverbSlider {
    Decay,
    EarlyLevel,
    TailLevel,
    PreDelay,
    LowColour,
    HighColour,
    HighFactor,
    Diffuse,
    ModSpeed,
    ModDepth,
}

impl EffectsReverbSlider {
    pub fn full_sliders() -> Vec<Self> {
        vec![
            Self::Decay,
            Self::EarlyLevel,
            Self::TailLevel,
            Self::PreDelay,
            Self::LowColour,
            Self::HighColour,
            Self::HighFactor,
            Self::Diffuse,
            Self::ModSpeed,
            Self::ModDepth,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Decay => "Decay",
            Self::EarlyLevel => "Early level",
            Self::TailLevel => "Tail level",
            Self::PreDelay => "Pre-delay",
            Self::LowColour => "Low colour",
            Self::HighColour => "High colour",
            Self::HighFactor => "High factor",
            Self::Diffuse => "Diffuse",
            Self::ModSpeed => "Mod speed",
            Self::ModDepth => "Mod depth",
        }
    }

    pub fn range(&self) -> RangeInclusive<i16> {
        match self {
            Self::Decay => 0..=3000,
            Self::PreDelay => 0..=100,
            Self::EarlyLevel
            | Self::TailLevel
            | Self::LowColour
            | Self::HighColour
            | Self::HighFactor
            | Self::Diffuse
            | Self::ModSpeed
            | Self::ModDepth => -50..=50,
        }
    }

    pub fn default_value(&self) -> i16 {
        match self {
            Self::Decay => 1500,
            Self::PreDelay => 25,
            Self::EarlyLevel
            | Self::TailLevel
            | Self::LowColour
            | Self::HighColour
            | Self::HighFactor
            | Self::Diffuse
            | Self::ModSpeed
            | Self::ModDepth => 0,
        }
    }

    pub fn command_for_value(&self, value: i16) -> PersonalCommand {
        let clamped = value.clamp(*self.range().start(), *self.range().end());
        match self {
            Self::Decay => PersonalCommand::SetReverbDecay(clamped as u16),
            Self::EarlyLevel => PersonalCommand::SetReverbEarlyLevel(clamped as i8),
            Self::TailLevel => PersonalCommand::SetReverbTailLevel(clamped as i8),
            Self::PreDelay => PersonalCommand::SetReverbPreDelay(clamped as u8),
            Self::LowColour => PersonalCommand::SetReverbLowColour(clamped as i8),
            Self::HighColour => PersonalCommand::SetReverbHighColour(clamped as i8),
            Self::HighFactor => PersonalCommand::SetReverbHighFactor(clamped as i8),
            Self::Diffuse => PersonalCommand::SetReverbDiffuse(clamped as i8),
            Self::ModSpeed => PersonalCommand::SetReverbModSpeed(clamped as i8),
            Self::ModDepth => PersonalCommand::SetReverbModDepth(clamped as i8),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectsEchoSlider {
    Feedback,
    Tempo,
    DelayLeft,
    DelayRight,
    FeedbackLeft,
    FeedbackRight,
    CrossFeedbackLeftToRight,
    CrossFeedbackRightToLeft,
}

impl EffectsEchoSlider {
    pub fn full_sliders() -> Vec<Self> {
        vec![
            Self::Feedback,
            Self::Tempo,
            Self::DelayLeft,
            Self::DelayRight,
            Self::FeedbackLeft,
            Self::FeedbackRight,
            Self::CrossFeedbackLeftToRight,
            Self::CrossFeedbackRightToLeft,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Feedback => "Feedback",
            Self::Tempo => "Tempo",
            Self::DelayLeft => "Left delay",
            Self::DelayRight => "Right delay",
            Self::FeedbackLeft => "Left feedback",
            Self::FeedbackRight => "Right feedback",
            Self::CrossFeedbackLeftToRight => "Cross L→R",
            Self::CrossFeedbackRightToLeft => "Cross R→L",
        }
    }

    pub fn range(&self) -> RangeInclusive<i16> {
        match self {
            Self::Tempo => 60..=300,
            Self::DelayLeft | Self::DelayRight => 0..=2500,
            Self::Feedback
            | Self::FeedbackLeft
            | Self::FeedbackRight
            | Self::CrossFeedbackLeftToRight
            | Self::CrossFeedbackRightToLeft => 0..=100,
        }
    }

    pub fn default_value(&self) -> i16 {
        match self {
            Self::Feedback | Self::FeedbackLeft | Self::FeedbackRight => 35,
            Self::Tempo => 120,
            Self::DelayLeft => 250,
            Self::DelayRight => 375,
            Self::CrossFeedbackLeftToRight | Self::CrossFeedbackRightToLeft => 0,
        }
    }

    pub fn command_for_value(&self, value: i16) -> PersonalCommand {
        let clamped = value.clamp(*self.range().start(), *self.range().end());
        match self {
            Self::Feedback => PersonalCommand::SetEchoFeedback(clamped as u8),
            Self::Tempo => PersonalCommand::SetEchoTempo(clamped as u16),
            Self::DelayLeft => PersonalCommand::SetEchoDelayLeft(clamped as u16),
            Self::DelayRight => PersonalCommand::SetEchoDelayRight(clamped as u16),
            Self::FeedbackLeft => PersonalCommand::SetEchoFeedbackLeft(clamped as u8),
            Self::FeedbackRight => PersonalCommand::SetEchoFeedbackRight(clamped as u8),
            Self::CrossFeedbackLeftToRight => {
                PersonalCommand::SetEchoFeedbackXFBLtoR(clamped as u8)
            }
            Self::CrossFeedbackRightToLeft => {
                PersonalCommand::SetEchoFeedbackXFBRtoL(clamped as u8)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectsPitchSlider {
    Character,
}

impl EffectsPitchSlider {
    pub fn full_sliders() -> Vec<Self> {
        vec![Self::Character]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Character => "Character",
        }
    }

    pub fn range(&self) -> RangeInclusive<i16> {
        match self {
            Self::Character => 0..=100,
        }
    }

    pub fn default_value(&self) -> i16 {
        match self {
            Self::Character => 50,
        }
    }

    pub fn command_for_value(&self, value: i16) -> PersonalCommand {
        let clamped = value.clamp(*self.range().start(), *self.range().end());
        match self {
            Self::Character => PersonalCommand::SetPitchCharacter(clamped as u8),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectsMegaphoneSlider {
    PostGain,
}

impl EffectsMegaphoneSlider {
    pub fn full_sliders() -> Vec<Self> {
        vec![Self::PostGain]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::PostGain => "Post gain",
        }
    }

    pub fn range(&self) -> RangeInclusive<i16> {
        match self {
            Self::PostGain => -20..=20,
        }
    }

    pub fn default_value(&self) -> i16 {
        match self {
            Self::PostGain => 0,
        }
    }

    pub fn command_for_value(&self, value: i16) -> PersonalCommand {
        let clamped = value.clamp(*self.range().start(), *self.range().end());
        match self {
            Self::PostGain => PersonalCommand::SetMegaphonePostGain(clamped as i8),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectsRobotSlider {
    LowGain,
    LowFrequency,
    LowWidth,
    MidGain,
    MidFrequency,
    MidWidth,
    HighGain,
    HighFrequency,
    HighWidth,
    Waveform,
    PulseWidth,
    Threshold,
    DryMix,
}

impl EffectsRobotSlider {
    pub fn full_sliders() -> Vec<Self> {
        vec![
            Self::LowGain,
            Self::LowFrequency,
            Self::LowWidth,
            Self::MidGain,
            Self::MidFrequency,
            Self::MidWidth,
            Self::HighGain,
            Self::HighFrequency,
            Self::HighWidth,
            Self::Waveform,
            Self::PulseWidth,
            Self::Threshold,
            Self::DryMix,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::LowGain => "Low gain",
            Self::LowFrequency => "Low frequency",
            Self::LowWidth => "Low width",
            Self::MidGain => "Mid gain",
            Self::MidFrequency => "Mid frequency",
            Self::MidWidth => "Mid width",
            Self::HighGain => "High gain",
            Self::HighFrequency => "High frequency",
            Self::HighWidth => "High width",
            Self::Waveform => "Waveform",
            Self::PulseWidth => "Pulse width",
            Self::Threshold => "Threshold",
            Self::DryMix => "Dry mix",
        }
    }

    pub fn range(&self) -> RangeInclusive<i16> {
        match self {
            Self::LowGain | Self::MidGain | Self::HighGain => -12..=12,
            Self::LowFrequency => 0..=88,
            Self::MidFrequency => 86..=184,
            Self::HighFrequency => 182..=240,
            Self::LowWidth | Self::MidWidth | Self::HighWidth => 0..=32,
            Self::Waveform => 0..=3,
            Self::PulseWidth => 0..=100,
            Self::Threshold | Self::DryMix => -36..=0,
        }
    }

    pub fn default_value(&self) -> i16 {
        match self {
            Self::LowGain => -10,
            Self::LowFrequency => 88,
            Self::LowWidth => 0,
            Self::MidGain => 5,
            Self::MidFrequency => 173,
            Self::MidWidth => 32,
            Self::HighGain => 0,
            Self::HighFrequency => 182,
            Self::HighWidth => 0,
            Self::Waveform => 0,
            Self::PulseWidth => 50,
            Self::Threshold => -36,
            Self::DryMix => -6,
        }
    }

    pub fn command_for_value(&self, value: i16) -> PersonalCommand {
        let clamped = value.clamp(*self.range().start(), *self.range().end());
        match self {
            Self::LowGain => PersonalCommand::SetRobotGain(RobotRange::Low, clamped as i8),
            Self::LowFrequency => PersonalCommand::SetRobotFreq(RobotRange::Low, clamped as u8),
            Self::LowWidth => PersonalCommand::SetRobotWidth(RobotRange::Low, clamped as u8),
            Self::MidGain => PersonalCommand::SetRobotGain(RobotRange::Medium, clamped as i8),
            Self::MidFrequency => PersonalCommand::SetRobotFreq(RobotRange::Medium, clamped as u8),
            Self::MidWidth => PersonalCommand::SetRobotWidth(RobotRange::Medium, clamped as u8),
            Self::HighGain => PersonalCommand::SetRobotGain(RobotRange::High, clamped as i8),
            Self::HighFrequency => PersonalCommand::SetRobotFreq(RobotRange::High, clamped as u8),
            Self::HighWidth => PersonalCommand::SetRobotWidth(RobotRange::High, clamped as u8),
            Self::Waveform => PersonalCommand::SetRobotWaveform(clamped as u8),
            Self::PulseWidth => PersonalCommand::SetRobotPulseWidth(clamped as u8),
            Self::Threshold => PersonalCommand::SetRobotThreshold(clamped as i8),
            Self::DryMix => PersonalCommand::SetRobotDryMix(clamped as i8),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectsHardTuneSlider {
    Amount,
    Rate,
    Window,
}

impl EffectsHardTuneSlider {
    pub fn full_sliders() -> Vec<Self> {
        vec![Self::Amount, Self::Rate, Self::Window]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Amount => "Amount",
            Self::Rate => "Rate",
            Self::Window => "Window",
        }
    }

    pub fn range(&self) -> RangeInclusive<i16> {
        match self {
            Self::Amount | Self::Rate => 0..=100,
            Self::Window => 0..=600,
        }
    }

    pub fn default_value(&self) -> i16 {
        match self {
            Self::Amount | Self::Rate => 50,
            Self::Window => 200,
        }
    }

    pub fn command_for_value(&self, value: i16) -> PersonalCommand {
        let clamped = value.clamp(*self.range().start(), *self.range().end());
        match self {
            Self::Amount => PersonalCommand::SetHardTuneAmount(clamped as u8),
            Self::Rate => PersonalCommand::SetHardTuneRate(clamped as u8),
            Self::Window => PersonalCommand::SetHardTuneWindow(clamped as u16),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectsAmountControl {
    ReverbAmount,
    EchoAmount,
    PitchAmount,
    GenderAmount,
    MegaphoneAmount,
}

impl EffectsAmountControl {
    pub fn full_controls() -> Vec<Self> {
        vec![
            Self::ReverbAmount,
            Self::EchoAmount,
            Self::PitchAmount,
            Self::GenderAmount,
            Self::MegaphoneAmount,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::ReverbAmount => "Reverb amount",
            Self::EchoAmount => "Echo amount",
            Self::PitchAmount => "Pitch amount",
            Self::GenderAmount => "Gender amount",
            Self::MegaphoneAmount => "Megaphone amount",
        }
    }

    pub fn range(&self) -> RangeInclusive<i16> {
        match self {
            Self::PitchAmount | Self::GenderAmount => -50..=50,
            Self::ReverbAmount | Self::EchoAmount | Self::MegaphoneAmount => 0..=100,
        }
    }

    pub fn default_value(&self) -> i16 {
        match self {
            Self::ReverbAmount => 25,
            Self::EchoAmount => 0,
            Self::PitchAmount | Self::GenderAmount => 0,
            Self::MegaphoneAmount => 35,
        }
    }

    pub fn command_for_value(&self, value: i16) -> PersonalCommand {
        let clamped = value.clamp(*self.range().start(), *self.range().end());
        match self {
            Self::ReverbAmount => PersonalCommand::SetReverbAmount(clamped as u8),
            Self::EchoAmount => PersonalCommand::SetEchoAmount(clamped as u8),
            Self::PitchAmount => PersonalCommand::SetPitchAmount(clamped as i8),
            Self::GenderAmount => PersonalCommand::SetGenderAmount(clamped as i8),
            Self::MegaphoneAmount => PersonalCommand::SetMegaphoneAmount(clamped as u8),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectsStyleGroup {
    Reverb,
    Echo,
    Pitch,
    Gender,
    Megaphone,
    Robot,
    HardTune,
}

impl EffectsStyleGroup {
    pub fn full_groups() -> Vec<Self> {
        vec![
            Self::Reverb,
            Self::Echo,
            Self::Pitch,
            Self::Gender,
            Self::Megaphone,
            Self::Robot,
            Self::HardTune,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Reverb => "Reverb style",
            Self::Echo => "Echo style",
            Self::Pitch => "Pitch style",
            Self::Gender => "Gender style",
            Self::Megaphone => "Megaphone style",
            Self::Robot => "Robot style",
            Self::HardTune => "Hard tune style",
        }
    }

    pub fn commands(&self) -> Vec<PersonalCommand> {
        match self {
            Self::Reverb => vec![
                PersonalCommand::SetReverbStyle(ReverbStyle::Library),
                PersonalCommand::SetReverbStyle(ReverbStyle::DarkBloom),
                PersonalCommand::SetReverbStyle(ReverbStyle::MusicClub),
                PersonalCommand::SetReverbStyle(ReverbStyle::RealPlate),
                PersonalCommand::SetReverbStyle(ReverbStyle::Chapel),
                PersonalCommand::SetReverbStyle(ReverbStyle::HockeyArena),
            ],
            Self::Echo => vec![
                PersonalCommand::SetEchoStyle(EchoStyle::Quarter),
                PersonalCommand::SetEchoStyle(EchoStyle::Eighth),
                PersonalCommand::SetEchoStyle(EchoStyle::Triplet),
                PersonalCommand::SetEchoStyle(EchoStyle::PingPong),
                PersonalCommand::SetEchoStyle(EchoStyle::ClassicSlap),
                PersonalCommand::SetEchoStyle(EchoStyle::MultiTap),
            ],
            Self::Pitch => vec![
                PersonalCommand::SetPitchStyle(PitchStyle::Narrow),
                PersonalCommand::SetPitchStyle(PitchStyle::Wide),
            ],
            Self::Gender => vec![
                PersonalCommand::SetGenderStyle(GenderStyle::Narrow),
                PersonalCommand::SetGenderStyle(GenderStyle::Medium),
                PersonalCommand::SetGenderStyle(GenderStyle::Wide),
            ],
            Self::Megaphone => vec![
                PersonalCommand::SetMegaphoneStyle(MegaphoneStyle::Megaphone),
                PersonalCommand::SetMegaphoneStyle(MegaphoneStyle::Radio),
                PersonalCommand::SetMegaphoneStyle(MegaphoneStyle::OnThePhone),
                PersonalCommand::SetMegaphoneStyle(MegaphoneStyle::Overdrive),
                PersonalCommand::SetMegaphoneStyle(MegaphoneStyle::BuzzCutt),
                PersonalCommand::SetMegaphoneStyle(MegaphoneStyle::Tweed),
            ],
            Self::Robot => vec![
                PersonalCommand::SetRobotStyle(RobotStyle::Robot1),
                PersonalCommand::SetRobotStyle(RobotStyle::Robot2),
                PersonalCommand::SetRobotStyle(RobotStyle::Robot3),
            ],
            Self::HardTune => vec![
                PersonalCommand::SetHardTuneStyle(HardTuneStyle::Natural),
                PersonalCommand::SetHardTuneStyle(HardTuneStyle::Medium),
                PersonalCommand::SetHardTuneStyle(HardTuneStyle::Hard),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightingQuickTheme {
    name: &'static str,
    description: &'static str,
    commands: Vec<PersonalCommand>,
}

impl LightingQuickTheme {
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

    pub fn daily_themes() -> Vec<Self> {
        vec![
            Self::new(
                "Dim White",
                "Low-key neutral lighting for calls and late desktop use.",
                vec![
                    PersonalCommand::SetAnimationMode(AnimationMode::Simple),
                    PersonalCommand::SetGlobalColour("404040".to_string()),
                    PersonalCommand::SetAllFaderColours("606060".to_string(), "202020".to_string()),
                    PersonalCommand::SetButtonGroupColours(
                        ButtonColourGroups::FaderMute,
                        "404040".to_string(),
                        Some("101010".to_string()),
                    ),
                    PersonalCommand::SetSimpleColour(
                        SimpleColourTargets::Accent,
                        "808080".to_string(),
                    ),
                ],
            ),
            Self::new(
                "Broadcast Red",
                "Warm red theme for recording or stream mode.",
                vec![
                    PersonalCommand::SetAnimationMode(AnimationMode::Simple),
                    PersonalCommand::SetGlobalColour("FF1F1F".to_string()),
                    PersonalCommand::SetAllFaderColours("FF3030".to_string(), "400000".to_string()),
                    PersonalCommand::SetButtonGroupColours(
                        ButtonColourGroups::EffectTypes,
                        "FF3030".to_string(),
                        Some("400000".to_string()),
                    ),
                    PersonalCommand::SetSimpleColour(
                        SimpleColourTargets::Accent,
                        "FF8080".to_string(),
                    ),
                ],
            ),
            Self::new(
                "Cool Blue",
                "Calm blue/cyan theme for normal desktop work.",
                vec![
                    PersonalCommand::SetAnimationMode(AnimationMode::Simple),
                    PersonalCommand::SetGlobalColour("1F6FFF".to_string()),
                    PersonalCommand::SetAllFaderColours("2E8BFF".to_string(), "002040".to_string()),
                    PersonalCommand::SetButtonGroupColours(
                        ButtonColourGroups::EffectSelector,
                        "00A8FF".to_string(),
                        Some("001A33".to_string()),
                    ),
                    PersonalCommand::SetSimpleColour(
                        SimpleColourTargets::Accent,
                        "80DFFF".to_string(),
                    ),
                ],
            ),
            Self::new(
                "Lights Off",
                "Disable animated lighting and set visible groups to black.",
                vec![
                    PersonalCommand::SetAnimationMode(AnimationMode::None),
                    PersonalCommand::SetGlobalColour("000000".to_string()),
                    PersonalCommand::SetAllFaderColours("000000".to_string(), "000000".to_string()),
                    PersonalCommand::SetAllFaderDisplayStyle(FaderDisplayStyle::TwoColour),
                    PersonalCommand::SetButtonGroupColours(
                        ButtonColourGroups::FaderMute,
                        "000000".to_string(),
                        Some("000000".to_string()),
                    ),
                    PersonalCommand::SetButtonGroupColours(
                        ButtonColourGroups::EffectSelector,
                        "000000".to_string(),
                        Some("000000".to_string()),
                    ),
                    PersonalCommand::SetButtonGroupColours(
                        ButtonColourGroups::EffectTypes,
                        "000000".to_string(),
                        Some("000000".to_string()),
                    ),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightingSimpleColourTarget {
    label: &'static str,
    target: SimpleColourTargets,
}

impl LightingSimpleColourTarget {
    pub fn all_targets() -> Vec<Self> {
        vec![
            Self::new("Global", SimpleColourTargets::Global),
            Self::new("Accent", SimpleColourTargets::Accent),
            Self::new("Scribble 1", SimpleColourTargets::Scribble1),
            Self::new("Scribble 2", SimpleColourTargets::Scribble2),
            Self::new("Scribble 3", SimpleColourTargets::Scribble3),
            Self::new("Scribble 4", SimpleColourTargets::Scribble4),
        ]
    }

    pub fn new(label: &'static str, target: SimpleColourTargets) -> Self {
        Self { label, target }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn target(&self) -> SimpleColourTargets {
        self.target
    }

    pub fn command_for_colour(&self, colour: impl Into<String>) -> PersonalCommand {
        PersonalCommand::SetSimpleColour(self.target, colour.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightingFaderColourTarget {
    All,
    Fader(FaderName),
}

impl LightingFaderColourTarget {
    pub fn all_targets() -> Vec<Self> {
        vec![
            Self::All,
            Self::Fader(FaderName::A),
            Self::Fader(FaderName::B),
            Self::Fader(FaderName::C),
            Self::Fader(FaderName::D),
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All faders",
            Self::Fader(FaderName::A) => "Fader A",
            Self::Fader(FaderName::B) => "Fader B",
            Self::Fader(FaderName::C) => "Fader C",
            Self::Fader(FaderName::D) => "Fader D",
        }
    }

    pub fn colour_command(
        &self,
        top: impl Into<String>,
        bottom: impl Into<String>,
    ) -> PersonalCommand {
        match self {
            Self::All => PersonalCommand::SetAllFaderColours(top.into(), bottom.into()),
            Self::Fader(fader) => {
                PersonalCommand::SetFaderColours(*fader, top.into(), bottom.into())
            }
        }
    }

    pub fn display_command(&self, style: FaderDisplayStyle) -> PersonalCommand {
        match self {
            Self::All => PersonalCommand::SetAllFaderDisplayStyle(style),
            Self::Fader(fader) => PersonalCommand::SetFaderDisplayStyle(*fader, style),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightingButtonColourTarget {
    Group(&'static str, ButtonColourGroups),
    Button(&'static str, Button),
}

impl LightingButtonColourTarget {
    pub fn daily_targets() -> Vec<Self> {
        vec![
            Self::Group("Fader mutes", ButtonColourGroups::FaderMute),
            Self::Group("Effect selectors", ButtonColourGroups::EffectSelector),
            Self::Group("Effect types", ButtonColourGroups::EffectTypes),
            Self::Button("Cough button", Button::Cough),
            Self::Button("Bleep button", Button::Bleep),
            Self::Button("Effect preset 1", Button::EffectSelect1),
            Self::Button("Effect preset 2", Button::EffectSelect2),
            Self::Button("Effect preset 3", Button::EffectSelect3),
            Self::Button("Effect preset 4", Button::EffectSelect4),
            Self::Button("Effect preset 5", Button::EffectSelect5),
            Self::Button("Effect preset 6", Button::EffectSelect6),
            Self::Button("FX button", Button::EffectFx),
            Self::Button("Megaphone button", Button::EffectMegaphone),
            Self::Button("Robot button", Button::EffectRobot),
            Self::Button("Hard Tune button", Button::EffectHardTune),
            Self::Button("Sampler top left", Button::SamplerTopLeft),
            Self::Button("Sampler top right", Button::SamplerTopRight),
            Self::Button("Sampler bottom left", Button::SamplerBottomLeft),
            Self::Button("Sampler bottom right", Button::SamplerBottomRight),
            Self::Button("Sampler clear", Button::SamplerClear),
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Group(label, _) | Self::Button(label, _) => label,
        }
    }

    pub fn colour_command(
        &self,
        colour_one: impl Into<String>,
        colour_two: impl Into<String>,
    ) -> PersonalCommand {
        match self {
            Self::Group(_, group) => PersonalCommand::SetButtonGroupColours(
                *group,
                colour_one.into(),
                Some(colour_two.into()),
            ),
            Self::Button(_, button) => PersonalCommand::SetButtonColours(
                *button,
                colour_one.into(),
                Some(colour_two.into()),
            ),
        }
    }

    pub fn off_style_command(&self, off_style: ButtonColourOffStyle) -> PersonalCommand {
        match self {
            Self::Group(_, group) => PersonalCommand::SetButtonGroupOffStyle(*group, off_style),
            Self::Button(_, button) => PersonalCommand::SetButtonOffStyle(*button, off_style),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightingTripleColourTarget {
    Encoder(&'static str, EncoderColourTargets),
    Sampler(&'static str, SamplerColourTargets),
}

impl LightingTripleColourTarget {
    pub fn all_targets() -> Vec<Self> {
        vec![
            Self::Encoder("Reverb encoder", EncoderColourTargets::Reverb),
            Self::Encoder("Pitch encoder", EncoderColourTargets::Pitch),
            Self::Encoder("Echo encoder", EncoderColourTargets::Echo),
            Self::Encoder("Gender encoder", EncoderColourTargets::Gender),
            Self::Sampler("Sampler select A", SamplerColourTargets::SamplerSelectA),
            Self::Sampler("Sampler select B", SamplerColourTargets::SamplerSelectB),
            Self::Sampler("Sampler select C", SamplerColourTargets::SamplerSelectC),
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Encoder(label, _) | Self::Sampler(label, _) => label,
        }
    }

    pub fn colour_command(
        &self,
        colour_one: impl Into<String>,
        colour_two: impl Into<String>,
        colour_three: impl Into<String>,
    ) -> PersonalCommand {
        match self {
            Self::Encoder(_, target) => PersonalCommand::SetEncoderColour(
                *target,
                colour_one.into(),
                colour_two.into(),
                colour_three.into(),
            ),
            Self::Sampler(_, target) => PersonalCommand::SetSampleColour(
                *target,
                colour_one.into(),
                colour_two.into(),
                colour_three.into(),
            ),
        }
    }

    pub fn off_style_command(&self, off_style: ButtonColourOffStyle) -> Option<PersonalCommand> {
        match self {
            Self::Encoder(_, _) => None,
            Self::Sampler(_, target) => {
                Some(PersonalCommand::SetSampleOffStyle(*target, off_style))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightingAnimationControl {
    Mode,
    Mod1,
    Mod2,
    Waterfall,
}

impl LightingAnimationControl {
    pub fn practical_controls() -> Vec<Self> {
        vec![Self::Mode, Self::Mod1, Self::Mod2, Self::Waterfall]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Mode => "Mode",
            Self::Mod1 => "Mod 1",
            Self::Mod2 => "Mod 2",
            Self::Waterfall => "Waterfall",
        }
    }

    pub fn command_for_value(&self, value: u8) -> PersonalCommand {
        match self {
            Self::Mode => {
                let modes = [
                    AnimationMode::Simple,
                    AnimationMode::RainbowDark,
                    AnimationMode::Ripple,
                    AnimationMode::RetroRainbow,
                    AnimationMode::None,
                ];
                let index = usize::from(value).min(modes.len() - 1);
                PersonalCommand::SetAnimationMode(modes[index])
            }
            Self::Mod1 => PersonalCommand::SetAnimationMod1(value.min(100)),
            Self::Mod2 => PersonalCommand::SetAnimationMod2(value.min(100)),
            Self::Waterfall => PersonalCommand::SetAnimationWaterfall(if value == 0 {
                WaterfallDirection::Down
            } else {
                WaterfallDirection::Up
            }),
        }
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
        let app = self.app.trim();
        if !self.enabled || app.is_empty() {
            return false;
        }
        stream
            .display_name
            .to_ascii_lowercase()
            .contains(&app.to_ascii_lowercase())
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
    pub sink_name: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingRuleDiffStatus {
    Matched,
    NeedsMove,
    WaitingForStream,
    MissingTarget,
    Disabled,
}

impl RoutingRuleDiffStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Matched => "Matched",
            Self::NeedsMove => "Move needed",
            Self::WaitingForStream => "Waiting",
            Self::MissingTarget => "No route target",
            Self::Disabled => "Disabled",
        }
    }

    pub fn color(&self) -> egui::Color32 {
        match self {
            Self::Matched => egui::Color32::from_rgb(150, 255, 185),
            Self::NeedsMove => egui::Color32::from_rgb(255, 205, 110),
            Self::WaitingForStream => egui::Color32::from_rgb(180, 195, 205),
            Self::MissingTarget => egui::Color32::from_rgb(255, 135, 120),
            Self::Disabled => egui::Color32::from_rgb(140, 145, 150),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingRuleDiffRow {
    app: String,
    desired_route: String,
    current_route: Option<String>,
    status: RoutingRuleDiffStatus,
}

impl RoutingRuleDiffRow {
    pub fn new(
        app: impl Into<String>,
        desired_route: impl Into<String>,
        current_route: Option<String>,
        status: RoutingRuleDiffStatus,
    ) -> Self {
        Self {
            app: app.into(),
            desired_route: desired_route.into(),
            current_route,
            status,
        }
    }

    pub fn app(&self) -> &str {
        &self.app
    }

    pub fn desired_route(&self) -> &str {
        &self.desired_route
    }

    pub fn current_route(&self) -> Option<&str> {
        self.current_route.as_deref()
    }

    pub fn status(&self) -> RoutingRuleDiffStatus {
        self.status
    }

    pub fn status_label(&self) -> &'static str {
        self.status.label()
    }

    pub fn status_color(&self) -> egui::Color32 {
        self.status.color()
    }

    pub fn summary(&self) -> String {
        match self.status {
            RoutingRuleDiffStatus::Matched => format!("{}: {} ✓", self.app, self.desired_route),
            RoutingRuleDiffStatus::NeedsMove => format!(
                "{}: {} → {}",
                self.app,
                self.current_route.as_deref().unwrap_or("Unknown"),
                self.desired_route
            ),
            RoutingRuleDiffStatus::WaitingForStream => {
                format!("{}: waiting for {} stream", self.app, self.desired_route)
            }
            RoutingRuleDiffStatus::MissingTarget => {
                format!("{}: {} target unavailable", self.app, self.desired_route)
            }
            RoutingRuleDiffStatus::Disabled => {
                format!("{}: disabled rule for {}", self.app, self.desired_route)
            }
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

        let mut sinks_by_index = HashMap::new();
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
            sinks_by_index.insert(index, (sink_name.to_string(), label));
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
            let (sink_name, sink_label) = sink_id
                .and_then(|id| sinks_by_index.get(&id).cloned())
                .map(|(name, label)| (Some(name), label))
                .unwrap_or_else(|| (None, "Unknown output".to_string()));

            streams.push(AudioStream {
                id,
                app_name: app_name.map(ToOwned::to_owned),
                display_name,
                sink_name,
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
                if self
                    .current_route_label_for_stream(stream)
                    .is_some_and(|route| route.eq_ignore_ascii_case(&target.label))
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

    pub fn routing_rule_diffs(&self, rules: &[AudioRoutingRule]) -> Vec<RoutingRuleDiffRow> {
        rules
            .iter()
            .map(|rule| {
                let desired_target_exists = self
                    .route_targets
                    .iter()
                    .any(|target| target.label.eq_ignore_ascii_case(&rule.route));
                if !rule.enabled {
                    return RoutingRuleDiffRow::new(
                        rule.app.clone(),
                        rule.route.clone(),
                        None,
                        RoutingRuleDiffStatus::Disabled,
                    );
                }
                if !desired_target_exists {
                    return RoutingRuleDiffRow::new(
                        rule.app.clone(),
                        rule.route.clone(),
                        None,
                        RoutingRuleDiffStatus::MissingTarget,
                    );
                }

                let Some(stream) = self
                    .streams
                    .iter()
                    .find(|stream| rule.matches_stream(stream))
                else {
                    return RoutingRuleDiffRow::new(
                        rule.app.clone(),
                        rule.route.clone(),
                        None,
                        RoutingRuleDiffStatus::WaitingForStream,
                    );
                };

                let current_route = self
                    .current_route_label_for_stream(stream)
                    .map(str::to_string);
                let status = if current_route
                    .as_deref()
                    .is_some_and(|route| route.eq_ignore_ascii_case(&rule.route))
                {
                    RoutingRuleDiffStatus::Matched
                } else {
                    RoutingRuleDiffStatus::NeedsMove
                };

                RoutingRuleDiffRow::new(rule.app.clone(), rule.route.clone(), current_route, status)
            })
            .collect()
    }

    fn current_route_label_for_stream(&self, stream: &AudioStream) -> Option<&str> {
        self.route_targets
            .iter()
            .find(|target| {
                stream.sink_name.as_deref() == Some(target.sink_name.as_str())
                    || stream
                        .sink_label
                        .to_ascii_lowercase()
                        .contains(&target.label.to_ascii_lowercase())
            })
            .map(|target| target.label.as_str())
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
    pub routing_matrix_routes: Vec<RoutingMatrixRoute>,
    pub clip_guard_enabled: bool,
    pub clip_guard_threshold: u8,
    pub headphone_limiter_enabled: bool,
    pub headphone_limiter_threshold: u8,
    pub headphone_eq_enabled: bool,
    pub headphone_eq_backend: Option<String>,
    pub headphone_eq_profile: Option<String>,
    pub system_settings: Option<SystemSettingsSnapshot>,
    pub active_audio_streams: ActiveAudioStreams,
    pub active_audio_error: Option<String>,
    pub sampler_slots: Vec<SamplerSlotSnapshot>,
    pub submix_channels: Vec<SubmixChannelSnapshot>,
    pub submix_outputs: Vec<SubmixOutputSnapshot>,
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
            routing_matrix_routes: Vec::new(),
            clip_guard_enabled: false,
            clip_guard_threshold: 0,
            headphone_limiter_enabled: false,
            headphone_limiter_threshold: 0,
            headphone_eq_enabled: false,
            headphone_eq_backend: None,
            headphone_eq_profile: None,
            system_settings: None,
            active_audio_streams: ActiveAudioStreams::default(),
            active_audio_error: None,
            sampler_slots: Vec::new(),
            submix_channels: Vec::new(),
            submix_outputs: Vec::new(),
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
            routing_matrix_routes: RoutingMatrixModel::cells()
                .into_iter()
                .map(|cell| {
                    RoutingMatrixRoute::new(
                        cell.input(),
                        cell.output(),
                        mixer.router[cell.input()][cell.output()],
                    )
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
            system_settings: Some(SystemSettingsSnapshot::new(
                settings.mute_hold_duration,
                settings.vc_mute_also_mute_cm,
                settings.enable_monitor_with_fx,
                settings.lock_faders,
                settings.vod_mode,
            )),
            active_audio_streams: ActiveAudioStreams::default(),
            active_audio_error: None,
            sampler_slots: mixer
                .sampler
                .as_ref()
                .map(SamplerSlotSnapshot::from_sampler)
                .unwrap_or_default(),
            submix_channels: mixer
                .levels
                .submix
                .as_ref()
                .map(SubmixChannelSnapshot::from_submixes)
                .unwrap_or_default(),
            submix_outputs: mixer
                .levels
                .submix
                .as_ref()
                .map(SubmixOutputSnapshot::from_submixes)
                .unwrap_or_default(),
        }
    }

    pub fn submix_channel_state(&self, channel: ChannelName) -> Option<String> {
        self.submix_channels
            .iter()
            .find(|snapshot| snapshot.channel() == channel)
            .map(SubmixChannelSnapshot::state_label)
    }

    pub fn submix_output_state(&self, output: OutputDevice) -> Option<String> {
        self.submix_outputs
            .iter()
            .find(|snapshot| snapshot.output() == output)
            .map(SubmixOutputSnapshot::state_label)
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

    pub fn diagnostics_rows(&self) -> Vec<DiagnosticsStatusRow> {
        let connection_value = if self.connected {
            "Connected".to_string()
        } else {
            format!(
                "Disconnected: {}",
                self.error.as_deref().unwrap_or("unknown error")
            )
        };
        let connection_severity = if self.connected {
            DiagnosticsStatusSeverity::Ok
        } else {
            DiagnosticsStatusSeverity::Warning
        };

        let daemon_value = self
            .daemon_version
            .as_deref()
            .unwrap_or("Unknown")
            .to_string();
        let daemon_severity = if self.daemon_version.is_some() {
            DiagnosticsStatusSeverity::Ok
        } else {
            DiagnosticsStatusSeverity::Warning
        };

        let device_value = match (self.device_type.as_deref(), self.device_serial.as_deref()) {
            (Some(device), Some(serial)) => format!("{device} ({serial})"),
            (Some(device), None) => device.to_string(),
            (None, Some(serial)) => format!("GoXLR ({serial})"),
            (None, None) => "No selected device".to_string(),
        };
        let device_severity = if self.device_serial.is_some() {
            DiagnosticsStatusSeverity::Ok
        } else {
            DiagnosticsStatusSeverity::Warning
        };

        let audio_value = self
            .active_audio_error
            .clone()
            .unwrap_or_else(|| self.active_audio_streams.summary());

        vec![
            DiagnosticsStatusRow::new("Connection", connection_value, connection_severity),
            DiagnosticsStatusRow::new("Daemon", daemon_value, daemon_severity),
            DiagnosticsStatusRow::new("Device", device_value, device_severity),
            DiagnosticsStatusRow::new(
                "Detected devices",
                self.device_serials.len().to_string(),
                DiagnosticsStatusSeverity::Info,
            ),
            DiagnosticsStatusRow::new(
                "Profiles",
                self.profile_name
                    .as_deref()
                    .unwrap_or("No profile reported"),
                if self.profile_name.is_some() {
                    DiagnosticsStatusSeverity::Ok
                } else {
                    DiagnosticsStatusSeverity::Info
                },
            ),
            DiagnosticsStatusRow::new(
                "Mic profile",
                self.mic_profile_name
                    .as_deref()
                    .unwrap_or("No mic profile reported"),
                if self.mic_profile_name.is_some() {
                    DiagnosticsStatusSeverity::Ok
                } else {
                    DiagnosticsStatusSeverity::Info
                },
            ),
            DiagnosticsStatusRow::new(
                "Headphone EQ",
                self.headphone_eq_profile
                    .as_deref()
                    .unwrap_or("No headphone EQ profile reported"),
                if self.headphone_eq_profile.is_some() {
                    DiagnosticsStatusSeverity::Ok
                } else {
                    DiagnosticsStatusSeverity::Info
                },
            ),
            DiagnosticsStatusRow::new(
                "Desktop audio",
                audio_value,
                if self.active_audio_error.is_some() {
                    DiagnosticsStatusSeverity::Warning
                } else {
                    DiagnosticsStatusSeverity::Info
                },
            ),
        ]
    }

    pub fn volume_for(&self, channel: ChannelName) -> Option<u8> {
        self.channel_volumes
            .iter()
            .find(|volume| volume.channel == channel)
            .map(|volume| volume.value)
    }

    pub fn routing_enabled_for(&self, input: InputDevice, output: OutputDevice) -> Option<bool> {
        self.routing_matrix_routes
            .iter()
            .find(|route| route.input() == input && route.output() == output)
            .map(RoutingMatrixRoute::enabled)
    }

    pub fn routing_state_label(&self, input: InputDevice, output: OutputDevice) -> &'static str {
        self.routing_state_badge(input, output).label()
    }

    pub fn routing_state_badge(
        &self,
        input: InputDevice,
        output: OutputDevice,
    ) -> RoutingStateBadge {
        RoutingStateBadge::for_state(self.routing_enabled_for(input, output))
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
    Lighting,
    HeadphoneEq,
    Sampler,
    System,
    Diagnostics,
    About,
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

#[derive(Debug, Clone, PartialEq)]
pub struct PersonalPreset {
    name: &'static str,
    description: &'static str,
    safety_preset: bool,
    commands: Vec<PersonalCommand>,
}

impl PersonalPreset {
    pub fn new(
        name: &'static str,
        description: &'static str,
        safety_preset: bool,
        commands: Vec<PersonalCommand>,
    ) -> Self {
        Self {
            name,
            description,
            safety_preset,
            commands,
        }
    }

    pub fn daily_presets() -> Vec<Self> {
        vec![
            Self::new(
                "Go Live",
                "Route mic/music/game to broadcast, switch to recording lighting, and keep voice FX clean.",
                false,
                vec![
                    PersonalCommand::SetRouter(
                        InputDevice::Microphone,
                        OutputDevice::BroadcastMix,
                        true,
                    ),
                    PersonalCommand::SetRouter(
                        InputDevice::Music,
                        OutputDevice::BroadcastMix,
                        true,
                    ),
                    PersonalCommand::SetRouter(InputDevice::Game, OutputDevice::BroadcastMix, true),
                    PersonalCommand::SetRouter(
                        InputDevice::Chat,
                        OutputDevice::BroadcastMix,
                        false,
                    ),
                    PersonalCommand::SetMonitorMix(OutputDevice::Headphones),
                    PersonalCommand::SetAnimationMode(AnimationMode::Simple),
                    PersonalCommand::SetGlobalColour("FF1F1F".to_string()),
                    PersonalCommand::SetAllFaderColours("FF3030".to_string(), "400000".to_string()),
                    PersonalCommand::SetFXEnabled(false),
                    PersonalCommand::SetHardTuneEnabled(false),
                ],
            ),
            Self::new(
                "Desktop Focus",
                "Keep routing local to headphones, use calm blue lighting, and disable voice FX distractions.",
                false,
                vec![
                    PersonalCommand::SetRouter(InputDevice::Music, OutputDevice::Headphones, true),
                    PersonalCommand::SetRouter(InputDevice::Game, OutputDevice::Headphones, true),
                    PersonalCommand::SetRouter(InputDevice::Chat, OutputDevice::Headphones, true),
                    PersonalCommand::SetMonitorMix(OutputDevice::Headphones),
                    PersonalCommand::SetAnimationMode(AnimationMode::Simple),
                    PersonalCommand::SetGlobalColour("1F6FFF".to_string()),
                    PersonalCommand::SetAllFaderColours("2E8BFF".to_string(), "002040".to_string()),
                    PersonalCommand::SetFXEnabled(false),
                ],
            ),
            Self::new(
                "Late Night",
                "Drop playback levels, keep limiter/EQ safety on, and use lights-off styling.",
                false,
                vec![
                    PersonalCommand::SetVolume(ChannelName::Music, 35),
                    PersonalCommand::SetVolume(ChannelName::Game, 35),
                    PersonalCommand::SetVolume(ChannelName::Chat, 45),
                    PersonalCommand::SetVolume(ChannelName::Headphones, 55),
                    PersonalCommand::SetHeadphoneLimiterEnabled(true),
                    PersonalCommand::SetHeadphoneEqEnabled(true),
                    PersonalCommand::SetAnimationMode(AnimationMode::None),
                    PersonalCommand::SetGlobalColour("000000".to_string()),
                    PersonalCommand::SetFXEnabled(false),
                ],
            ),
            Self::new(
                "FX Panic",
                "Immediately return voice effects and lighting to a safe neutral state.",
                true,
                vec![
                    PersonalCommand::SetFXEnabled(false),
                    PersonalCommand::SetMegaphoneEnabled(false),
                    PersonalCommand::SetRobotEnabled(false),
                    PersonalCommand::SetHardTuneEnabled(false),
                    PersonalCommand::SetReverbAmount(0),
                    PersonalCommand::SetEchoAmount(0),
                    PersonalCommand::SetPitchAmount(0),
                    PersonalCommand::SetGenderAmount(0),
                    PersonalCommand::SetAnimationMode(AnimationMode::Simple),
                    PersonalCommand::SetGlobalColour("404040".to_string()),
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

    pub fn is_safety_preset(&self) -> bool {
        self.safety_preset
    }

    pub fn commands(&self) -> Vec<PersonalCommand> {
        self.commands.clone()
    }

    pub fn to_scene(&self) -> UiScene {
        UiScene::new(self.name, self.commands())
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
            AppViewMode::Mic
            | AppViewMode::Effects
            | AppViewMode::Lighting
            | AppViewMode::HeadphoneEq
            | AppViewMode::Sampler
            | AppViewMode::System
            | AppViewMode::Diagnostics
            | AppViewMode::About
            | AppViewMode::Full => AppViewMode::QuickActions,
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

    pub fn personal_preset_buttons(presets: &[PersonalPreset]) -> Vec<PersonalPreset> {
        let mut selected = Vec::new();
        if let Some(safety) = presets.iter().find(|preset| preset.is_safety_preset()) {
            selected.push(safety.clone());
        }
        for preset in presets {
            if selected.len() >= 4 {
                break;
            }
            if !preset.is_safety_preset() {
                selected.push(preset.clone());
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MiniWindowMode {
    mini: bool,
    always_on_top: bool,
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
    Snapshot(Box<AppSnapshot>),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PendingConfirmationKind {
    MainProfile,
    LightingProfile,
    MicProfile,
    EffectPreset,
    HeadphoneEqProfile,
    SamplerFile,
}

impl From<ProfileBrowserKind> for PendingConfirmationKind {
    fn from(kind: ProfileBrowserKind) -> Self {
        match kind {
            ProfileBrowserKind::Main => Self::MainProfile,
            ProfileBrowserKind::LightingColours => Self::LightingProfile,
            ProfileBrowserKind::Mic => Self::MicProfile,
            ProfileBrowserKind::EffectsPreset => Self::EffectPreset,
            ProfileBrowserKind::HeadphoneEq => Self::HeadphoneEqProfile,
        }
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
    pending_confirmations: HashMap<PendingConfirmationKind, PersonalCommand>,
    sampler_file_path: String,
    sampler_browser_path: String,
    diagnostics_log: Vec<DiagnosticsLogEntry>,
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
            pending_confirmations: HashMap::new(),
            sampler_file_path: String::new(),
            sampler_browser_path: Self::default_sampler_browser_path()
                .to_string_lossy()
                .to_string(),
            diagnostics_log: vec![DiagnosticsLogEntry::new(
                "00:00",
                DiagnosticsStatusSeverity::Info,
                "App",
                "personal UI started; waiting for daemon status",
            )],
        }
    }

    fn drain_events(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                WorkerEvent::Snapshot(snapshot) => {
                    let snapshot = *snapshot;
                    let status_line = snapshot.status_line();
                    self.device_selection
                        .sync_available_devices(snapshot.device_serials.clone());
                    self.pending_volumes = snapshot.channel_volumes.clone();
                    self.snapshot = snapshot;
                    self.record_diagnostics_log(
                        DiagnosticsStatusSeverity::Info,
                        "Snapshot",
                        status_line,
                    );
                }
                WorkerEvent::Error(error) => {
                    self.record_diagnostics_log(
                        DiagnosticsStatusSeverity::Warning,
                        "IPC error",
                        error.clone(),
                    );
                    self.snapshot = AppSnapshot::disconnected(error);
                    self.pending_volumes = self.snapshot.channel_volumes.clone();
                }
            }
        }
    }

    fn diagnostics_timestamp(&self) -> String {
        let elapsed = self.started_at.elapsed().as_secs();
        format!("{:02}:{:02}", elapsed / 60, elapsed % 60)
    }

    fn record_diagnostics_log(
        &mut self,
        severity: DiagnosticsStatusSeverity,
        category: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics_log.push(DiagnosticsLogEntry::new(
            self.diagnostics_timestamp(),
            severity,
            category,
            message,
        ));
        let max_entries = DiagnosticsLayoutPolicy::log_row_limit() * 4;
        if self.diagnostics_log.len() > max_entries {
            self.diagnostics_log
                .drain(0..self.diagnostics_log.len() - max_entries);
        }
    }

    fn send(&self, command: UiCommand) {
        let _ = self.commands.send(command);
    }

    fn default_sampler_browser_path() -> &'static Path {
        Path::new("defaults/resources/samples")
    }

    fn default_profile_browser_path(kind: ProfileBrowserKind) -> &'static Path {
        match kind {
            ProfileBrowserKind::Main | ProfileBrowserKind::LightingColours => {
                Path::new("defaults/resources/profiles")
            }
            ProfileBrowserKind::Mic => Path::new("defaults/resources/mic-profiles"),
            ProfileBrowserKind::EffectsPreset => Path::new("defaults/resources/presets"),
            ProfileBrowserKind::HeadphoneEq => {
                Path::new("defaults/resources/headphone-eq-profiles")
            }
        }
    }

    fn sampler_sample_browser(&self) -> SamplerSampleBrowser {
        let browser_path = self.sampler_browser_path.trim();
        let root = if browser_path.is_empty() {
            Self::default_sampler_browser_path().to_path_buf()
        } else {
            PathBuf::from(browser_path)
        };
        SamplerSampleBrowser::from_directory(root)
    }

    fn profile_browser_for(&self, kind: ProfileBrowserKind) -> ProfileBrowser {
        let active_name = match kind {
            ProfileBrowserKind::Main | ProfileBrowserKind::LightingColours => {
                self.snapshot.profile_name.as_deref()
            }
            ProfileBrowserKind::Mic => self.snapshot.mic_profile_name.as_deref(),
            ProfileBrowserKind::EffectsPreset => None,
            ProfileBrowserKind::HeadphoneEq => self.snapshot.headphone_eq_profile.as_deref(),
        };
        ProfileBrowser::from_directory(
            kind,
            active_name,
            Some(Self::default_profile_browser_path(kind)),
        )
    }

    fn pending_confirmation(&self, kind: PendingConfirmationKind) -> Option<&PersonalCommand> {
        self.pending_confirmations.get(&kind)
    }

    fn set_pending_confirmation(
        &mut self,
        kind: PendingConfirmationKind,
        command: Option<PersonalCommand>,
    ) {
        match command {
            Some(c) => {
                self.pending_confirmations.insert(kind, c);
            }
            None => {
                self.pending_confirmations.remove(&kind);
            }
        }
    }

    fn has_pending_confirmation(&self, kind: PendingConfirmationKind) -> bool {
        self.pending_confirmations.contains_key(&kind)
    }

    fn pending_profile_confirmation(&self, kind: ProfileBrowserKind) -> Option<&PersonalCommand> {
        self.pending_confirmation(kind.into())
    }

    fn set_pending_profile_confirmation(
        &mut self,
        kind: ProfileBrowserKind,
        command: Option<PersonalCommand>,
    ) {
        self.set_pending_confirmation(kind.into(), command);
    }

    fn handle_profile_browser_action(
        &mut self,
        kind: ProfileBrowserKind,
        action: &ProfileBrowserAction,
    ) {
        let command = action.command();
        let confirmed = self
            .pending_profile_confirmation(kind)
            .is_some_and(|pending| pending == &command);
        if action.requires_confirmation() && !confirmed {
            self.set_pending_profile_confirmation(kind, Some(command));
        } else {
            self.set_pending_profile_confirmation(kind, None);
            self.send(UiCommand::Send(command));
        }
    }

    fn handle_sampler_file_action(&mut self, action: &SamplerFileAction) {
        let command = action.command();
        let confirmed = self
            .pending_confirmation(PendingConfirmationKind::SamplerFile)
            .is_some_and(|pending| pending == &command);
        if let Some(command) = action.command_if_confirmed(confirmed) {
            self.set_pending_confirmation(PendingConfirmationKind::SamplerFile, None);
            self.send(UiCommand::Send(command));
        } else {
            self.set_pending_confirmation(PendingConfirmationKind::SamplerFile, Some(command));
        }
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
        egui::Color32::from_rgb(232, 132, 86)
    }

    fn bg() -> egui::Color32 {
        egui::Color32::from_rgb(30, 27, 24)
    }

    fn panel_bg() -> egui::Color32 {
        egui::Color32::from_rgb(40, 36, 32)
    }

    fn strip_bg() -> egui::Color32 {
        egui::Color32::from_rgb(50, 45, 40)
    }

    fn panel_border() -> egui::Color32 {
        egui::Color32::from_rgb(70, 62, 54)
    }

    fn muted_text() -> egui::Color32 {
        egui::Color32::from_rgb(168, 158, 144)
    }

    fn diagnostics_severity_color(severity: DiagnosticsStatusSeverity) -> egui::Color32 {
        match severity {
            DiagnosticsStatusSeverity::Ok => Self::accent(),
            DiagnosticsStatusSeverity::Warning => egui::Color32::YELLOW,
            DiagnosticsStatusSeverity::Info => Self::muted_text(),
        }
    }

    pub fn apply_goxlr_style(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = Self::bg();
        style.visuals.window_fill = Self::panel_bg();
        style.visuals.faint_bg_color = egui::Color32::from_rgb(44, 38, 33);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(54, 47, 41);
        style.visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(238, 234, 226);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(72, 60, 50);
        style.visuals.widgets.hovered.fg_stroke.color = Self::accent();
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(78, 46, 28);
        style.visuals.widgets.active.fg_stroke.color = Self::accent();
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 8.0);
        ctx.set_style(style);
    }

    fn panel_frame() -> egui::Frame {
        egui::Frame::new()
            .fill(Self::panel_bg())
            .stroke(egui::Stroke::new(1.0, Self::panel_border()))
            .corner_radius(egui::CornerRadius::same(2))
            .inner_margin(egui::Margin::same(12))
    }

    fn soft_panel_frame() -> egui::Frame {
        egui::Frame::new()
            .fill(Self::strip_bg())
            .stroke(egui::Stroke::new(1.0, Self::panel_border()))
            .corner_radius(egui::CornerRadius::same(2))
            .inner_margin(egui::Margin::same(10))
    }

    fn bounded_panel<R>(
        ui: &mut egui::Ui,
        width: f32,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        let frame = Self::panel_frame();
        let desired_size = egui::vec2(
            width + frame.total_margin().sum().x,
            ContentLayoutPolicy::bounded_panel_outer_min_height(),
        );
        ui.allocate_ui_with_layout(
            desired_size,
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                frame
                    .show(ui, |ui| {
                        ui.set_min_width(width);
                        ui.set_max_width(width);
                        add_contents(ui)
                    })
                    .inner
            },
        )
        .inner
    }

    fn bounded_sized_panel<R>(
        ui: &mut egui::Ui,
        width: f32,
        height: f32,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        let frame = Self::panel_frame();
        ui.allocate_ui_with_layout(
            egui::vec2(width + frame.total_margin().sum().x, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                frame
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(
                            width,
                            (height - frame.total_margin().sum().y).max(0.0),
                        ));
                        ui.set_max_width(width);
                        add_contents(ui)
                    })
                    .inner
            },
        )
        .inner
    }

    fn soft_bounded_panel<R>(
        ui: &mut egui::Ui,
        width: f32,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        let frame = Self::soft_panel_frame();
        let desired_size = egui::vec2(
            width + frame.total_margin().sum().x,
            ContentLayoutPolicy::bounded_panel_outer_min_height(),
        );
        ui.allocate_ui_with_layout(
            desired_size,
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                frame
                    .show(ui, |ui| {
                        ui.set_min_width(width);
                        ui.set_max_width(width);
                        add_contents(ui)
                    })
                    .inner
            },
        )
        .inner
    }

    fn soft_sized_panel<R>(
        ui: &mut egui::Ui,
        width: f32,
        height: f32,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        let frame = Self::soft_panel_frame();
        ui.allocate_ui_with_layout(
            egui::vec2(width + frame.total_margin().sum().x, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                frame
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(
                            width,
                            (height - frame.total_margin().sum().y).max(0.0),
                        ));
                        ui.set_max_width(width);
                        add_contents(ui)
                    })
                    .inner
            },
        )
        .inner
    }

    fn centered_exact_width<R>(
        ui: &mut egui::Ui,
        width: f32,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        let left_margin = ((ui.available_width() - width) / 2.0).max(0.0);
        ui.horizontal_top(|ui| {
            ui.add_space(left_margin);
            ui.allocate_ui_with_layout(
                egui::vec2(width, ui.available_height()),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.set_min_width(width);
                    ui.set_max_width(width);
                    add_contents(ui)
                },
            )
            .inner
        })
        .inner
    }

    fn centered_fixed_row<R>(
        ui: &mut egui::Ui,
        width: f32,
        gap: f32,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        Self::centered_exact_width(ui, width, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                add_contents(ui)
            })
            .inner
        })
    }

    fn mixer_card_row<R>(
        ui: &mut egui::Ui,
        gap: f32,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
            add_contents(ui)
        })
        .inner
    }

    fn polished_row<R>(
        ui: &mut egui::Ui,
        spacing: egui::Vec2,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = spacing;
            ui.with_layout(
                egui::Layout::left_to_right(egui::Align::Min)
                    .with_main_wrap(true)
                    .with_cross_align(egui::Align::Min),
                add_contents,
            )
            .inner
        })
        .inner
    }

    fn content_width(ui: &egui::Ui) -> f32 {
        ContentLayoutPolicy::content_width_for_available_width(ui.available_width())
    }

    fn centered_page_body<R>(
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        let content_width = Self::content_width(ui);
        let side_margin = ContentLayoutPolicy::wide_window_side_margin(ui.available_width());
        ui.horizontal_top(|ui| {
            if ContentLayoutPolicy::page_body_centers_in_wide_windows() && side_margin > 0.0 {
                ui.add_space(side_margin);
            }
            ui.vertical(|ui| {
                ui.set_min_width(content_width);
                ui.set_max_width(content_width);
                add_contents(ui)
            })
            .inner
        })
        .inner
    }

    fn section_header(ui: &mut egui::Ui, title: &str, kicker: &str, description: &str) {
        ui.set_max_width(Self::content_width(ui).min(ContentLayoutPolicy::section_header_width()));
        ui.horizontal_wrapped(|ui| {
            ui.heading(title);
            ui.separator();
            ui.label(kicker);
        });
        if !description.is_empty() {
            ui.add_space(6.0);
            ui.label(description);
        }
    }

    fn compressor_ratio_label(ratio: CompressorRatio) -> &'static str {
        match ratio {
            CompressorRatio::Ratio2_0 => "2:1",
            CompressorRatio::Ratio4_0 => "4:1",
            CompressorRatio::Ratio8_0 => "8:1",
            _ => "Ratio",
        }
    }

    fn accent_button(label: impl Into<String>) -> egui::Button<'static> {
        egui::Button::new(
            egui::RichText::new(label.into())
                .monospace()
                .color(Self::accent())
                .strong(),
        )
        .fill(egui::Color32::from_rgb(58, 36, 22))
        .stroke(egui::Stroke::new(1.0, Self::accent()))
        .min_size(egui::vec2(96.0, 34.0))
    }

    fn danger_button(label: impl Into<String>) -> egui::Button<'static> {
        egui::Button::new(
            egui::RichText::new(label.into())
                .monospace()
                .color(egui::Color32::from_rgb(245, 152, 130))
                .strong(),
        )
        .fill(egui::Color32::from_rgb(70, 36, 32))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 75, 70)))
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
        ui.horizontal_wrapped(|ui| {
            ui.heading(
                egui::RichText::new("GoXLR Personal Control")
                    .monospace()
                    .color(egui::Color32::WHITE),
            );
            ui.add_space(12.0);
            let toggle_label = match self.quick_actions.view_mode() {
                AppViewMode::Mic
                | AppViewMode::Effects
                | AppViewMode::Lighting
                | AppViewMode::HeadphoneEq
                | AppViewMode::Sampler
                | AppViewMode::System
                | AppViewMode::Diagnostics
                | AppViewMode::About
                | AppViewMode::Full => "Mixer dashboard",
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
        Self::bounded_sized_panel(
            ui,
            MixerLayoutPolicy::panel_width(),
            MixerLayoutPolicy::top_row_height(),
            |ui| {
                ui.add_space(28.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("Profiles & Scenes")
                            .monospace()
                            .color(egui::Color32::WHITE)
                            .size(16.0)
                            .strong(),
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
                    ui.add_space(18.0);
                    ui.label(
                        egui::RichText::new("Quick scenes")
                            .monospace()
                            .color(Self::muted_text()),
                    );
                    let scene_buttons = QuickActions::scene_buttons(&self.scene_config.scenes());
                    for row in scene_buttons.chunks(2) {
                        let button_width = 150.0;
                        let gap = 10.0;
                        let row_width = row.len() as f32 * button_width
                            + row.len().saturating_sub(1) as f32 * gap;
                        ui.horizontal(|ui| {
                            ui.add_space(
                                ((MixerLayoutPolicy::panel_width() - row_width) / 2.0).max(0.0),
                            );
                            ui.spacing_mut().item_spacing.x = gap;
                            for scene in row {
                                let is_safe = scene.name().eq_ignore_ascii_case("safe now");
                                let button_size = egui::vec2(button_width, 30.0);
                                let response = if is_safe {
                                    ui.add_sized(
                                        button_size,
                                        Self::danger_button(scene.name().to_string()),
                                    )
                                } else {
                                    ui.add_sized(
                                        button_size,
                                        Self::accent_button(scene.name().to_string()),
                                    )
                                };
                                if response.clicked() {
                                    self.send(UiCommand::ApplyScene(scene.clone()));
                                }
                            }
                        });
                        ui.add_space(10.0);
                    }
                    ui.add_space(18.0);
                    ui.label(
                        egui::RichText::new("Personal presets")
                            .monospace()
                            .color(Self::muted_text()),
                    );
                    let preset_buttons =
                        QuickActions::personal_preset_buttons(&PersonalPreset::daily_presets());
                    for row in preset_buttons.chunks(2) {
                        let button_width = 150.0;
                        let gap = 10.0;
                        let row_width = row.len() as f32 * button_width
                            + row.len().saturating_sub(1) as f32 * gap;
                        ui.horizontal(|ui| {
                            ui.add_space(
                                ((MixerLayoutPolicy::panel_width() - row_width) / 2.0).max(0.0),
                            );
                            ui.spacing_mut().item_spacing.x = gap;
                            for preset in row {
                                let button_size = egui::vec2(button_width, 30.0);
                                let response = if preset.is_safety_preset() {
                                    ui.add_sized(
                                        button_size,
                                        Self::danger_button(preset.name().to_string()),
                                    )
                                } else {
                                    ui.add_sized(
                                        button_size,
                                        Self::accent_button(preset.name().to_string()),
                                    )
                                }
                                .on_hover_text(preset.description());
                                if response.clicked() {
                                    self.send(UiCommand::ApplyScene(preset.to_scene()));
                                }
                            }
                        });
                        ui.add_space(10.0);
                    }
                    if let Some(error) = self.scene_config.reload_error() {
                        ui.add_space(10.0);
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            format!("Scene reload issue: {error}"),
                        );
                    }
                });
            },
        );
    }

    fn render_status_card(&mut self, ui: &mut egui::Ui) {
        Self::bounded_sized_panel(
            ui,
            MixerLayoutPolicy::panel_width(),
            MixerLayoutPolicy::status_row_height(),
            |ui| {
                ui.add_space(56.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("GoXLR")
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
                ui.vertical_centered(|ui| {
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
                });
                ui.add_space(20.0);
                Self::centered_fixed_row(ui, 330.0, 18.0, |ui| {
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
            },
        );
    }

    fn render_active_streams_panel(&mut self, ui: &mut egui::Ui) {
        Self::bounded_sized_panel(
            ui,
            MixerLayoutPolicy::panel_width(),
            MixerLayoutPolicy::detail_panel_height(),
            |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(DashboardCopy::active_playback_heading())
                            .monospace()
                            .color(egui::Color32::WHITE)
                            .size(16.0)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(self.snapshot.active_audio_streams.summary())
                            .monospace()
                            .color(Self::muted_text()),
                    );
                    if let Some(error) = &self.snapshot.active_audio_error {
                        ui.colored_label(egui::Color32::YELLOW, format!("pactl: {error}"));
                    }
                });
                ui.separator();
                if self.snapshot.active_audio_streams.streams.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("Start audio in an app to see its route here.")
                                .monospace()
                                .color(Self::muted_text()),
                        );
                    });
                }
                let route_targets = self.snapshot.active_audio_streams.route_targets.clone();
                for stream in self.snapshot.active_audio_streams.streams.clone() {
                    Self::soft_panel_frame().show(ui, |ui| {
                        ui.vertical_centered(|ui| {
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
                        });
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
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(flags.join(" • "))
                                        .monospace()
                                        .color(Self::muted_text()),
                                );
                            });
                        }
                        ui.horizontal_centered(|ui| {
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
                            ui.horizontal_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(DashboardCopy::manual_route_label())
                                        .monospace()
                                        .color(Self::muted_text()),
                                );
                                for target in &route_targets {
                                    let already_on_target =
                                        stream.sink_label.contains(&target.label);
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
                            ui.horizontal_centered(|ui| {
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
            },
        );
    }

    fn render_fader_assignment_panel(&mut self, ui: &mut egui::Ui) {
        Self::bounded_sized_panel(
            ui,
            MixerLayoutPolicy::assignment_panel_width(),
            MixerLayoutPolicy::detail_row_height(),
            |ui| {
                ui.set_width(MixerLayoutPolicy::assignment_panel_width());
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("Fader assignment")
                            .monospace()
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.label("Safe first-pass hardware fader channel and mute-target controls.");
                });
                ui.add_space(12.0);
                Self::centered_exact_width(
                    ui,
                    MixerLayoutPolicy::assignment_card_width()
                        * MixerLayoutPolicy::assignment_cards_per_row() as f32
                        + MixerLayoutPolicy::assignment_card_gap()
                            * (MixerLayoutPolicy::assignment_cards_per_row() - 1) as f32,
                    |ui| {
                        egui::Grid::new("mixer_fader_assignment_cards")
                    .num_columns(MixerLayoutPolicy::assignment_cards_per_row())
                    .spacing(egui::vec2(
                        MixerLayoutPolicy::assignment_card_gap(),
                        MixerLayoutPolicy::assignment_card_gap(),
                    ))
                    .show(ui, |ui| {
                        for (index, assignment) in FaderAssignmentControl::daily_controls()
                            .iter()
                            .enumerate()
                        {
                            Self::soft_sized_panel(
                                ui,
                                MixerLayoutPolicy::assignment_card_width(),
                                MixerLayoutPolicy::assignment_card_height(),
                                |ui| {
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            egui::RichText::new(assignment.label())
                                                .monospace()
                                                .color(Self::muted_text()),
                                        );
                                        ui.label(assignment.description());
                                    });
                                    ui.vertical_centered(|ui| {
                                        egui::Grid::new(format!("{}_channel_grid", assignment.label()))
                                            .num_columns(4)
                                            .spacing(egui::vec2(6.0, 6.0))
                                            .show(ui, |ui| {
                                                for (index, channel) in FaderAssignmentControl::daily_channels()
                                                    .into_iter()
                                                    .enumerate()
                                                {
                                                    if ui
                                                        .add_sized(
                                                            egui::vec2(62.0, 22.0),
                                                            egui::Button::new(channel.to_string()).small(),
                                                        )
                                                        .on_hover_text(format!(
                                                            "Assign {} to {}",
                                                            channel,
                                                            assignment.label()
                                                        ))
                                                        .clicked()
                                                    {
                                                        self.send(UiCommand::Send(
                                                            assignment.assign_command(channel),
                                                        ));
                                                    }
                                                    if (index + 1) % 4 == 0 {
                                                        ui.end_row();
                                                    }
                                                }
                                            });
                                    });
                                    ui.separator();
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            egui::RichText::new("Mute")
                                                .monospace()
                                                .color(Self::muted_text()),
                                        );
                                    });
                                    if let Some(control) = FaderMuteFunctionControl::daily_controls()
                                        .into_iter()
                                        .find(|control| control.fader() == assignment.fader())
                                    {
                                        ui.vertical_centered(|ui| {
                                            egui::Grid::new(format!("{}_mute_function_grid", control.label()))
                                                .num_columns(2)
                                                .spacing(egui::vec2(8.0, 6.0))
                                                .show(ui, |ui| {
                                                    for (index, function) in FaderMuteFunctionControl::daily_functions()
                                                        .into_iter()
                                                        .enumerate()
                                                    {
                                                        if ui
                                                            .add_sized(
                                                                egui::vec2(118.0, 22.0),
                                                                egui::Button::new(function.to_string()).small(),
                                                            )
                                                            .on_hover_text(format!(
                                                                "Set {} mute behaviour to {}",
                                                                control.label(),
                                                                function
                                                            ))
                                                            .clicked()
                                                        {
                                                            self.send(UiCommand::Send(
                                                                control.function_command(function),
                                                            ));
                                                        }
                                                        if (index + 1) % 2 == 0 {
                                                            ui.end_row();
                                                        }
                                                    }
                                                });
                                        });
                                    }
                                    ui.separator();
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            egui::RichText::new("Current state")
                                                .monospace()
                                                .color(Self::muted_text()),
                                        );
                                    });
                                    if let Some(control) = FaderMuteStateControl::daily_controls()
                                        .into_iter()
                                        .find(|control| control.fader() == assignment.fader())
                                    {
                                        ui.vertical_centered(|ui| {
                                            egui::Grid::new(format!("{}_mute_state_grid", control.label()))
                                                .num_columns(3)
                                                .spacing(egui::vec2(6.0, 6.0))
                                                .show(ui, |ui| {
                                                    for state in FaderMuteStateControl::daily_states() {
                                                        if ui
                                                            .add_sized(
                                                                egui::vec2(82.0, 22.0),
                                                                egui::Button::new(
                                                                    FaderMuteStateControl::state_label(state),
                                                                )
                                                                .small(),
                                                            )
                                                            .on_hover_text(format!(
                                                                "Set {} to {}",
                                                                control.label(),
                                                                state
                                                            ))
                                                            .clicked()
                                                        {
                                                            self.send(UiCommand::Send(
                                                                control.state_command(state),
                                                            ));
                                                        }
                                                    }
                                                    ui.end_row();
                                                });
                                        });
                                    }
                                },
                            );
                            if (index + 1) % MixerLayoutPolicy::assignment_cards_per_row() == 0 {
                                ui.end_row();
                            }
                        }
                            });
                    },
                );
            },
        );
    }

    fn render_monitor_mix_panel(&mut self, ui: &mut egui::Ui) {
        Self::bounded_sized_panel(
            ui,
            MixerLayoutPolicy::monitor_mix_panel_width(),
            MixerLayoutPolicy::status_row_height(),
            |ui| {
                ui.set_width(MixerLayoutPolicy::monitor_mix_panel_width());
                ui.add_space(68.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("Monitor mix")
                            .strong()
                            .color(Self::accent()),
                    );
                    ui.label(
                        "Choose which output mix is monitored in headphones. This is the hardware monitor selector, not the monitor-with-FX toggle.",
                    );
                    ui.add_space(14.0);

                    Self::centered_exact_width(
                        ui,
                        MixerLayoutPolicy::monitor_mix_button_width() * 2.0 + 12.0,
                        |ui| {
                            egui::Grid::new("monitor_mix_button_grid")
                                .num_columns(2)
                                .min_col_width(MixerLayoutPolicy::monitor_mix_button_width())
                                .spacing(egui::vec2(12.0, 12.0))
                                .show(ui, |ui| {
                            for (index, control) in MonitorMixControl::daily_controls()
                                .into_iter()
                                .enumerate()
                            {
                                if ui
                                    .add_sized(
                                        egui::vec2(MixerLayoutPolicy::monitor_mix_button_width(), 28.0),
                                        Self::accent_button(control.label()),
                                    )
                                    .on_hover_text(control.description())
                                    .clicked()
                                {
                                    self.send(UiCommand::Send(control.command()));
                                }
                                if (index + 1) % 2 == 0 {
                                    ui.end_row();
                                }
                            }
                                });
                        },
                    );
                });
            },
        );
    }

    fn render_submix_panel(&mut self, ui: &mut egui::Ui) {
        Self::bounded_panel(ui, MixerLayoutPolicy::submix_panel_width(), |ui| {
            ui.set_width(MixerLayoutPolicy::submix_panel_width());
            ui.label(egui::RichText::new("Submix").strong().color(Self::accent()));
            ui.label(
                "First-pass submix controls: enable/disable submix, set safe channel volume presets, link channels, and choose whether each output follows mix A or B.",
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_sized(
                        egui::vec2(MixerLayoutPolicy::submix_button_width(), 24.0),
                        Self::accent_button("Enable"),
                    )
                    .on_hover_text("Enable GoXLR submix routing")
                    .clicked()
                {
                    self.send(UiCommand::Send(PersonalCommand::SetSubMixEnabled(true)));
                }
                if ui
                    .add_sized(
                        egui::vec2(MixerLayoutPolicy::submix_button_width(), 24.0),
                        egui::Button::new("Disable").small(),
                    )
                    .on_hover_text("Disable GoXLR submix routing")
                    .clicked()
                {
                    self.send(UiCommand::Send(PersonalCommand::SetSubMixEnabled(false)));
                }
            });
            ui.separator();
            ui.label(
                egui::RichText::new("Channel presets")
                    .monospace()
                    .color(Self::muted_text()),
            );
            for control in SubmixChannelControl::daily_controls() {
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(control.label());
                        if let Some(state) = self.snapshot.submix_channel_state(control.channel()) {
                            ui.label(egui::RichText::new(state).small().color(Self::muted_text()));
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        for volume in control.volume_presets() {
                            if ui
                                .add_sized(
                                    egui::vec2(MixerLayoutPolicy::submix_button_width(), 22.0),
                                    egui::Button::new(format!("{}%", volume)).small(),
                                )
                                .on_hover_text(format!(
                                    "Set {} submix volume to {}%",
                                    control.label(),
                                    volume
                                ))
                                .clicked()
                            {
                                self.send(UiCommand::Send(control.volume_command(volume)));
                            }
                        }
                        for (label, linked) in [("Link", true), ("Unlink", false)] {
                            if ui
                                .add_sized(
                                    egui::vec2(MixerLayoutPolicy::submix_button_width(), 22.0),
                                    egui::Button::new(label).small(),
                                )
                                .on_hover_text(format!(
                                    "{} {} with the main mix",
                                    label,
                                    control.label()
                                ))
                                .clicked()
                            {
                                self.send(UiCommand::Send(control.link_command(linked)));
                            }
                        }
                    });
                    ui.add_space(4.0);
                });
            }
            ui.separator();
            ui.label(
                egui::RichText::new("Custom channel volumes")
                    .monospace()
                    .color(Self::muted_text()),
            );
            for slider in SubmixVolumeSlider::daily_sliders() {
                let snapshot = self
                    .snapshot
                    .submix_channels
                    .iter()
                    .find(|snapshot| snapshot.channel() == slider.channel());
                let mut volume_percent = slider.value_from_snapshot(snapshot);
                ui.horizontal_wrapped(|ui| {
                    ui.label(slider.label());
                    if ui
                        .add_sized(
                            egui::vec2(MixerLayoutPolicy::submix_slider_width(), 22.0),
                            egui::Slider::new(&mut volume_percent, slider.range())
                                .suffix("%")
                                .clamping(egui::SliderClamping::Always),
                        )
                        .on_hover_text(format!("Set {} custom submix volume", slider.label()))
                        .changed()
                    {
                        self.send(UiCommand::Send(
                            slider.command_for_percent(volume_percent as u16),
                        ));
                    }
                });
            }
            ui.separator();
            ui.label(
                egui::RichText::new("Output mix")
                    .monospace()
                    .color(Self::muted_text()),
            );
            for output in SubmixOutputMixControl::daily_controls() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(output.label());
                    if let Some(state) = self.snapshot.submix_output_state(output.output()) {
                        ui.label(egui::RichText::new(state).small().color(Self::muted_text()));
                    }
                    for mix in output.mixes() {
                        if ui
                            .add_sized(
                                egui::vec2(MixerLayoutPolicy::submix_button_width(), 22.0),
                                egui::Button::new(format!("Mix {}", mix)).small(),
                            )
                            .on_hover_text(format!("Route {} to submix {}", output.label(), mix))
                            .clicked()
                        {
                            self.send(UiCommand::Send(output.mix_command(mix)));
                        }
                    }
                });
            }
        });
    }

    fn render_scribble_strip_panel(&mut self, ui: &mut egui::Ui) {
        Self::bounded_panel(ui, MixerLayoutPolicy::scribble_panel_width(), |ui| {
            ui.set_width(MixerLayoutPolicy::scribble_panel_width());
            ui.label(
                egui::RichText::new("Scribble strips")
                    .monospace()
                    .size(18.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.label(
                "First-pass hardware scribble-strip labels, numbers, icons, and invert toggles.",
            );
            ui.add_space(8.0);
            for control in HardwareScribbleControl::daily_controls() {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(control.label())
                            .monospace()
                            .color(Self::muted_text()),
                    );
                    ui.label(control.description());
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_sized(
                                egui::vec2(MixerLayoutPolicy::scribble_button_width(), 24.0),
                                egui::Button::new(format!("Text: {}", control.default_text()))
                                    .small(),
                            )
                            .clicked()
                        {
                            self.send(UiCommand::Send(
                                control.text_command(control.default_text()),
                            ));
                        }
                        if ui
                            .add_sized(
                                egui::vec2(MixerLayoutPolicy::scribble_button_width(), 24.0),
                                egui::Button::new(format!("No. {}", control.default_number()))
                                    .small(),
                            )
                            .clicked()
                        {
                            self.send(UiCommand::Send(
                                control.number_command(control.default_number()),
                            ));
                        }
                        for icon in HardwareScribbleControl::daily_icon_presets() {
                            let label = icon.unwrap_or("no icon");
                            if ui
                                .add_sized(
                                    egui::vec2(MixerLayoutPolicy::scribble_button_width(), 24.0),
                                    egui::Button::new(format!("Icon: {label}")).small(),
                                )
                                .clicked()
                            {
                                self.send(UiCommand::Send(control.icon_command(icon)));
                            }
                        }
                        for (label, inverted) in [("Invert on", true), ("Invert off", false)] {
                            if ui
                                .add_sized(
                                    egui::vec2(MixerLayoutPolicy::scribble_button_width(), 24.0),
                                    egui::Button::new(label).small(),
                                )
                                .clicked()
                            {
                                self.send(UiCommand::Send(control.invert_command(inverted)));
                            }
                        }
                    });
                    ui.add_space(6.0);
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
            ui.set_min_width(MixerLayoutPolicy::channel_strip_width());
            ui.set_max_width(MixerLayoutPolicy::channel_strip_width());
            ui.set_min_height(MixerLayoutPolicy::channel_strip_height());
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(label)
                        .monospace()
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(6.0);
                let changed = ui
                    .add_sized(
                        egui::vec2(46.0, MixerLayoutPolicy::channel_slider_height()),
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

    fn render_lighting_profile_panel(&mut self, ui: &mut egui::Ui) {
        Self::bounded_panel(ui, LightingLayoutPolicy::profile_panel_width(), |ui| {
            ui.set_width(LightingLayoutPolicy::profile_panel_width());
            ui.vertical(|ui| {
                ui.set_width(LightingLayoutPolicy::profile_panel_width());
                ui.label(
                    egui::RichText::new("Lighting profile")
                        .monospace()
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.label("Load only colours from your personal profile; audio, routing, mic, and effects stay untouched.");
                ui.add_space(6.0);
                for action in LightingProfileAction::guarded_daily_actions("Personal") {
                    let confirmed = self
                        .pending_confirmation(PendingConfirmationKind::LightingProfile)
                        .is_some_and(|pending| pending == &action.command());
                    let label = if confirmed {
                        format!("Confirm {}", action.label())
                    } else {
                        action.label().to_string()
                    };
                    let button = ui.add_sized(
                        [LightingLayoutPolicy::profile_button_width(), 28.0],
                        Self::accent_button(label),
                    );
                    if button.on_hover_text(action.description()).clicked() {
                        if let Some(command) = action.command_if_confirmed(confirmed) {
                            self.set_pending_confirmation(
                                PendingConfirmationKind::LightingProfile,
                                None,
                            );
                            self.send(UiCommand::Send(command));
                        } else {
                            self.set_pending_confirmation(
                                PendingConfirmationKind::LightingProfile,
                                Some(action.command()),
                            );
                        }
                    }
                }
            });
            if self.has_pending_confirmation(PendingConfirmationKind::LightingProfile) {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("Click the same lighting action again to confirm.")
                        .color(egui::Color32::from_rgb(255, 210, 120)),
                );
            }
        });
    }

    fn render_lighting_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        Self::section_header(
            ui,
            "Lighting",
            "Quick themes plus detailed colour editor controls",
            "Native lighting parity: themes, animation controls, simple colours, faders, button groups, encoders, and sampler select colours.",
        );
        ui.add_space(12.0);

        ui.allocate_ui_with_layout(
            egui::vec2(LightingLayoutPolicy::editor_intro_width(), 20.0),
            egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
            |ui| {
                ui.label(
                    "Start with a whole-device theme, then fine-tune individual lighting groups below.",
                );
            },
        );
        ui.add_space(6.0);
        self.render_lighting_profile_panel(ui);
        ui.add_space(8.0);
        self.render_profile_browser_panel(
            ui,
            self.profile_browser_for(ProfileBrowserKind::LightingColours),
        );
        ui.add_space(8.0);
        let quick_theme_card_width =
            LightingLayoutPolicy::quick_theme_card_width_for_available_width(ui.available_width());
        Self::polished_row(
            ui,
            egui::vec2(
                LightingLayoutPolicy::panel_gap(),
                LightingLayoutPolicy::panel_gap(),
            ),
            |ui| {
                for theme in LightingQuickTheme::daily_themes() {
                    Self::bounded_panel(ui, quick_theme_card_width, |ui| {
                        ui.set_width(quick_theme_card_width);
                        ui.set_min_height(LightingLayoutPolicy::quick_theme_card_height());
                        ui.vertical(|ui| {
                            ui.set_width(quick_theme_card_width);
                            if ui.add(Self::accent_button(theme.name())).clicked() {
                                self.send(UiCommand::ApplyScene(UiScene::new(
                                    theme.name(),
                                    theme.commands(),
                                )));
                            }
                            ui.add_space(4.0);
                            ui.label(theme.description());
                        });
                    });
                }
            },
        );

        ui.add_space(14.0);
        ui.separator();
        ui.add_space(8.0);
        ui.label("Detailed editor — panels wrap into the available window width instead of hiding everything to the right.");
        ui.add_space(8.0);
        Self::polished_row(
            ui,
            egui::vec2(
                LightingLayoutPolicy::panel_gap(),
                LightingLayoutPolicy::panel_gap(),
            ),
            |ui| {
                Self::bounded_panel(
                    ui,
                    LightingLayoutPolicy::compact_editor_panel_width(),
                    |ui| {
                        ui.set_width(LightingLayoutPolicy::compact_editor_panel_width());
                        ui.vertical(|ui| {
                            ui.set_width(LightingLayoutPolicy::compact_editor_panel_width());
                            ui.label(
                                egui::RichText::new("Animation")
                                    .monospace()
                                    .size(18.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            );
                            ui.label("Mode, modifiers, and waterfall direction.");
                            ui.add_space(6.0);
                            egui::Grid::new("lighting_animation_controls")
                                .num_columns(LightingLayoutPolicy::animation_control_grid_columns())
                                .spacing(egui::vec2(8.0, 6.0))
                                .striped(false)
                                .show(ui, |ui| {
                                    ui.label("Mode");
                                    ui.horizontal_wrapped(|ui| {
                                        for (label, value) in [
                                            ("Simple", 0),
                                            ("Rainbow", 1),
                                            ("Ripple", 2),
                                            ("Retro", 3),
                                            ("None", 4),
                                        ] {
                                            if ui.button(label).clicked() {
                                                self.send(UiCommand::Send(
                                                    LightingAnimationControl::Mode
                                                        .command_for_value(value),
                                                ));
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Mod 1");
                                    ui.horizontal(|ui| {
                                        for value in [0, 50, 100] {
                                            if ui.button(value.to_string()).clicked() {
                                                self.send(UiCommand::Send(
                                                    LightingAnimationControl::Mod1
                                                        .command_for_value(value),
                                                ));
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Mod 2");
                                    ui.horizontal(|ui| {
                                        for value in [0, 50, 100] {
                                            if ui.button(value.to_string()).clicked() {
                                                self.send(UiCommand::Send(
                                                    LightingAnimationControl::Mod2
                                                        .command_for_value(value),
                                                ));
                                            }
                                        }
                                    });
                                    ui.end_row();

                                    ui.label("Waterfall");
                                    ui.horizontal(|ui| {
                                        if ui.button("Down").clicked() {
                                            self.send(UiCommand::Send(
                                                LightingAnimationControl::Waterfall
                                                    .command_for_value(0),
                                            ));
                                        }
                                        if ui.button("Up").clicked() {
                                            self.send(UiCommand::Send(
                                                LightingAnimationControl::Waterfall
                                                    .command_for_value(1),
                                            ));
                                        }
                                    });
                                    ui.end_row();
                                });
                        });
                    },
                );

                Self::bounded_panel(
                    ui,
                    LightingLayoutPolicy::compact_editor_panel_width(),
                    |ui| {
                        ui.set_width(LightingLayoutPolicy::compact_editor_panel_width());
                        ui.vertical(|ui| {
                            ui.set_width(LightingLayoutPolicy::compact_editor_panel_width());
                            ui.label(
                                egui::RichText::new("Simple colours")
                                    .monospace()
                                    .size(18.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            );
                            ui.label("Global, accent, and scribble-strip colours.");
                            ui.add_space(6.0);
                            for target in LightingSimpleColourTarget::all_targets() {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(target.label());
                                    for (label, colour) in [
                                        ("White", "FFFFFF"),
                                        ("Blue", "1F6FFF"),
                                        ("Red", "FF3030"),
                                        ("Off", "000000"),
                                    ] {
                                        if ui.button(label).clicked() {
                                            self.send(UiCommand::Send(
                                                target.command_for_colour(colour),
                                            ));
                                        }
                                    }
                                });
                            }
                        });
                    },
                );
            },
        );

        ui.add_space(12.0);
        Self::polished_row(
            ui,
            egui::vec2(
                LightingLayoutPolicy::panel_gap(),
                LightingLayoutPolicy::panel_gap(),
            ),
            |ui| {
                Self::bounded_panel(ui, LightingLayoutPolicy::wide_editor_panel_width(), |ui| {
                    ui.set_width(LightingLayoutPolicy::wide_editor_panel_width());
                    ui.vertical(|ui| {
                        ui.set_width(LightingLayoutPolicy::wide_editor_panel_width());
                        ui.label(
                            egui::RichText::new("Faders")
                                .monospace()
                                .size(18.0)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );
                        ui.label(
                            "Set all faders or individual fader A-D colours and display style.",
                        );
                        ui.add_space(6.0);
                        for target in LightingFaderColourTarget::all_targets() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(target.label());
                                for (label, top, bottom) in [
                                    ("Cool", "2E8BFF", "002040"),
                                    ("Warm", "FF3030", "400000"),
                                    ("Dim", "606060", "202020"),
                                    ("Off", "000000", "000000"),
                                ] {
                                    if ui.button(label).clicked() {
                                        self.send(UiCommand::Send(
                                            target.colour_command(top, bottom),
                                        ));
                                    }
                                }
                                for style in [
                                    FaderDisplayStyle::TwoColour,
                                    FaderDisplayStyle::Gradient,
                                    FaderDisplayStyle::Meter,
                                    FaderDisplayStyle::GradientMeter,
                                ] {
                                    if ui
                                        .add_sized(
                                            egui::vec2(
                                                ContentLayoutPolicy::min_action_button_width(),
                                                22.0,
                                            ),
                                            egui::Button::new(format!("{:?}", style)).small(),
                                        )
                                        .clicked()
                                    {
                                        self.send(UiCommand::Send(target.display_command(style)));
                                    }
                                }
                            });
                        }
                    });
                });

                Self::bounded_panel(ui, LightingLayoutPolicy::wide_editor_panel_width(), |ui| {
                    ui.set_width(LightingLayoutPolicy::wide_editor_panel_width());
                    ui.vertical(|ui| {
                        ui.set_width(LightingLayoutPolicy::wide_editor_panel_width());
                        ui.label(
                            egui::RichText::new("Buttons")
                                .monospace()
                                .size(18.0)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );
                        ui.label(
                        "Button groups plus daily-use Cough/Bleep button colours and off styles.",
                    );
                        ui.add_space(6.0);
                        for target in LightingButtonColourTarget::daily_targets() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(target.label());
                                for (label, one, two) in [
                                    ("Blue", "00A8FF", "001A33"),
                                    ("Red", "FF3030", "400000"),
                                    ("Dim", "404040", "101010"),
                                    ("Off", "000000", "000000"),
                                ] {
                                    if ui.button(label).clicked() {
                                        self.send(UiCommand::Send(target.colour_command(one, two)));
                                    }
                                }
                                for style in [
                                    ButtonColourOffStyle::Dimmed,
                                    ButtonColourOffStyle::Colour2,
                                    ButtonColourOffStyle::DimmedColour2,
                                ] {
                                    if ui
                                        .add_sized(
                                            egui::vec2(
                                                ContentLayoutPolicy::min_action_button_width(),
                                                22.0,
                                            ),
                                            egui::Button::new(format!("{:?}", style)).small(),
                                        )
                                        .clicked()
                                    {
                                        self.send(UiCommand::Send(target.off_style_command(style)));
                                    }
                                }
                            });
                        }
                    });
                });

                Self::bounded_panel(ui, LightingLayoutPolicy::wide_editor_panel_width(), |ui| {
                    ui.set_width(LightingLayoutPolicy::wide_editor_panel_width());
                    ui.vertical(|ui| {
                        ui.set_width(LightingLayoutPolicy::wide_editor_panel_width());
                        ui.label(
                            egui::RichText::new("Encoders & sampler")
                                .monospace()
                                .size(18.0)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );
                        ui.label("Three-colour encoder rings and sampler select buttons.");
                        ui.add_space(6.0);
                        for target in LightingTripleColourTarget::all_targets() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(target.label());
                                for (label, one, two, three) in [
                                    ("Cool", "00A8FF", "1F6FFF", "80DFFF"),
                                    ("Warm", "FF3030", "FF8080", "400000"),
                                    ("White", "FFFFFF", "808080", "202020"),
                                    ("Off", "000000", "000000", "000000"),
                                ] {
                                    if ui.button(label).clicked() {
                                        self.send(UiCommand::Send(
                                            target.colour_command(one, two, three),
                                        ));
                                    }
                                }
                                if let Some(command) =
                                    target.off_style_command(ButtonColourOffStyle::DimmedColour2)
                                    && ui
                                        .add_sized(
                                            egui::vec2(
                                                ContentLayoutPolicy::wide_action_button_width(),
                                                22.0,
                                            ),
                                            egui::Button::new("Sampler off: DimmedColour2").small(),
                                        )
                                        .clicked()
                                {
                                    self.send(UiCommand::Send(command));
                                }
                            });
                        }
                    });
                });
            },
        );
    }

    fn render_profile_browser_panel(&mut self, ui: &mut egui::Ui, browser: ProfileBrowser) {
        let panel_width = 640.0;
        let inner_width = panel_width - 28.0;
        Self::bounded_panel(ui, panel_width, |ui| {
            ui.set_width(inner_width);
            ui.label(
                egui::RichText::new(browser.title())
                    .monospace()
                    .size(18.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(
                    "Browse discovered profile files and arm stateful load/save/delete commands per row.",
                )
                .color(Self::muted_text()),
            );
            ui.add_space(8.0);

            if browser.rows().is_empty() {
                ui.label(egui::RichText::new(browser.empty_hint()).color(Self::muted_text()));
                return;
            }

            for row in browser.rows() {
                Self::soft_panel_frame().show(ui, |ui| {
                    ui.set_min_width(inner_width);
                    ui.set_max_width(inner_width);
                    ui.horizontal_wrapped(|ui| {
                        let status = if row.is_active() {
                            "active"
                        } else {
                            "available"
                        };
                        ui.label(
                            egui::RichText::new(row.name())
                                .monospace()
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new(status)
                                .small()
                                .color(Self::muted_text()),
                        );
                    });
                    ui.horizontal_wrapped(|ui| {
                        for action in row.actions() {
                            let command = action.command();
                            let confirmed = self
                                .pending_profile_confirmation(browser.kind())
                                .is_some_and(|pending| pending == &command);
                            let label = if action.requires_confirmation() && confirmed {
                                format!("Confirm {}", action.label())
                            } else if action.requires_confirmation() {
                                format!("Arm {}", action.label())
                            } else {
                                action.label().to_string()
                            };
                            if ui
                                .add_sized(
                                    egui::vec2(112.0, 24.0),
                                    egui::Button::new(label).small(),
                                )
                                .clicked()
                            {
                                self.handle_profile_browser_action(browser.kind(), &action);
                            }
                        }
                    });
                });
                ui.add_space(4.0);
            }

            if self.pending_profile_confirmation(browser.kind()).is_some() {
                ui.label(
                    egui::RichText::new("Click the same armed row action again to send it.")
                        .small()
                        .color(Self::muted_text()),
                );
            }
        });
    }

    fn render_main_profile_panel(&mut self, ui: &mut egui::Ui) {
        Self::bounded_panel(ui, SystemLayoutPolicy::profile_panel_width(), |ui| {
            ui.set_width(SystemLayoutPolicy::profile_panel_width());
            ui.label(
                egui::RichText::new("Main profile")
                    .monospace()
                    .size(18.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.label("Guarded full-profile load, save, create, and delete actions for one named personal slot.");
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for action in MainProfileAction::guarded_daily_actions("Personal") {
                    let confirmed = self
                        .pending_confirmation(PendingConfirmationKind::MainProfile)
                        .is_some_and(|pending| pending == &action.command());
                    let label = if confirmed {
                        format!("Confirm {}", action.label())
                    } else {
                        action.label().to_string()
                    };
                    if ui
                        .add_sized(
                            egui::vec2(SystemLayoutPolicy::profile_button_width(), 26.0),
                            egui::Button::new(label).small(),
                        )
                        .clicked()
                    {
                        if let Some(command) = action.command_if_confirmed(confirmed) {
                            self.set_pending_confirmation(
                                PendingConfirmationKind::MainProfile,
                                None,
                            );
                            self.send(UiCommand::Send(command));
                        } else {
                            self.set_pending_confirmation(
                                PendingConfirmationKind::MainProfile,
                                Some(action.command()),
                            );
                        }
                    }
                }
            });
            if self.has_pending_confirmation(PendingConfirmationKind::MainProfile) {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("Click the same main profile action again to confirm.")
                        .small()
                        .color(Self::muted_text()),
                );
            }
        });
    }

    fn render_diagnostics_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        Self::section_header(
            ui,
            "Diagnostics / Status",
            "Read-only connection, daemon, device, profile, and IPC details",
            "Use this page before troubleshooting so socket paths, selected device state, and profile status are visible without touching destructive controls.",
        );
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            Self::bounded_panel(ui, DiagnosticsLayoutPolicy::panel_width(), |ui| {
                ui.label(
                    egui::RichText::new("Live status")
                        .monospace()
                        .color(egui::Color32::WHITE)
                        .size(16.0),
                );
                ui.separator();
                for row in self.snapshot.diagnostics_rows() {
                    Self::soft_panel_frame().show(ui, |ui| {
                        ui.set_min_width(DiagnosticsLayoutPolicy::panel_width() - 24.0);
                        ui.set_max_width(DiagnosticsLayoutPolicy::panel_width() - 24.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(row.label())
                                    .monospace()
                                    .color(Self::muted_text()),
                            );
                            ui.label(
                                egui::RichText::new(row.severity().label())
                                    .monospace()
                                    .color(Self::diagnostics_severity_color(row.severity())),
                            );
                        });
                        ui.label(
                            egui::RichText::new(row.value())
                                .monospace()
                                .color(egui::Color32::WHITE),
                        );
                    });
                    ui.add_space(4.0);
                }
                ui.add_space(8.0);
                if ui
                    .add_sized(
                        [DiagnosticsLayoutPolicy::button_width(), 30.0],
                        Self::accent_button("Refresh status"),
                    )
                    .clicked()
                {
                    self.send(UiCommand::Refresh);
                }
            });

            Self::bounded_panel(ui, DiagnosticsLayoutPolicy::detail_panel_width(), |ui| {
                ui.label(
                    egui::RichText::new("IPC & socket candidates")
                        .monospace()
                        .color(egui::Color32::WHITE)
                        .size(16.0),
                );
                ui.separator();
                ui.label(
                    egui::RichText::new(
                        "The app tries these daemon socket paths in order; existing paths are highlighted.",
                    )
                    .monospace()
                    .color(Self::muted_text()),
                );
                ui.add_space(6.0);
                for candidate in ipc_socket_path_candidates() {
                    let exists = Path::new(&candidate).exists();
                    Self::soft_panel_frame().show(ui, |ui| {
                        ui.set_min_width(DiagnosticsLayoutPolicy::detail_panel_width() - 24.0);
                        ui.set_max_width(DiagnosticsLayoutPolicy::detail_panel_width() - 24.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(if exists { "Found" } else { "Missing" })
                                    .monospace()
                                    .color(if exists { Self::accent() } else { Self::muted_text() }),
                            );
                            ui.label(
                                egui::RichText::new(candidate)
                                    .monospace()
                                    .color(egui::Color32::WHITE),
                            );
                        });
                    });
                    ui.add_space(4.0);
                }
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(format!("Scene config: {}", self.scene_config.path().display()))
                        .monospace()
                        .color(Self::muted_text()),
                );
                if let Some(error) = self.scene_config.reload_error() {
                    ui.colored_label(egui::Color32::YELLOW, format!("Scene reload issue: {error}"));
                }
            });

            Self::bounded_panel(ui, DiagnosticsLayoutPolicy::log_panel_width(), |ui| {
                ui.label(
                    egui::RichText::new("Recent app & IPC log")
                        .monospace()
                        .color(egui::Color32::WHITE)
                        .size(16.0),
                );
                ui.separator();
                ui.label(
                    egui::RichText::new(
                        "Read-only in-app event trail for snapshots, IPC errors, and troubleshooting context. This does not tail daemon files yet.",
                    )
                    .monospace()
                    .color(Self::muted_text()),
                );
                ui.add_space(6.0);
                let rows = DiagnosticsLogEntry::recent_rows(
                    &self.diagnostics_log,
                    DiagnosticsLayoutPolicy::log_row_limit(),
                    DiagnosticsLogFilter::All,
                );
                if rows.is_empty() {
                    ui.label(
                        egui::RichText::new("No app events recorded yet")
                            .monospace()
                            .color(Self::muted_text()),
                    );
                }
                for row in rows {
                    Self::soft_panel_frame().show(ui, |ui| {
                        ui.set_min_width(DiagnosticsLayoutPolicy::log_panel_width() - 24.0);
                        ui.set_max_width(DiagnosticsLayoutPolicy::log_panel_width() - 24.0);
                        ui.set_min_height(DiagnosticsLayoutPolicy::log_row_height());
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(row.timestamp())
                                    .monospace()
                                    .color(Self::muted_text()),
                            );
                            ui.label(
                                egui::RichText::new(row.severity().label())
                                    .monospace()
                                    .color(Self::diagnostics_severity_color(row.severity())),
                            );
                            ui.label(
                                egui::RichText::new(row.category())
                                    .monospace()
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            );
                        });
                        ui.label(
                            egui::RichText::new(row.message())
                                .monospace()
                                .color(Self::muted_text()),
                        );
                    });
                    ui.add_space(4.0);
                }
            });
        });
    }

    fn render_about_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        Self::section_header(
            ui,
            "About / Implemented Parity",
            "Read-only summary of what this native personal UI currently covers",
            "Use this page as an in-app checklist: implemented means daily native parity is solid; partial means useful controls exist but a full web-style browser/editor is still intentionally deferred.",
        );
        ui.add_space(12.0);
        egui::Grid::new("about_implemented_parity_cards")
            .num_columns(2)
            .spacing(egui::vec2(
                ContentLayoutPolicy::desktop_panel_gap(),
                ContentLayoutPolicy::desktop_panel_gap(),
            ))
            .show(ui, |ui| {
                for (index, item) in ImplementedParityItem::current_items().iter().enumerate() {
                    Self::soft_sized_panel(
                        ui,
                        AboutLayoutPolicy::panel_width(),
                        AboutLayoutPolicy::panel_height(),
                        |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    egui::RichText::new(item.label())
                                        .monospace()
                                        .size(16.0)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                );
                                let status_color = match item.status() {
                                    ImplementedParityStatus::Implemented => Self::accent(),
                                    ImplementedParityStatus::Partial => egui::Color32::YELLOW,
                                };
                                ui.add_sized(
                                    egui::vec2(AboutLayoutPolicy::status_badge_width(), 20.0),
                                    egui::Label::new(
                                        egui::RichText::new(item.status_label())
                                            .monospace()
                                            .small()
                                            .color(status_color),
                                    ),
                                );
                            });
                            ui.separator();
                            ui.label(
                                egui::RichText::new(item.description())
                                    .monospace()
                                    .color(Self::muted_text()),
                            );
                        },
                    );
                    if index % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
    }

    fn render_system_live_status_panel(&self, ui: &mut egui::Ui) {
        Self::bounded_panel(ui, SystemLayoutPolicy::profile_panel_width(), |ui| {
            ui.label(
                egui::RichText::new("Live system settings")
                    .monospace()
                    .size(18.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.label("Read-only daemon snapshot for the settings controlled below.");
            ui.add_space(8.0);
            if let Some(settings) = &self.snapshot.system_settings {
                for row in settings.rows() {
                    Self::soft_panel_frame().show(ui, |ui| {
                        ui.set_min_width(SystemLayoutPolicy::profile_panel_width() - 24.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(row.label())
                                    .monospace()
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            );
                            ui.label(
                                egui::RichText::new(row.value())
                                    .monospace()
                                    .color(Self::accent()),
                            );
                        });
                        ui.label(
                            egui::RichText::new(row.description())
                                .monospace()
                                .small()
                                .color(Self::muted_text()),
                        );
                    });
                    ui.add_space(4.0);
                }
            } else {
                ui.label(
                    egui::RichText::new("No daemon settings snapshot received yet.")
                        .monospace()
                        .color(Self::muted_text()),
                );
            }
        });
    }

    fn render_system_page(&mut self, ui: &mut egui::Ui) {
        Self::section_header(
            ui,
            "System",
            "Daily device settings plus guarded profile workflows",
            "Quick controls for mute timing, monitoring, fader lock, VOD mode, reloading settings, and same-action-confirmed full-profile workflows.",
        );
        ui.add_space(12.0);
        self.render_main_profile_panel(ui);
        ui.add_space(12.0);
        self.render_profile_browser_panel(ui, self.profile_browser_for(ProfileBrowserKind::Main));
        ui.add_space(12.0);
        self.render_system_live_status_panel(ui);
        ui.add_space(12.0);

        ui.allocate_ui_with_layout(
            egui::vec2(Self::content_width(ui), 0.0),
            egui::Layout::left_to_right(egui::Align::Min)
                .with_main_wrap(true)
                .with_cross_align(egui::Align::Min),
            |ui| {
                for action in SystemSettingsAction::daily_controls() {
                    ui.allocate_ui_with_layout(
                        egui::vec2(SystemLayoutPolicy::panel_width(), 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            Self::soft_bounded_panel(ui, SystemLayoutPolicy::panel_width(), |ui| {
                                ui.heading(action.label());
                                ui.label(action.description());
                                ui.add_space(8.0);
                                if ui
                                    .add_sized(
                                        egui::vec2(SystemLayoutPolicy::button_width(), 28.0),
                                        Self::accent_button(action.label()),
                                    )
                                    .clicked()
                                {
                                    self.send(UiCommand::Send(action.command()));
                                }
                            });
                        },
                    );
                    ui.add_space(ContentLayoutPolicy::desktop_panel_gap());
                }
            },
        );
    }

    fn render_effect_preset_management_panel(&mut self, ui: &mut egui::Ui) {
        Self::bounded_panel(
            ui,
            EffectsLayoutPolicy::preset_management_panel_width(),
            |ui| {
                ui.set_width(EffectsLayoutPolicy::preset_management_panel_width());
                ui.label(
                    egui::RichText::new("Effect presets")
                        .monospace()
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.label(
                    "Guarded load, rename, and save actions for the active effects bank preset.",
                );
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    for action in EffectPresetAction::guarded_daily_actions("Personal") {
                        let confirmed = self
                            .pending_confirmation(PendingConfirmationKind::EffectPreset)
                            .is_some_and(|pending| pending == &action.command());
                        let label = if confirmed {
                            format!("Confirm {}", action.label())
                        } else {
                            action.label().to_string()
                        };
                        if ui
                            .add_sized(
                                egui::vec2(
                                    EffectsLayoutPolicy::preset_management_button_width(),
                                    26.0,
                                ),
                                egui::Button::new(label).small(),
                            )
                            .clicked()
                        {
                            if let Some(command) = action.command_if_confirmed(confirmed) {
                                self.set_pending_confirmation(
                                    PendingConfirmationKind::EffectPreset,
                                    None,
                                );
                                self.send(UiCommand::Send(command));
                            } else {
                                self.set_pending_confirmation(
                                    PendingConfirmationKind::EffectPreset,
                                    Some(action.command()),
                                );
                            }
                        }
                    }
                });
                if self.has_pending_confirmation(PendingConfirmationKind::EffectPreset) {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(
                            "Click the same effect preset action again to confirm.",
                        )
                        .small()
                        .color(Self::muted_text()),
                    );
                }
            },
        );
    }

    fn render_effects_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        Self::section_header(
            ui,
            "Voice Effects",
            "Quick presets for the GoXLR effects bank",
            "Fast access to FX on/off, reverb, robot, hard tune, amounts, and style controls without opening the browser UI.",
        );
        ui.add_space(12.0);

        let available_width = ui.available_width();
        let preset_card_width =
            EffectsLayoutPolicy::quick_preset_card_width_for_available_width(available_width);
        Self::polished_row(
            ui,
            egui::vec2(
                EffectsLayoutPolicy::detail_panel_gap(),
                EffectsLayoutPolicy::detail_panel_gap(),
            ),
            |ui| {
                for preset in EffectsQuickPreset::daily_presets() {
                    ui.allocate_ui_with_layout(
                        egui::vec2(
                            preset_card_width,
                            EffectsLayoutPolicy::quick_preset_card_height(),
                        ),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_min_width(preset_card_width);
                            ui.set_max_width(preset_card_width);
                            ui.set_min_height(EffectsLayoutPolicy::quick_preset_card_height());
                            Self::panel_frame().show(ui, |ui| {
                                ui.set_min_width(EffectsLayoutPolicy::quick_preset_inner_width());
                                ui.set_max_width(EffectsLayoutPolicy::quick_preset_inner_width());
                                ui.set_min_height(EffectsLayoutPolicy::quick_preset_inner_height());
                                if ui.add(Self::accent_button(preset.name())).clicked() {
                                    self.send(UiCommand::ApplyScene(UiScene::new(
                                        preset.name(),
                                        preset.commands(),
                                    )));
                                }
                                ui.add_space(4.0);
                                ui.label(preset.description());
                                ui.add_space(3.0);
                                ui.add_sized(
                                    [
                                        EffectsLayoutPolicy::quick_preset_command_label_min_width(),
                                        16.0,
                                    ],
                                    egui::Label::new(
                                        egui::RichText::new(format!(
                                            "{} commands",
                                            preset.commands().len()
                                        ))
                                        .monospace()
                                        .small()
                                        .color(Self::muted_text()),
                                    ),
                                );
                            });
                        },
                    );
                }
            },
        );

        ui.add_space(12.0);
        self.render_effect_preset_management_panel(ui);
        ui.add_space(12.0);
        self.render_profile_browser_panel(
            ui,
            self.profile_browser_for(ProfileBrowserKind::EffectsPreset),
        );

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
            if ui
                .add_sized(
                    egui::vec2(ContentLayoutPolicy::min_action_button_width(), 24.0),
                    egui::Button::new("Hard Tune On").small(),
                )
                .clicked()
            {
                self.send(UiCommand::Send(PersonalCommand::SetHardTuneEnabled(true)));
            }
        });

        ui.add_space(14.0);
        Self::polished_row(
            ui,
            egui::vec2(
                EffectsLayoutPolicy::detail_panel_gap(),
                EffectsLayoutPolicy::detail_panel_gap(),
            ),
            |ui| {
                Self::bounded_panel(ui, EffectsLayoutPolicy::amount_panel_width(), |ui| {
                    ui.label(
                        egui::RichText::new("Amounts")
                            .monospace()
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.label("Send detailed effect amount changes without leaving the native UI.");
                    ui.add_space(8.0);
                    for control in EffectsAmountControl::full_controls() {
                        let mut value = control.default_value();
                        if ui
                            .add_sized(
                                egui::vec2(ContentLayoutPolicy::slider_width(), 20.0),
                                egui::Slider::new(&mut value, control.range())
                                    .text(control.label()),
                            )
                            .changed()
                        {
                            self.send(UiCommand::Send(control.command_for_value(value)));
                        }
                    }
                });

                ui.add_space(EffectsLayoutPolicy::detail_panel_gap());
                Self::bounded_panel(ui, EffectsLayoutPolicy::style_panel_width(), |ui| {
                    ui.label(
                        egui::RichText::new("Styles")
                            .monospace()
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.label("Style buttons cover reverb, echo, pitch, gender, megaphone, robot, and hard tune.");
                    ui.add_space(8.0);
                    Self::polished_row(
                        ui,
                        egui::vec2(
                            EffectsLayoutPolicy::detail_panel_gap(),
                            EffectsLayoutPolicy::detail_panel_gap(),
                        ),
                        |ui| {
                            for group in EffectsStyleGroup::full_groups() {
                                Self::soft_bounded_panel(
                                    ui,
                                    EffectsLayoutPolicy::style_group_card_width(),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(group.label())
                                                .monospace()
                                                .color(egui::Color32::WHITE),
                                        );
                                        ui.add_space(4.0);
                                        ui.horizontal_wrapped(|ui| {
                                            for command in group.commands() {
                                                let label = match &command {
                                                    PersonalCommand::SetReverbStyle(style) => {
                                                        format!("{style:?}")
                                                    }
                                                    PersonalCommand::SetEchoStyle(style) => {
                                                        format!("{style:?}")
                                                    }
                                                    PersonalCommand::SetPitchStyle(style) => {
                                                        format!("{style:?}")
                                                    }
                                                    PersonalCommand::SetGenderStyle(style) => {
                                                        format!("{style:?}")
                                                    }
                                                    PersonalCommand::SetMegaphoneStyle(style) => {
                                                        format!("{style:?}")
                                                    }
                                                    PersonalCommand::SetRobotStyle(style) => {
                                                        format!("{style:?}")
                                                    }
                                                    PersonalCommand::SetHardTuneStyle(style) => {
                                                        format!("{style:?}")
                                                    }
                                                    _ => "Style".to_string(),
                                                };
                                                if ui
                                        .add_sized(
                                            [EffectsLayoutPolicy::style_button_min_width(), 22.0],
                                            egui::Button::new(label).small(),
                                        )
                                        .clicked()
                                    {
                                        self.send(UiCommand::Send(command));
                                    }
                                            }
                                        });
                                    },
                                );
                            }
                        },
                    );
                });

                Self::bounded_panel(ui, EffectsLayoutPolicy::amount_panel_width(), |ui| {
                    ui.label(
                        egui::RichText::new("Advanced DSP")
                            .monospace()
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.label(
                        "Arbitrary clamped reverb sliders plus quick defaults for deeper Effects DSP.",
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Reverb sliders")
                            .monospace()
                            .color(egui::Color32::WHITE),
                    );
                    for slider in EffectsReverbSlider::full_sliders() {
                        let mut value = slider.default_value();
                        if ui
                            .add_sized(
                                egui::vec2(EffectsLayoutPolicy::advanced_slider_width(), 20.0),
                                egui::Slider::new(&mut value, slider.range())
                                    .text(slider.label())
                                    .clamping(egui::SliderClamping::Always),
                            )
                            .changed()
                        {
                            self.send(UiCommand::Send(slider.command_for_value(value)));
                        }
                    }
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Echo sliders")
                            .monospace()
                            .color(egui::Color32::WHITE),
                    );
                    for slider in EffectsEchoSlider::full_sliders() {
                        let mut value = slider.default_value();
                        if ui
                            .add_sized(
                                egui::vec2(EffectsLayoutPolicy::advanced_slider_width(), 20.0),
                                egui::Slider::new(&mut value, slider.range())
                                    .text(slider.label())
                                    .clamping(egui::SliderClamping::Always),
                            )
                            .changed()
                        {
                            self.send(UiCommand::Send(slider.command_for_value(value)));
                        }
                    }
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Pitch sliders")
                            .monospace()
                            .color(egui::Color32::WHITE),
                    );
                    for slider in EffectsPitchSlider::full_sliders() {
                        let mut value = slider.default_value();
                        if ui
                            .add_sized(
                                egui::vec2(EffectsLayoutPolicy::advanced_slider_width(), 20.0),
                                egui::Slider::new(&mut value, slider.range())
                                    .text(slider.label())
                                    .clamping(egui::SliderClamping::Always),
                            )
                            .changed()
                        {
                            self.send(UiCommand::Send(slider.command_for_value(value)));
                        }
                    }
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Megaphone sliders")
                            .monospace()
                            .color(egui::Color32::WHITE),
                    );
                    for slider in EffectsMegaphoneSlider::full_sliders() {
                        let mut value = slider.default_value();
                        if ui
                            .add_sized(
                                egui::vec2(EffectsLayoutPolicy::advanced_slider_width(), 20.0),
                                egui::Slider::new(&mut value, slider.range())
                                    .text(slider.label())
                                    .clamping(egui::SliderClamping::Always),
                            )
                            .changed()
                        {
                            self.send(UiCommand::Send(slider.command_for_value(value)));
                        }
                    }
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Robot sliders")
                            .monospace()
                            .color(egui::Color32::WHITE),
                    );
                    for slider in EffectsRobotSlider::full_sliders() {
                        let mut value = slider.default_value();
                        if ui
                            .add_sized(
                                egui::vec2(EffectsLayoutPolicy::advanced_slider_width(), 20.0),
                                egui::Slider::new(&mut value, slider.range())
                                    .text(slider.label())
                                    .clamping(egui::SliderClamping::Always),
                            )
                            .changed()
                        {
                            self.send(UiCommand::Send(slider.command_for_value(value)));
                        }
                    }
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Hard tune sliders")
                            .monospace()
                            .color(egui::Color32::WHITE),
                    );
                    for slider in EffectsHardTuneSlider::full_sliders() {
                        let mut value = slider.default_value();
                        if ui
                            .add_sized(
                                egui::vec2(EffectsLayoutPolicy::advanced_slider_width(), 20.0),
                                egui::Slider::new(&mut value, slider.range())
                                    .text(slider.label())
                                    .clamping(egui::SliderClamping::Always),
                            )
                            .changed()
                        {
                            self.send(UiCommand::Send(slider.command_for_value(value)));
                        }
                    }
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Quick defaults")
                            .monospace()
                            .color(egui::Color32::WHITE),
                    );
                    for control in EffectsAdvancedControl::daily_controls() {
                        if ui
                            .add_sized(
                                egui::vec2(ContentLayoutPolicy::wide_action_button_width(), 24.0),
                                egui::Button::new(control.label()).small(),
                            )
                            .clicked()
                        {
                            self.send(UiCommand::Send(control.command_for_default()));
                        }
                    }
                });
            },
        );
    }

    fn render_mic_processing_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        Self::section_header(
            ui,
            "Mic",
            "Setup, gate, de-ess, compressor, and safety controls",
            "Three compact cards keep daily mic work aligned instead of drifting diagonally across the window.",
        );
        ui.add_space(12.0);
        Self::polished_row(
            ui,
            egui::vec2(MicLayoutPolicy::panel_gap(), MicLayoutPolicy::panel_gap()),
            |ui| {
                Self::bounded_panel(ui, MicLayoutPolicy::panel_width(), |ui| {
                    ui.label(
                        egui::RichText::new("Mic setup")
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
                        .add_sized(
                            egui::vec2(MicLayoutPolicy::slider_width(), 20.0),
                            egui::Slider::new(&mut mic_gain, 0..=72).text("Mic gain"),
                        )
                        .changed()
                    {
                        self.send(UiCommand::Send(PersonalCommand::SetMicrophoneGain(
                            self.snapshot.mic_type,
                            mic_gain,
                        )));
                    }
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        let save_profile_command = PersonalCommand::SaveMicProfile;
                        let save_profile_confirmed = self
                            .pending_confirmation(PendingConfirmationKind::MicProfile)
                            .is_some_and(|pending| pending == &save_profile_command);
                        let save_profile_label = if save_profile_confirmed {
                            "Confirm save mic profile"
                        } else {
                            "Arm save mic profile"
                        };
                        if ui
                            .add_sized(
                                egui::vec2(ContentLayoutPolicy::wide_action_button_width(), 34.0),
                                Self::accent_button(save_profile_label),
                            )
                            .clicked()
                        {
                            if save_profile_confirmed {
                                self.set_pending_confirmation(
                                    PendingConfirmationKind::MicProfile,
                                    None,
                                );
                                self.send(UiCommand::Send(save_profile_command));
                            } else {
                                self.set_pending_confirmation(
                                    PendingConfirmationKind::MicProfile,
                                    Some(save_profile_command),
                                );
                            }
                        }
                        if ui
                            .add_sized(
                                egui::vec2(ContentLayoutPolicy::wide_action_button_width(), 34.0),
                                Self::accent_button("Reload settings"),
                            )
                            .clicked()
                        {
                            self.send(UiCommand::Send(PersonalCommand::ReloadSettings));
                        }
                    });
                });

                Self::bounded_panel(ui, MicLayoutPolicy::setup_guide_panel_width(), |ui| {
                    ui.label(
                        egui::RichText::new("Setup guide")
                            .monospace()
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Read-only workflow hints until live mic metering is available.",
                        )
                        .color(Self::muted_text()),
                    );
                    ui.separator();
                    for step in MicSetupGuideStep::daily_steps() {
                        ui.label(egui::RichText::new(step.label()).strong());
                        ui.label(egui::RichText::new(step.description()).color(Self::muted_text()));
                        ui.add_space(4.0);
                    }
                    ui.separator();
                    ui.label(egui::RichText::new("Live meter").monospace().strong());
                    ui.label(
                        egui::RichText::new(MicSetupGuideStep::live_meter_status_note())
                            .color(Self::muted_text()),
                    );
                });

                Self::bounded_panel(ui, MicLayoutPolicy::panel_width(), |ui| {
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
                        .add_sized(
                            egui::vec2(MicLayoutPolicy::slider_width(), 20.0),
                            egui::Slider::new(&mut gate_threshold, -59..=0)
                                .text("Gate threshold dB"),
                        )
                        .changed()
                    {
                        self.send(UiCommand::Send(PersonalCommand::SetGateThreshold(
                            gate_threshold,
                        )));
                    }
                    let mut gate_attenuation = self.snapshot.gate_attenuation;
                    if ui
                        .add_sized(
                            egui::vec2(MicLayoutPolicy::slider_width(), 20.0),
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
                        .add_sized(
                            egui::vec2(MicLayoutPolicy::slider_width(), 20.0),
                            egui::Slider::new(&mut deesser, 0..=100).text("De-esser %"),
                        )
                        .changed()
                    {
                        self.send(UiCommand::Send(PersonalCommand::SetDeesser(deesser)));
                    }
                });

                Self::bounded_panel(ui, MicLayoutPolicy::panel_width(), |ui| {
                    ui.label(
                        egui::RichText::new("Compressor & safety")
                            .monospace()
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    let mut compressor_threshold = self.snapshot.compressor_threshold;
                    if ui
                        .add_sized(
                            egui::vec2(MicLayoutPolicy::slider_width(), 20.0),
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
                        .add_sized(
                            egui::vec2(MicLayoutPolicy::slider_width(), 20.0),
                            egui::Slider::new(&mut makeup_gain, 0..=24).text("Makeup gain dB"),
                        )
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
                            if ui
                                .add_sized(
                                    egui::vec2(
                                        ContentLayoutPolicy::min_action_button_width(),
                                        22.0,
                                    ),
                                    egui::Button::new(Self::compressor_ratio_label(ratio)).small(),
                                )
                                .clicked()
                            {
                                self.send(UiCommand::Send(PersonalCommand::SetCompressorRatio(
                                    ratio,
                                )));
                            }
                        }
                    });
                    let mut clip_threshold = self.snapshot.clip_guard_threshold;
                    if ui
                        .add_sized(
                            egui::vec2(MicLayoutPolicy::slider_width(), 20.0),
                            egui::Slider::new(&mut clip_threshold, 0..=100)
                                .text("ClipGuard threshold"),
                        )
                        .changed()
                    {
                        self.send(UiCommand::Send(PersonalCommand::SetClipGuardThreshold(
                            clip_threshold,
                        )));
                    }
                    let mut limiter_threshold = self.snapshot.headphone_limiter_threshold;
                    if ui
                        .add_sized(
                            egui::vec2(MicLayoutPolicy::slider_width(), 20.0),
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

                Self::bounded_panel(ui, MicLayoutPolicy::eq_panel_width(), |ui| {
                    ui.label(
                        egui::RichText::new("Mic EQ")
                            .monospace()
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.label(
                        "First-pass mini/full band EQ sends typed gain and frequency commands.",
                    );
                    ui.separator();
                    ui.label(egui::RichText::new("Mini EQ band nudges").strong());
                    ui.horizontal_wrapped(|ui| {
                        for band in MicEqBandControl::mini_bands() {
                            if ui
                                .add_sized(
                                    egui::vec2(MicLayoutPolicy::eq_slider_width(), 22.0),
                                    egui::Button::new(format!("{} +1dB", band.label())).small(),
                                )
                                .clicked()
                            {
                                self.send(UiCommand::Send(band.gain_command(1)));
                            }
                            if ui
                                .add_sized(
                                    egui::vec2(MicLayoutPolicy::eq_slider_width(), 22.0),
                                    egui::Button::new(format!("{} freq", band.label())).small(),
                                )
                                .clicked()
                            {
                                self.send(UiCommand::Send(
                                    band.frequency_command(band.default_frequency_hz()),
                                ));
                            }
                        }
                    });
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Full EQ band nudges").strong());
                    ui.horizontal_wrapped(|ui| {
                        for band in MicEqBandControl::full_bands() {
                            if ui
                                .add_sized(
                                    egui::vec2(MicLayoutPolicy::eq_slider_width(), 22.0),
                                    egui::Button::new(format!("{} +1dB", band.label())).small(),
                                )
                                .clicked()
                            {
                                self.send(UiCommand::Send(band.gain_command(1)));
                            }
                            if ui
                                .add_sized(
                                    egui::vec2(MicLayoutPolicy::eq_slider_width(), 22.0),
                                    egui::Button::new(format!("{} freq", band.label())).small(),
                                )
                                .clicked()
                            {
                                self.send(UiCommand::Send(
                                    band.frequency_command(band.default_frequency_hz()),
                                ));
                            }
                        }
                    });
                });

                Self::bounded_panel(ui, MicLayoutPolicy::profile_panel_width(), |ui| {
                    ui.label(
                        egui::RichText::new("Mic profiles")
                            .monospace()
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.label("Guarded profile actions use an explicit named slot before destructive commands.");
                    for action in MicProfileAction::guarded_daily_actions("Personal") {
                        let confirmed = self
                            .pending_confirmation(PendingConfirmationKind::MicProfile)
                            .is_some_and(|pending| pending == &action.command());
                        let label = if action.requires_confirmation() && confirmed {
                            format!("Confirm again: {}", action.label())
                        } else if action.requires_confirmation() {
                            format!("Arm: {}", action.label())
                        } else {
                            action.label().to_string()
                        };
                        if ui
                            .add_sized(
                                egui::vec2(ContentLayoutPolicy::wide_action_button_width(), 24.0),
                                egui::Button::new(label).small(),
                            )
                            .clicked()
                        {
                            if let Some(command) = action.command_if_confirmed(confirmed) {
                                self.set_pending_confirmation(
                                    PendingConfirmationKind::MicProfile,
                                    None,
                                );
                                self.send(UiCommand::Send(command));
                            } else {
                                self.set_pending_confirmation(
                                    PendingConfirmationKind::MicProfile,
                                    Some(action.command()),
                                );
                            }
                        }
                    }
                    if self.has_pending_confirmation(PendingConfirmationKind::MicProfile) {
                        ui.label(
                            egui::RichText::new(
                                "A guarded profile action is armed; click the same action again to send it.",
                            )
                            .small(),
                        );
                    }
                });

                self.render_profile_browser_panel(
                    ui,
                    self.profile_browser_for(ProfileBrowserKind::Mic),
                );
            },
        );
    }

    fn render_headphone_audio_presets_panel(&mut self, ui: &mut egui::Ui) {
        Self::bounded_panel(ui, HeadphoneEqLayoutPolicy::preset_panel_width(), |ui| {
            ui.label(
                egui::RichText::new("Listening presets")
                    .monospace()
                    .size(18.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(
                    "Hardware-first listening presets: route to headphones, gain-stage, enable limiter, then apply EQ.",
                )
                .color(Self::muted_text()),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for preset in HeadphoneListeningPreset::daily_presets() {
                    Self::soft_sized_panel(
                        ui,
                        HeadphoneEqLayoutPolicy::preset_button_width(),
                        HeadphoneEqLayoutPolicy::preset_card_height(),
                        |ui| {
                            let button = if preset.is_safety_preset() {
                                Self::accent_button(preset.name())
                            } else {
                                egui::Button::new(preset.name())
                            };
                            if ui
                                .add_sized(
                                    egui::vec2(
                                        HeadphoneEqLayoutPolicy::preset_button_width() - 16.0,
                                        24.0,
                                    ),
                                    button,
                                )
                                .clicked()
                            {
                                self.send(UiCommand::ApplyScene(preset.to_scene()));
                            }
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(preset.description())
                                    .small()
                                    .color(Self::muted_text()),
                            );
                        },
                    );
                }
            });
            ui.separator();
            for step in HeadphoneAudioStep::recommended_steps() {
                ui.label(egui::RichText::new(step.label()).strong());
                ui.label(
                    egui::RichText::new(step.description())
                        .small()
                        .color(Self::muted_text()),
                );
                ui.add_space(4.0);
            }
        });
    }

    fn render_headphone_eq_profile_panel(&mut self, ui: &mut egui::Ui) {
        Self::bounded_panel(ui, HeadphoneEqLayoutPolicy::profile_panel_width(), |ui| {
            ui.label(
                egui::RichText::new("EQ profiles")
                    .monospace()
                    .size(18.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(
                    "Guarded named-slot profile actions. Click the same action twice to send it.",
                )
                .color(Self::muted_text()),
            );
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                for action in HeadphoneEqProfileAction::guarded_daily_actions("Personal Phones") {
                    let confirmed = self
                        .pending_confirmation(PendingConfirmationKind::HeadphoneEqProfile)
                        .is_some_and(|pending| pending == &action.command());
                    let label = if confirmed {
                        format!("CONFIRM {}", action.label())
                    } else {
                        action.label().to_string()
                    };
                    let button = if confirmed {
                        Self::accent_button(label)
                    } else {
                        egui::Button::new(label).small()
                    };
                    if ui
                        .add_sized(
                            egui::vec2(HeadphoneEqLayoutPolicy::profile_button_width(), 24.0),
                            button,
                        )
                        .clicked()
                    {
                        if let Some(command) = action.command_if_confirmed(confirmed) {
                            self.set_pending_confirmation(
                                PendingConfirmationKind::HeadphoneEqProfile,
                                None,
                            );
                            self.send(UiCommand::Send(command));
                        } else {
                            self.set_pending_confirmation(
                                PendingConfirmationKind::HeadphoneEqProfile,
                                Some(action.command()),
                            );
                        }
                    }
                }
            });
            if self.has_pending_confirmation(PendingConfirmationKind::HeadphoneEqProfile) {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "Pending confirmation: click the same EQ profile action again.",
                    )
                    .color(egui::Color32::YELLOW),
                );
            }
        });
    }

    fn render_headphone_eq_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        Self::section_header(
            ui,
            "Headphone EQ",
            "Ten-band headphone equalizer controls",
            "Primary EQ controls stay in a fixed grid; presets and profile actions sit in aligned cards below.",
        );
        ui.add_space(12.0);

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(
                ContentLayoutPolicy::desktop_panel_gap(),
                ContentLayoutPolicy::desktop_panel_gap(),
            );
            Self::bounded_panel(ui, HeadphoneEqLayoutPolicy::panel_width(), |ui| {
                ui.label(
                    egui::RichText::new("Headphone EQ")
                        .monospace()
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.add(Self::accent_button("Enable EQ")).clicked() {
                        self.send(UiCommand::Send(PersonalCommand::SetHeadphoneEqEnabled(
                            true,
                        )));
                    }
                    if ui.button("Disable EQ").clicked() {
                        self.send(UiCommand::Send(PersonalCommand::SetHeadphoneEqEnabled(
                            false,
                        )));
                    }
                    if ui.button("Preamp 0 dB").clicked() {
                        self.send(UiCommand::Send(PersonalCommand::SetHeadphoneEqPreamp(0.0)));
                    }
                });
                ui.separator();
                let bands = HeadphoneEqBandControl::ten_band_editor();
                let columns = HeadphoneEqLayoutPolicy::grid_columns();
                egui::Grid::new("headphone_eq_band_grid")
                    .num_columns(columns)
                    .spacing(egui::vec2(
                        HeadphoneEqLayoutPolicy::band_card_gap(),
                        HeadphoneEqLayoutPolicy::band_card_gap(),
                    ))
                    .show(ui, |ui| {
                        for (index, band) in bands.iter().enumerate() {
                            Self::soft_sized_panel(
                                ui,
                                HeadphoneEqLayoutPolicy::band_card_width(),
                                HeadphoneEqLayoutPolicy::band_card_height(),
                                |ui| {
                                    ui.set_min_width(
                                        HeadphoneEqLayoutPolicy::band_card_width() - 20.0,
                                    );
                                    ui.label(
                                        egui::RichText::new(band.label()).monospace().strong(),
                                    );
                                    if ui.button("Gain 0").clicked() {
                                        self.send(UiCommand::Send(band.gain_command(0.0)));
                                    }
                                    if ui.button("Freq default").clicked() {
                                        self.send(UiCommand::Send(
                                            band.frequency_command(band.default_frequency_hz()),
                                        ));
                                    }
                                    if ui.button("Q 0.9").clicked() {
                                        self.send(UiCommand::Send(band.q_command(0.9)));
                                    }
                                },
                            );
                            if (index + 1) % columns == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });
            self.render_headphone_audio_presets_panel(ui);
            self.render_headphone_eq_profile_panel(ui);
            self.render_profile_browser_panel(
                ui,
                self.profile_browser_for(ProfileBrowserKind::HeadphoneEq),
            );
        });
    }

    fn render_sampler_live_slots_panel(&mut self, ui: &mut egui::Ui) {
        Self::bounded_panel(
            ui,
            SamplerLayoutPolicy::sample_browser_panel_width(),
            |ui| {
                ui.label(
                    egui::RichText::new("Live sample slots")
                        .monospace()
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.label(
                egui::RichText::new(
                    "Daemon-reported sampler contents. Remove is guarded; play and trim reset target the shown index.",
                )
                .color(Self::muted_text()),
            );
                ui.add_space(6.0);

                let slots = self.snapshot.sampler_slots.clone();
                if slots.is_empty() {
                    ui.label(
                    egui::RichText::new(
                        "No live sampler slot state is available from the selected daemon/device yet.",
                    )
                    .color(Self::muted_text()),
                );
                    return;
                }

                egui::ScrollArea::vertical()
                .max_height(360.0)
                .id_salt("sampler_live_slots")
                .show(ui, |ui| {
                    for slot in slots {
                        ui.group(|ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{:?} / {:?}",
                                        slot.bank(),
                                        slot.button()
                                    ))
                                    .monospace()
                                    .strong(),
                                );
                                ui.label(format!("{} sample(s)", slot.sample_count()));
                                ui.label(slot.status_label());
                                ui.label(format!("{:?} / {:?}", slot.function(), slot.order()));
                            });

                            if slot.samples().is_empty() {
                                ui.label(egui::RichText::new("Empty slot").color(Self::muted_text()));
                            } else {
                                for sample in slot.samples() {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(format!(
                                            "#{} {} ({})",
                                            sample.index() + 1,
                                            sample.name(),
                                            sample.trim_label()
                                        ));

                                        let play = SamplerFileAction::play_by_index(
                                            slot.bank(),
                                            slot.button(),
                                            sample.index(),
                                        );
                                        if ui
                                            .add_sized(
                                                egui::vec2(
                                                    SamplerLayoutPolicy::sample_browser_row_button_width(),
                                                    22.0,
                                                ),
                                                egui::Button::new(play.label()).small(),
                                            )
                                            .on_hover_text(play.description())
                                            .clicked()
                                        {
                                            self.handle_sampler_file_action(&play);
                                        }

                                        let remove = SamplerFileAction::remove_by_index(
                                            slot.bank(),
                                            slot.button(),
                                            sample.index(),
                                        );
                                        let confirmed = self
                                            .pending_confirmation(
                                                PendingConfirmationKind::SamplerFile,
                                            )
                                            .is_some_and(|pending| pending == &remove.command());
                                        let remove_label = if confirmed {
                                            format!("Confirm {}", remove.label())
                                        } else {
                                            format!("Arm {}", remove.label())
                                        };
                                        if ui
                                            .add_sized(
                                                egui::vec2(112.0, 22.0),
                                                egui::Button::new(remove_label).small(),
                                            )
                                            .on_hover_text(remove.description())
                                            .clicked()
                                        {
                                            self.handle_sampler_file_action(&remove);
                                        }

                                        for trim in SampleTrimAction::safe_trim_actions(
                                            slot.bank(),
                                            slot.button(),
                                            sample.index(),
                                        ) {
                                            if ui
                                                .add_sized(
                                                    egui::vec2(84.0, 22.0),
                                                    egui::Button::new(trim.label()).small(),
                                                )
                                                .clicked()
                                            {
                                                self.send(UiCommand::Send(trim.command()));
                                            }
                                        }
                                    });

                                    ui.horizontal_wrapped(|ui| {
                                        let trim_editor = SampleTrimEditor::new(
                                            slot.bank(),
                                            slot.button(),
                                            sample.index(),
                                            sample.start_pct(),
                                            sample.stop_pct(),
                                        );
                                        ui.label(
                                            egui::RichText::new("Custom trim")
                                                .monospace()
                                                .color(Self::muted_text()),
                                        );
                                        let mut start_pct = trim_editor.start_pct();
                                        if ui
                                            .add_sized(
                                                egui::vec2(
                                                    SamplerLayoutPolicy::custom_trim_slider_width(),
                                                    20.0,
                                                ),
                                                egui::Slider::new(&mut start_pct, 0.0..=100.0)
                                                    .text(trim_editor.start_label())
                                                    .clamping(egui::SliderClamping::Always),
                                            )
                                            .on_hover_text(
                                                "Set this sample's start point to an arbitrary percentage from 0–100.",
                                            )
                                            .changed()
                                        {
                                            self.send(UiCommand::Send(
                                                trim_editor.start_command(start_pct),
                                            ));
                                        }

                                        let mut stop_pct = trim_editor.stop_pct();
                                        if ui
                                            .add_sized(
                                                egui::vec2(
                                                    SamplerLayoutPolicy::custom_trim_slider_width(),
                                                    20.0,
                                                ),
                                                egui::Slider::new(&mut stop_pct, 0.0..=100.0)
                                                    .text(trim_editor.stop_label())
                                                    .clamping(egui::SliderClamping::Always),
                                            )
                                            .on_hover_text(
                                                "Set this sample's stop point to an arbitrary percentage from 0–100.",
                                            )
                                            .changed()
                                        {
                                            self.send(UiCommand::Send(trim_editor.stop_command(stop_pct)));
                                        }
                                    });
                                }
                            }
                        });
                        ui.add_space(6.0);
                    }
                });
            },
        );
    }

    fn render_sampler_file_workflow_panel(&mut self, ui: &mut egui::Ui) {
        Self::bounded_panel(ui, SamplerLayoutPolicy::file_workflow_panel_width(), |ui| {
            ui.label(
                egui::RichText::new("Sample file workflow")
                    .monospace()
                    .size(18.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(
                    "Paste an audio file path, then arm the exact bank/slot action before importing or removing sample index 0.",
                )
                .color(Self::muted_text()),
            );
            ui.add_space(6.0);
            ui.label("Audio file path to add:");
            ui.add(
                egui::TextEdit::singleline(&mut self.sampler_file_path)
                    .hint_text("/home/pc/samples/clip.wav")
                    .desired_width(SamplerLayoutPolicy::file_workflow_panel_width() - 28.0),
            );
            ui.add_space(8.0);
            ui.separator();
            ui.label("Sample browser directory:");
            ui.add(
                egui::TextEdit::singleline(&mut self.sampler_browser_path)
                    .hint_text("defaults/resources/samples")
                    .desired_width(SamplerLayoutPolicy::sample_browser_panel_width() - 28.0),
            );
            let browser = self.sampler_sample_browser();
            ui.label(
                egui::RichText::new(format!(
                    "{} supported audio file(s) in {}",
                    browser.rows().len(),
                    browser.root().display()
                ))
                .color(Self::muted_text()),
            );
            if browser.is_empty() {
                ui.label(
                    egui::RichText::new(
                        "No .wav/.mp3/.flac/.ogg/.aiff/.aac/.m4a files found here.",
                    )
                    .color(Self::muted_text()),
                );
            } else {
                egui::ScrollArea::vertical()
                    .max_height(140.0)
                    .id_salt("sampler_sample_browser")
                    .show(ui, |ui| {
                        for row in browser.rows() {
                            ui.horizontal(|ui| {
                                if ui
                                    .add_sized(
                                        egui::vec2(
                                            SamplerLayoutPolicy::sample_browser_row_button_width(),
                                            22.0,
                                        ),
                                        egui::Button::new("Use path").small(),
                                    )
                                    .on_hover_text(
                                        "Copy this audio file path into the add-path field above.",
                                    )
                                    .clicked()
                                {
                                    self.sampler_file_path = row.path().to_string();
                                    self.set_pending_confirmation(
                                        PendingConfirmationKind::SamplerFile,
                                        None,
                                    );
                                }
                                ui.label(row.display_name());
                            });
                        }
                    });
            }
            ui.add_space(8.0);

            let buttons = [
                SampleButtons::TopLeft,
                SampleButtons::TopRight,
                SampleButtons::BottomLeft,
                SampleButtons::BottomRight,
            ];
            let sample_path = self.sampler_file_path.clone();
            for bank in [SampleBank::A, SampleBank::B, SampleBank::C] {
                ui.label(
                    egui::RichText::new(format!("Bank {bank:?}"))
                        .monospace()
                        .strong(),
                );
                for row in buttons.chunks(2) {
                    ui.horizontal(|ui| {
                        for button in row {
                            ui.vertical(|ui| {
                                ui.label(format!("{button:?}"));
                                let mut actions = Vec::new();
                                if let Some(action) =
                                    SamplerFileAction::add_from_path(bank, *button, &sample_path)
                                {
                                    actions.push(action);
                                }
                                actions.push(SamplerFileAction::remove_first(bank, *button));
                                actions.push(SamplerFileAction::play_first(bank, *button));

                                for action in actions {
                                    let command = action.command();
                                    let confirmed = self
                                        .pending_confirmation(PendingConfirmationKind::SamplerFile)
                                        .is_some_and(|pending| pending == &command);
                                    let label = if action.requires_confirmation() && confirmed {
                                        format!("Confirm {}", action.label())
                                    } else if action.requires_confirmation() {
                                        format!("Arm {}", action.label())
                                    } else {
                                        action.label().to_string()
                                    };
                                    if ui
                                        .add_sized(
                                            egui::vec2(
                                                SamplerLayoutPolicy::file_workflow_button_width(),
                                                22.0,
                                            ),
                                            egui::Button::new(label).small(),
                                        )
                                        .on_hover_text(action.description())
                                        .clicked()
                                    {
                                        self.handle_sampler_file_action(&action);
                                    }
                                }
                            });
                        }
                    });
                }
                ui.add_space(6.0);
            }

            if self.has_pending_confirmation(PendingConfirmationKind::SamplerFile) {
                ui.label(
                    egui::RichText::new(
                        "Click the same armed sampler file action again to send it.",
                    )
                    .color(egui::Color32::YELLOW),
                );
            }
            if self.sampler_file_path.trim().is_empty() {
                ui.label(
                    egui::RichText::new("Add buttons appear after a non-empty path is entered.")
                        .color(Self::muted_text()),
                );
            }
        });
    }

    fn render_sampler_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        Self::section_header(
            ui,
            "Sampler",
            "Sampler bank and playback controls",
            "First-pass sampler parity focuses on bank selection, play/stop mode, play-next, stop, random order, safe workflow settings, and default trim reset controls.",
        );
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(
                ContentLayoutPolicy::desktop_panel_gap(),
                ContentLayoutPolicy::desktop_panel_gap(),
            );
            {
                Self::bounded_panel(ui, SamplerLayoutPolicy::panel_width(), |ui| {
                    ui.label(
                        egui::RichText::new("Workflow settings")
                            .monospace()
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.label(
                        "Safe global sampler actions; add/remove sample files are guarded separately.",
                    );
                    ui.separator();
                    for setting in SamplerWorkflowSetting::safe_settings() {
                        if ui
                            .add_sized(
                                egui::vec2(ContentLayoutPolicy::wide_action_button_width(), 24.0),
                                egui::Button::new(setting.label()).small(),
                            )
                            .on_hover_text(setting.description())
                            .clicked()
                        {
                            self.send(UiCommand::Send(setting.command()));
                        }
                    }
                });
                self.render_sampler_live_slots_panel(ui);
                self.render_sampler_file_workflow_panel(ui);
                for bank in [SampleBank::A, SampleBank::B, SampleBank::C] {
                    Self::bounded_panel(ui, SamplerLayoutPolicy::panel_width(), |ui| {
                        ui.label(
                            egui::RichText::new(format!("Bank {bank:?}"))
                                .monospace()
                                .size(18.0)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );
                        let buttons = [
                            SampleButtons::TopLeft,
                            SampleButtons::TopRight,
                            SampleButtons::BottomLeft,
                            SampleButtons::BottomRight,
                        ];
                        ui.scope(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(
                                SamplerLayoutPolicy::bank_slot_gap(),
                                SamplerLayoutPolicy::bank_slot_gap(),
                            );
                            for row in buttons.chunks(SamplerLayoutPolicy::bank_slot_columns()) {
                                ui.horizontal(|ui| {
                                    for button in row {
                                        Self::soft_sized_panel(
                                            ui,
                                            SamplerLayoutPolicy::bank_slot_card_width(),
                                            SamplerLayoutPolicy::bank_slot_card_height(),
                                            |ui| {
                                                ui.label(
                                                    egui::RichText::new(format!("{button:?}"))
                                                        .monospace()
                                                        .strong(),
                                                );
                                                for action_row in SamplerAction::daily_bank_actions(
                                                    bank, *button,
                                                )
                                                .chunks(2)
                                                {
                                                    ui.horizontal(|ui| {
                                                        for action in action_row {
                                                            if ui
                                                                .add_sized(
                                                                    egui::vec2(70.0, 20.0),
                                                                    egui::Button::new(action.label())
                                                                        .small(),
                                                                )
                                                                .clicked()
                                                            {
                                                                self.send(UiCommand::Send(
                                                                    action.command(),
                                                                ));
                                                            }
                                                        }
                                                    });
                                                }
                                                ui.horizontal(|ui| {
                                                    for action in SampleTrimAction::safe_trim_actions(
                                                        bank, *button, 0,
                                                    ) {
                                                        if ui
                                                            .add_sized(
                                                                egui::vec2(70.0, 20.0),
                                                                egui::Button::new(action.label()).small(),
                                                            )
                                                            .on_hover_text("Reset sample slot 0 trim boundary without importing or removing files.")
                                                            .clicked()
                                                        {
                                                            self.send(UiCommand::Send(
                                                                action.command(),
                                                            ));
                                                        }
                                                    }
                                                });
                                            },
                                        );
                                    }
                                });
                            }
                        });
                    });
                }
            }
        });
    }

    fn render_mixer_dashboard(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        Self::section_header(
            ui,
            "Mixer",
            "Profiles, scenes, faders, and active app routing",
            "Daily controls are grouped in stable rows: overview first, hardware faders second, detailed routing below.",
        );
        ui.add_space(12.0);
        let gap = MixerLayoutPolicy::panel_gap();

        Self::mixer_card_row(ui, gap, |ui| {
            self.render_scene_panel(ui);
            Self::bounded_sized_panel(
                ui,
                MixerLayoutPolicy::panel_width(),
                MixerLayoutPolicy::overview_panel_height(),
                |ui| {
                    ui.add_space(34.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("Mixer")
                                .monospace()
                                .size(18.0)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );
                        ui.add_space(14.0);
                        let channel_labels = ControlledChannel::mvp_channels();
                        Self::centered_exact_width(ui, 406.0, |ui| {
                            egui::Grid::new("mixer_channel_strip_grid")
                                .num_columns(4)
                                .min_col_width(MixerLayoutPolicy::channel_strip_width())
                                .spacing(egui::vec2(10.0, 10.0))
                                .show(ui, |ui| {
                                    for (index, volume) in
                                        self.pending_volumes.clone().into_iter().enumerate()
                                    {
                                        let label = channel_labels
                                            .iter()
                                            .find(|channel| channel.channel == volume.channel)
                                            .map(|channel| channel.label)
                                            .unwrap_or("Channel");
                                        self.render_channel_strip(
                                            ui,
                                            label,
                                            volume.channel,
                                            volume.value,
                                        );
                                        if (index + 1) % 4 == 0 {
                                            ui.end_row();
                                        }
                                    }
                                });
                        });
                        ui.add_space(16.0);
                        Self::centered_exact_width(ui, 474.0, |ui| {
                            egui::Grid::new("mixer_action_button_grid")
                                .num_columns(3)
                                .min_col_width(150.0)
                                .spacing(egui::vec2(12.0, 10.0))
                                .show(ui, |ui| {
                                    if ui
                                        .add_sized(
                                            egui::vec2(150.0, 34.0),
                                            Self::accent_button("Enable ClipGuard"),
                                        )
                                        .clicked()
                                    {
                                        self.send(UiCommand::Send(
                                            PersonalCommand::SetClipGuardEnabled(true),
                                        ));
                                    }
                                    if ui
                                        .add_sized(
                                            egui::vec2(150.0, 34.0),
                                            Self::accent_button("Enable limiter"),
                                        )
                                        .clicked()
                                    {
                                        self.send(UiCommand::Send(
                                            PersonalCommand::SetHeadphoneLimiterEnabled(true),
                                        ));
                                    }
                                    if ui
                                        .add_sized(
                                            egui::vec2(150.0, 34.0),
                                            Self::accent_button("Enable EQ"),
                                        )
                                        .clicked()
                                    {
                                        self.send(UiCommand::Send(
                                            PersonalCommand::SetHeadphoneEqEnabled(true),
                                        ));
                                    }
                                    ui.end_row();
                                });
                        });
                    });
                },
            );
        });

        ui.add_space(gap);
        Self::mixer_card_row(ui, gap, |ui| {
            self.render_status_card(ui);
            self.render_monitor_mix_panel(ui);
        });

        ui.add_space(gap);
        Self::mixer_card_row(ui, gap, |ui| {
            self.render_fader_assignment_panel(ui);
            self.render_active_streams_panel(ui);
        });

        ui.add_space(gap);
        Self::mixer_card_row(ui, gap, |ui| {
            self.render_submix_panel(ui);
            self.render_scribble_strip_panel(ui);
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

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Self::bg()).inner_margin(egui::Margin::same(14)))
            .show(ctx, |ui| {
            self.render_header_controls(ui, ctx);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for (label, selected) in [
                    ("Mic", self.quick_actions.view_mode() == AppViewMode::Mic),
                    ("Effects", self.quick_actions.view_mode() == AppViewMode::Effects),
                    ("Lighting", self.quick_actions.view_mode() == AppViewMode::Lighting),
                    (
                        "Headphone EQ",
                        self.quick_actions.view_mode() == AppViewMode::HeadphoneEq,
                    ),
                    ("Sampler", self.quick_actions.view_mode() == AppViewMode::Sampler),
                    (
                        "System",
                        self.quick_actions.view_mode() == AppViewMode::System,
                    ),
                    (
                        "Diagnostics",
                        self.quick_actions.view_mode() == AppViewMode::Diagnostics,
                    ),
                    ("About", self.quick_actions.view_mode() == AppViewMode::About),
                    (DashboardCopy::mixer_tab(), self.quick_actions.view_mode() == AppViewMode::QuickActions),
                    (DashboardCopy::configuration_tab(), self.quick_actions.view_mode() == AppViewMode::Full),
                    ("Routing", false),
                ] {
                    let text = egui::RichText::new(label)
                        .monospace()
                        .color(if selected { Self::accent() } else { egui::Color32::WHITE });
                    let button = egui::Button::new(text)
                        .fill(if selected { egui::Color32::from_rgb(58, 36, 22) } else { Self::bg() })
                        .stroke(egui::Stroke::new(1.0, if selected { Self::accent() } else { Self::panel_border() }))
                        .min_size(egui::vec2(138.0, 34.0));
                    if ui.add(button).clicked() {
                        match label {
                            "Mic" => self.quick_actions.set_view_mode(AppViewMode::Mic),
                            "Effects" => self.quick_actions.set_view_mode(AppViewMode::Effects),
                            "Lighting" => self.quick_actions.set_view_mode(AppViewMode::Lighting),
                            "Headphone EQ" => self.quick_actions.set_view_mode(AppViewMode::HeadphoneEq),
                            "Sampler" => self.quick_actions.set_view_mode(AppViewMode::Sampler),
                            "System" => self.quick_actions.set_view_mode(AppViewMode::System),
                            "Diagnostics" => self.quick_actions.set_view_mode(AppViewMode::Diagnostics),
                            "About" => self.quick_actions.set_view_mode(AppViewMode::About),
                            label if label == DashboardCopy::mixer_tab() => self.quick_actions.set_view_mode(AppViewMode::QuickActions),
                            label if label == DashboardCopy::configuration_tab() => self.quick_actions.set_view_mode(AppViewMode::Full),
                            _ => {}
                        }
                    }
                }
            });

            egui::ScrollArea::vertical()
                .id_salt(ContentLayoutPolicy::scroll_area_id())
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    Self::centered_page_body(ui, |ui| {
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

            if self.quick_actions.view_mode() == AppViewMode::Lighting {
                self.render_lighting_page(ui);
                return;
            }

            if self.quick_actions.view_mode() == AppViewMode::HeadphoneEq {
                self.render_headphone_eq_page(ui);
                return;
            }

            if self.quick_actions.view_mode() == AppViewMode::Sampler {
                self.render_sampler_page(ui);
                return;
            }

            if self.quick_actions.view_mode() == AppViewMode::System {
                self.render_system_page(ui);
                return;
            }

            if self.quick_actions.view_mode() == AppViewMode::Diagnostics {
                self.render_diagnostics_page(ui);
                return;
            }

            if self.quick_actions.view_mode() == AppViewMode::About {
                self.render_about_page(ui);
                return;
            }

            Self::section_header(
                ui,
                "Config / Routing",
                "Scenes, named routing presets, router matrix, and volume safety controls",
                "Configuration controls now sit in aligned cards; the routing matrix stays explicit and readable for manual route changes.",
            );
            ui.add_space(12.0);
            Self::bounded_panel(ui, 720.0, |ui| {
                if let Some(profile) = &self.snapshot.profile_name {
                    ui.label(format!("Profile: {profile}"));
                }
                if let Some(mic_profile) = &self.snapshot.mic_profile_name {
                    ui.label(format!("Mic profile: {mic_profile}"));
                }

                ui.add_space(10.0);
                ui.heading("Scenes");
                ui.label(format!("Scene config: {}", self.scene_config.path().display()));
                if let Some(error) = self.scene_config.reload_error() {
                    ui.colored_label(egui::Color32::YELLOW, format!("Using previous scenes: {error}"));
                }
                Self::polished_row(ui, egui::vec2(8.0, 8.0), |ui| {
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
            Self::bounded_panel(ui, 700.0, |ui| {
                ui.heading("Routing rule diff");
                ui.label("Compares saved app routing rules with currently active playback stream routes.");
                let rules = self.scene_config.config().audio_routing_rules();
                let diffs = self.snapshot.active_audio_streams.routing_rule_diffs(&rules);
                if diffs.is_empty() {
                    ui.label("No persistent routing rules configured.");
                } else {
                    let pending_moves = self.snapshot.active_audio_streams.routing_moves(&rules);
                    let needs_move_count = diffs
                        .iter()
                        .filter(|row| row.status() == RoutingRuleDiffStatus::NeedsMove)
                        .count();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{} rules", diffs.len()));
                        ui.separator();
                        ui.colored_label(
                            RoutingRuleDiffStatus::NeedsMove.color(),
                            format!("{needs_move_count} need moves"),
                        );
                        if ui
                            .add_enabled(!pending_moves.is_empty(), egui::Button::new("Apply pending moves"))
                            .clicked()
                        {
                            for command in pending_moves {
                                self.send(command);
                            }
                        }
                    });
                    ui.add_space(6.0);
                    egui::Grid::new("personal_routing_rule_diff")
                        .striped(true)
                        .spacing(egui::vec2(10.0, 4.0))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Rule").strong());
                            ui.label(egui::RichText::new("Current").strong());
                            ui.label(egui::RichText::new("Desired").strong());
                            ui.label(egui::RichText::new("Status").strong());
                            ui.end_row();

                            for row in diffs {
                                ui.label(row.app());
                                ui.label(row.current_route().unwrap_or("—"));
                                ui.label(row.desired_route());
                                ui.colored_label(row.status_color(), row.status_label())
                                    .on_hover_text(row.summary());
                                ui.end_row();
                            }
                        });
                }
            });

            ui.add_space(12.0);
            Self::bounded_panel(ui, 700.0, |ui| {
                ui.heading("Routing presets");
                ui.label("Higher-level route bundles for common personal setups. They send explicit SetRouter commands, then daemon state refreshes the matrix below.");
                ui.add_space(8.0);
                Self::polished_row(ui, egui::vec2(8.0, 8.0), |ui| {
                    for preset in RoutingPreset::daily_presets() {
                        Self::soft_bounded_panel(ui, 190.0, |ui| {
                            ui.set_min_height(112.0);
                            if ui.add(Self::accent_button(preset.name())).clicked() {
                                self.send(UiCommand::ApplyScene(UiScene::new(
                                    preset.name(),
                                    preset.commands(),
                                )));
                            }
                            ui.add_space(4.0);
                            ui.label(preset.description());
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} route commands",
                                    preset.commands().len()
                                ))
                                .monospace()
                                .small()
                                .color(Self::muted_text()),
                            );
                        });
                    }
                });
            });

            ui.add_space(12.0);
            Self::bounded_panel(ui, 720.0, |ui| {
                ui.heading("Routing matrix");
                ui.label("Web-UI style input-to-output routing. Active route state is read from the daemon snapshot; On / Off still send explicit router commands.");
                ui.add_space(8.0);
                egui::Grid::new("personal_routing_matrix")
                    .striped(true)
                    .spacing(egui::vec2(
                        RoutingMatrixLayoutPolicy::grid_column_gap(),
                        RoutingMatrixLayoutPolicy::grid_row_gap(),
                    ))
                    .show(ui, |ui| {
                        ui.label("");
                        for output in RoutingMatrixModel::outputs() {
                            ui.label(egui::RichText::new(routing_output_label(output)).strong());
                        }
                        ui.end_row();

                        for input in RoutingMatrixModel::inputs() {
                            ui.label(egui::RichText::new(routing_input_label(input)).strong());
                            for output in RoutingMatrixModel::outputs() {
                                let cell = RoutingMatrixModel::cell(input, output);
                                let route_state = self.snapshot.routing_enabled_for(input, output);
                                ui.allocate_ui_with_layout(
                                    egui::vec2(
                                        RoutingMatrixLayoutPolicy::cell_width(),
                                        RoutingMatrixLayoutPolicy::cell_height(),
                                    ),
                                    egui::Layout::top_down(egui::Align::Center),
                                    |ui| {
                                        ui.set_min_width(RoutingMatrixLayoutPolicy::cell_width());
                                        ui.set_max_width(RoutingMatrixLayoutPolicy::cell_width());
                                        let badge = self.snapshot.routing_state_badge(input, output);
                                        egui::Frame::new()
                                            .fill(badge.fill())
                                            .stroke(egui::Stroke::new(1.0, badge.stroke()))
                                            .corner_radius(egui::CornerRadius::same(3))
                                            .inner_margin(egui::Margin::symmetric(4, 1))
                                            .show(ui, |ui| {
                                                ui.set_min_width(RoutingMatrixLayoutPolicy::badge_width());
                                                ui.set_max_width(RoutingMatrixLayoutPolicy::badge_width());
                                                ui.set_min_height(RoutingMatrixLayoutPolicy::badge_height());
                                                ui.set_max_height(RoutingMatrixLayoutPolicy::badge_height());
                                                ui.horizontal_centered(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(badge.label())
                                                            .monospace()
                                                            .size(RoutingMatrixLayoutPolicy::badge_text_size())
                                                            .strong()
                                                            .color(badge.text()),
                                                    );
                                                });
                                            })
                                            .response
                                            .on_hover_text(format!(
                                                "{} → {} is {} in the latest daemon snapshot",
                                                cell.input_label(),
                                                cell.output_label(),
                                                badge.label()
                                            ));
                                        ui.horizontal(|ui| {
                                            if ui
                                                .add_sized(
                                                    egui::vec2(
                                                        RoutingMatrixLayoutPolicy::button_width(),
                                                        RoutingMatrixLayoutPolicy::button_height(),
                                                    ),
                                                    egui::Button::selectable(
                                                        route_state == Some(true),
                                                        "On",
                                                    ),
                                                )
                                                .clicked()
                                            {
                                                self.send(UiCommand::Send(cell.command_for_enabled(true)));
                                            }
                                            if ui
                                                .add_sized(
                                                    egui::vec2(
                                                        RoutingMatrixLayoutPolicy::button_width(),
                                                        RoutingMatrixLayoutPolicy::button_height(),
                                                    ),
                                                    egui::Button::selectable(
                                                        route_state == Some(false),
                                                        "Off",
                                                    ),
                                                )
                                                .clicked()
                                            {
                                                self.send(UiCommand::Send(cell.command_for_enabled(false)));
                                            }
                                        });
                                    },
                                );
                            }
                            ui.end_row();
                        }
                    });
            });

            ui.add_space(12.0);
            Self::bounded_panel(ui, 520.0, |ui| {
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
                        .add_sized(
                            egui::vec2(ContentLayoutPolicy::slider_width(), 20.0),
                            egui::Slider::new(&mut value, 0..=100).text(label),
                        )
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
                    self.send(UiCommand::Send(PersonalCommand::SetHeadphoneEqEnabled(eq_enabled)));
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
                    });
                });
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
    let candidates = vec![ipc_socket_path()];

    #[cfg(target_family = "unix")]
    {
        let mut candidates = candidates;
        let fallback = "/tmp/goxlr.socket".to_string();
        if !candidates.contains(&fallback) {
            candidates.push(fallback);
        }
        candidates
    }

    #[cfg(not(target_family = "unix"))]
    {
        candidates
    }
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
                            && let Err(error) = move_audio_stream(stream_id, &sink_name)
                        {
                            let _ = events.send(WorkerEvent::Error(error.to_string()));
                        }
                    }
                    snapshot.active_audio_streams = streams;
                }
                Err(error) => snapshot.active_audio_error = Some(error.to_string()),
            }
            let _ = events.send(WorkerEvent::Snapshot(Box::new(snapshot)));
        }
        Err(error) => {
            let _ = events.send(WorkerEvent::Error(error.to_string()));
        }
    }
}

fn active_serial(status: &DaemonStatus, selected_serial: Option<&str>) -> Result<String> {
    if let Some(selected) = selected_serial
        && status.mixers.contains_key(selected)
    {
        return Ok(selected.to_string());
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
        Box::new(move |creation_context| {
            PersonalUiApp::apply_goxlr_style(&creation_context.egui_ctx);
            Ok(Box::new(PersonalUiApp::new(command_tx.clone(), event_rx)))
        }),
    )
}
