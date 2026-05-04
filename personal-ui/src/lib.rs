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
use goxlr_ipc::{DaemonRequest, DaemonResponse, DaemonStatus, GoXLRCommand, ipc_socket_path};
use goxlr_types::{
    AnimationMode, Button, ButtonColourGroups, ButtonColourOffStyle, ChannelName,
    CompressorAttackTime, CompressorRatio, CompressorReleaseTime, DeviceType, EchoStyle,
    EffectBankPresets, EncoderColourTargets, EqFrequencies, FaderDisplayStyle, FaderName,
    GateTimes, GenderStyle, HardTuneSource, HardTuneStyle, InputDevice, MegaphoneStyle,
    MicrophoneType, MiniEqFrequencies, OutputDevice, PitchStyle, ReverbStyle, RobotRange,
    RobotStyle, SampleBank, SampleButtons, SamplePlayOrder, SamplePlaybackMode,
    SamplerColourTargets, SimpleColourTargets, VodMode, WaterfallDirection,
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
            && Self::main_content_horizontal_scroll_enabled()
    }

    pub fn main_content_vertical_scroll_enabled() -> bool {
        true
    }

    pub fn main_content_horizontal_scroll_enabled() -> bool {
        true
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemSettingsAction {
    label: &'static str,
    description: &'static str,
    command: PersonalCommand,
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
pub struct EffectsAdvancedControl {
    label: &'static str,
    default_command: PersonalCommand,
}

impl EffectsAdvancedControl {
    pub fn daily_controls() -> Vec<Self> {
        vec![
            Self::new("Reverb decay", PersonalCommand::SetReverbDecay(1500)),
            Self::new("Echo feedback", PersonalCommand::SetEchoFeedback(35)),
            Self::new("Pitch character", PersonalCommand::SetPitchCharacter(50)),
            Self::new(
                "Megaphone post gain",
                PersonalCommand::SetMegaphonePostGain(0),
            ),
            Self::new("Robot threshold", PersonalCommand::SetRobotThreshold(-40)),
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
        720.0
    }
    pub fn uses_guarded_profile_actions() -> bool {
        false
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
    pub fn exposes_file_import_controls() -> bool {
        false
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixerLayoutPolicy;

impl MixerLayoutPolicy {
    pub fn panel_width() -> f32 {
        560.0
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

    pub fn uses_wrapped_dashboard_panels() -> bool {
        true
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
    SetVCMuteAlsoMuteCM(bool),
    SetMonitorWithFx(bool),
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
    PlayNextSample(SampleBank, SampleButtons),
    StopSamplePlayback(SampleBank, SampleButtons),
    SaveMicProfile,
    ReloadSettings,
}

impl From<PersonalCommand> for GoXLRCommand {
    fn from(value: PersonalCommand) -> Self {
        match value {
            PersonalCommand::SetVolume(channel, volume) => GoXLRCommand::SetVolume(channel, volume),
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
            PersonalCommand::SetVCMuteAlsoMuteCM(enabled) => {
                GoXLRCommand::SetVCMuteAlsoMuteCM(enabled)
            }
            PersonalCommand::SetMonitorWithFx(enabled) => GoXLRCommand::SetMonitorWithFx(enabled),
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
            PersonalCommand::PlayNextSample(bank, button) => {
                GoXLRCommand::PlayNextSample(bank, button)
            }
            PersonalCommand::StopSamplePlayback(bank, button) => {
                GoXLRCommand::StopSamplePlayback(bank, button)
            }
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
    pub routing_matrix_routes: Vec<RoutingMatrixRoute>,
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
            routing_matrix_routes: Vec::new(),
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
            AppViewMode::Mic
            | AppViewMode::Effects
            | AppViewMode::Lighting
            | AppViewMode::HeadphoneEq
            | AppViewMode::Sampler
            | AppViewMode::System
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
    pending_mic_profile_confirmation: Option<PersonalCommand>,
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
            pending_mic_profile_confirmation: None,
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
        Self::bounded_panel(ui, 245.0, |ui| {
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
        Self::bounded_panel(ui, 320.0, |ui| {
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
        Self::bounded_panel(ui, 320.0, |ui| {
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
            ui.set_min_width(MixerLayoutPolicy::channel_strip_width());
            ui.set_max_width(MixerLayoutPolicy::channel_strip_width());
            ui.set_min_height(MixerLayoutPolicy::channel_strip_height());
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(label.to_uppercase())
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
                                egui::RichText::new("ANIMATION")
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
                                egui::RichText::new("SIMPLE COLOURS")
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
                            egui::RichText::new("FADERS")
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
                            egui::RichText::new("BUTTONS")
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
                            egui::RichText::new("ENCODERS / SAMPLER")
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
                                {
                                    if ui
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
                                }
                            });
                        }
                    });
                });
            },
        );
    }

    fn render_system_page(&mut self, ui: &mut egui::Ui) {
        Self::section_header(
            ui,
            "System",
            "Daily device settings without destructive profile actions",
            "Quick controls for mute timing, monitoring, fader lock, VOD mode, and reloading settings. Profile create/delete workflows stay out of this first pass until they have stronger guardrails.",
        );
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
                        egui::RichText::new("AMOUNTS")
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
                        egui::RichText::new("STYLES")
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
                        egui::RichText::new("ADVANCED DSP")
                            .monospace()
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.label(
                        "Deeper DSP quick defaults for reverb, echo, pitch, robot, and hard tune.",
                    );
                    ui.add_space(8.0);
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
                            .pending_mic_profile_confirmation
                            .as_ref()
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
                                self.pending_mic_profile_confirmation = None;
                                self.send(UiCommand::Send(save_profile_command));
                            } else {
                                self.pending_mic_profile_confirmation = Some(save_profile_command);
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
                        egui::RichText::new("COMPRESSOR / SAFETY")
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
                        egui::RichText::new("MIC EQ")
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
                        egui::RichText::new("MIC PROFILES")
                            .monospace()
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.label("Guarded profile actions use an explicit named slot before destructive commands.");
                    for action in MicProfileAction::guarded_daily_actions("Personal") {
                        let confirmed = self
                            .pending_mic_profile_confirmation
                            .as_ref()
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
                                self.pending_mic_profile_confirmation = None;
                                self.send(UiCommand::Send(command));
                            } else {
                                self.pending_mic_profile_confirmation = Some(action.command());
                            }
                        }
                    }
                    if self.pending_mic_profile_confirmation.is_some() {
                        ui.label(
                            egui::RichText::new(
                                "A guarded profile action is armed; click the same action again to send it.",
                            )
                            .small(),
                        );
                    }
                });
            },
        );
    }

    fn render_headphone_eq_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        Self::section_header(
            ui,
            "Headphone EQ",
            "Ten-band headphone equalizer controls",
            "Enable EQ, adjust preamp, and nudge each band through typed gain/frequency/Q commands.",
        );
        ui.add_space(12.0);
        Self::polished_row(
            ui,
            egui::vec2(
                ContentLayoutPolicy::desktop_panel_gap(),
                ContentLayoutPolicy::desktop_panel_gap(),
            ),
            |ui| {
                Self::bounded_panel(ui, HeadphoneEqLayoutPolicy::panel_width(), |ui| {
                    ui.label(
                        egui::RichText::new("HEADPHONE EQ")
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
                    ui.horizontal_wrapped(|ui| {
                        for band in HeadphoneEqBandControl::ten_band_editor() {
                            Self::soft_bounded_panel(ui, 112.0, |ui| {
                                ui.label(egui::RichText::new(band.label()).monospace().strong());
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
                            });
                        }
                    });
                });
            },
        );
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
        Self::polished_row(
            ui,
            egui::vec2(
                ContentLayoutPolicy::desktop_panel_gap(),
                ContentLayoutPolicy::desktop_panel_gap(),
            ),
            |ui| {
                Self::bounded_panel(ui, SamplerLayoutPolicy::panel_width(), |ui| {
                    ui.label(
                        egui::RichText::new("WORKFLOW SETTINGS")
                            .monospace()
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.label(
                        "Safe global sampler actions; sample file import/removal remains deferred.",
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
                for bank in [SampleBank::A, SampleBank::B, SampleBank::C] {
                    Self::bounded_panel(ui, SamplerLayoutPolicy::panel_width(), |ui| {
                        ui.label(
                            egui::RichText::new(format!("BANK {bank:?}"))
                                .monospace()
                                .size(18.0)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );
                        for button in [
                            SampleButtons::TopLeft,
                            SampleButtons::TopRight,
                            SampleButtons::BottomLeft,
                            SampleButtons::BottomRight,
                        ] {
                            ui.label(
                                egui::RichText::new(format!("{button:?}"))
                                    .monospace()
                                    .strong(),
                            );
                            ui.horizontal_wrapped(|ui| {
                                for action in SamplerAction::daily_bank_actions(bank, button) {
                                    if ui
                                        .add_sized(
                                            egui::vec2(
                                                ContentLayoutPolicy::min_action_button_width(),
                                                22.0,
                                            ),
                                            egui::Button::new(action.label()).small(),
                                        )
                                        .clicked()
                                    {
                                        self.send(UiCommand::Send(action.command()));
                                    }
                                }
                            });
                            ui.horizontal_wrapped(|ui| {
                                for action in SampleTrimAction::safe_trim_actions(bank, button, 0) {
                                    if ui
                                        .add_sized(
                                            egui::vec2(
                                                ContentLayoutPolicy::min_action_button_width(),
                                                22.0,
                                            ),
                                            egui::Button::new(action.label()).small(),
                                        )
                                        .on_hover_text("Reset sample slot 0 trim boundary without importing or removing files.")
                                        .clicked()
                                    {
                                        self.send(UiCommand::Send(action.command()));
                                    }
                                }
                            });
                        }
                    });
                }
            },
        );
    }

    fn render_mixer_dashboard(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        Self::section_header(
            ui,
            "Mixer",
            "Profiles, scenes, faders, and active app routing",
            "Daily controls are grouped into aligned cards so the dashboard reads left-to-right instead of floating in separate islands.",
        );
        ui.add_space(12.0);
        Self::polished_row(
            ui,
            egui::vec2(
                MixerLayoutPolicy::panel_gap(),
                MixerLayoutPolicy::panel_gap(),
            ),
            |ui| {
                self.render_scene_panel(ui);
                Self::bounded_panel(ui, MixerLayoutPolicy::panel_width(), |ui| {
                    ui.label(
                        egui::RichText::new("MIXER")
                            .monospace()
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
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
                        if ui
                            .add_sized(
                                egui::vec2(ContentLayoutPolicy::wide_action_button_width(), 34.0),
                                Self::accent_button("Enable ClipGuard"),
                            )
                            .clicked()
                        {
                            self.send(UiCommand::Send(PersonalCommand::SetClipGuardEnabled(true)));
                        }
                        if ui
                            .add_sized(
                                egui::vec2(ContentLayoutPolicy::wide_action_button_width(), 34.0),
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
                                egui::vec2(ContentLayoutPolicy::min_action_button_width(), 34.0),
                                Self::accent_button("Enable EQ"),
                            )
                            .clicked()
                        {
                            self.send(UiCommand::Send(PersonalCommand::SetHeadphoneEqEnabled(
                                true,
                            )));
                        }
                    });
                });
                ui.vertical(|ui| {
                    self.render_status_card(ui);
                    ui.add_space(12.0);
                    self.render_active_streams_panel(ui);
                });
            },
        );
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
                    (DashboardCopy::mixer_tab(), self.quick_actions.view_mode() == AppViewMode::QuickActions),
                    (DashboardCopy::configuration_tab(), self.quick_actions.view_mode() == AppViewMode::Full),
                    ("Routing", false),
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
                            "Lighting" => self.quick_actions.set_view_mode(AppViewMode::Lighting),
                            "Headphone EQ" => self.quick_actions.set_view_mode(AppViewMode::HeadphoneEq),
                            "Sampler" => self.quick_actions.set_view_mode(AppViewMode::Sampler),
                            "System" => self.quick_actions.set_view_mode(AppViewMode::System),
                            label if label == DashboardCopy::mixer_tab() => self.quick_actions.set_view_mode(AppViewMode::QuickActions),
                            label if label == DashboardCopy::configuration_tab() => self.quick_actions.set_view_mode(AppViewMode::Full),
                            _ => {}
                        }
                    }
                }
            });

            egui::ScrollArea::both()
                .id_salt(ContentLayoutPolicy::scroll_area_id())
                .auto_shrink([false, false])
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
