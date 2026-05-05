use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use goxlr_ipc::{Sample, SampleProcessState, Sampler, SamplerButton};
use goxlr_personal_ui::{
    AboutLayoutPolicy, ActiveAudioStreams, AppConfig, AppSceneConfig, AppSnapshot, AppViewMode,
    AudioRouteTarget, AudioRoutingRule, ContentLayoutPolicy, ControlledChannel, DashboardCopy,
    DeviceSelection, DiagnosticsLayoutPolicy, DiagnosticsLogEntry, DiagnosticsLogFilter,
    DiagnosticsStatusRow, DiagnosticsStatusSeverity, EffectPresetAction, EffectsAdvancedControl,
    EffectsAmountControl, EffectsLayoutPolicy, EffectsQuickPreset, EffectsStyleGroup,
    ExternalAudioTool, FaderAssignmentControl, FaderMuteFunctionControl, HardwareScribbleControl,
    HeadphoneEqBandControl, HeadphoneEqLayoutPolicy, HeadphoneEqProfileAction,
    ImplementedParityItem, LightingAnimationControl, LightingButtonColourTarget,
    LightingFaderColourTarget, LightingLayoutPolicy, LightingProfileAction, LightingQuickTheme,
    LightingSimpleColourTarget, LightingTripleColourTarget, MainProfileAction, MicEqBandControl,
    MicLayoutPolicy, MicProfileAction, MicSetupGuideStep, MiniWindowMode, MixerLayoutPolicy,
    MonitorMixControl, OptionalBoolAction, PersonalCommand, PersonalPreset, ProfileBrowser,
    ProfileBrowserKind, QuickActions, RoutingMatrixLayoutPolicy, RoutingMatrixModel,
    RoutingMatrixRoute, RoutingPreset, RoutingRuleDiffStatus, RoutingRuleEditor, RoutingStateBadge,
    SampleTrimAction, SamplerAction, SamplerFileAction, SamplerLayoutPolicy, SamplerLoadedSample,
    SamplerSampleBrowser, SamplerSlotSnapshot, SamplerWorkflowSetting, SceneEditor,
    SubmixChannelControl, SubmixChannelSnapshot, SubmixOutputMixControl, SubmixOutputSnapshot,
    SystemLayoutPolicy, SystemSettingsAction, TrayAction, TrayMenuModel, UiCommand, UiScene,
    VolumeDebouncer, WindowAction, ipc_socket_path_candidates,
};
use goxlr_types::{
    AnimationMode, Button, ButtonColourGroups, ButtonColourOffStyle, ChannelName,
    CompressorAttackTime, CompressorRatio, CompressorReleaseTime, EchoStyle, EffectBankPresets,
    EncoderColourTargets, EqFrequencies, FaderDisplayStyle, FaderName, GateTimes, GenderStyle,
    HardTuneSource, HardTuneStyle, InputDevice, MegaphoneStyle, MicrophoneType, MiniEqFrequencies,
    Mix, MuteFunction, OutputDevice, PitchStyle, ReverbStyle, RobotRange, RobotStyle, SampleBank,
    SampleButtons, SamplePlayOrder, SamplePlaybackMode, SamplerColourTargets, SimpleColourTargets,
    VodMode, WaterfallDirection,
};

fn temp_scene_config_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("goxlr-personal-ui-{name}-{nonce}.json"))
}

fn temp_test_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("goxlr-personal-ui-{name}-{nonce}"));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn content_layout_policy_keeps_main_content_scrollable_below_fixed_header() {
    assert!(ContentLayoutPolicy::main_content_scroll_enabled());
    assert!(ContentLayoutPolicy::main_content_vertical_scroll_enabled());
    assert!(ContentLayoutPolicy::main_content_horizontal_scroll_enabled());
    assert_eq!(
        ContentLayoutPolicy::scroll_area_id(),
        "personal_ui_main_content_scroll"
    );
    assert!(ContentLayoutPolicy::bounded_panel_allocates_before_frame());
    assert!(ContentLayoutPolicy::bounded_panel_avoids_sentinel_height_allocation());
    assert_eq!(ContentLayoutPolicy::bounded_panel_outer_min_height(), 0.0);
    assert!(ContentLayoutPolicy::min_action_button_width() >= 112.0);
    assert!(ContentLayoutPolicy::wide_action_button_width() >= 140.0);
    assert!(ContentLayoutPolicy::slider_width() <= 190.0);
    assert!(ContentLayoutPolicy::max_content_width() >= 1280.0);
    assert!(ContentLayoutPolicy::max_content_width() <= 1320.0);
    assert!(ContentLayoutPolicy::desktop_panel_gap() >= 12.0);
    assert!(ContentLayoutPolicy::wrapped_rows_top_align());
    assert!(ContentLayoutPolicy::section_header_width() < ContentLayoutPolicy::max_content_width());
    assert!(ContentLayoutPolicy::page_body_centers_in_wide_windows());
    assert_eq!(
        ContentLayoutPolicy::content_width_for_available_width(3440.0),
        ContentLayoutPolicy::max_content_width()
    );
    assert!(ContentLayoutPolicy::wide_window_side_margin(3440.0) > 1000.0);
}

#[test]
fn diagnostics_layout_policy_keeps_status_page_read_only_and_compact() {
    assert!(DiagnosticsLayoutPolicy::uses_read_only_status_cards());
    assert!(DiagnosticsLayoutPolicy::panel_width() >= 420.0);
    assert!(DiagnosticsLayoutPolicy::panel_width() <= 520.0);
    assert!(
        DiagnosticsLayoutPolicy::detail_panel_width() >= DiagnosticsLayoutPolicy::panel_width()
    );
    assert!(DiagnosticsLayoutPolicy::button_width() >= 120.0);
    assert!(DiagnosticsLayoutPolicy::shows_ipc_socket_candidates());
}

#[test]
fn app_snapshot_diagnostics_rows_explain_connection_device_and_profiles() {
    let disconnected = AppSnapshot::disconnected("unit test socket refused");
    let rows = disconnected.diagnostics_rows();
    assert!(rows.iter().any(|row| {
        row.label() == "Connection"
            && row.value().contains("Disconnected")
            && row.severity() == DiagnosticsStatusSeverity::Warning
    }));
    assert!(rows.iter().any(|row| {
        row.label() == "Daemon"
            && row.value() == "Unknown"
            && row.severity() == DiagnosticsStatusSeverity::Warning
    }));

    let connected = AppSnapshot {
        connected: true,
        error: None,
        daemon_version: Some("1.2.3".to_string()),
        device_serials: vec!["SERIAL-2".to_string(), "SERIAL-1".to_string()],
        device_serial: Some("SERIAL-1".to_string()),
        device_type: Some("GoXLR".to_string()),
        profile_name: Some("Personal".to_string()),
        mic_profile_name: Some("Broadcast".to_string()),
        headphone_eq_profile: Some("Personal Phones".to_string()),
        ..AppSnapshot::disconnected("unused")
    };
    let rows = connected.diagnostics_rows();
    assert!(rows.iter().any(|row| {
        row.label() == "Connection"
            && row.value() == "Connected"
            && row.severity() == DiagnosticsStatusSeverity::Ok
    }));
    assert!(
        rows.iter()
            .any(|row| row.label() == "Daemon" && row.value() == "1.2.3")
    );
    assert!(rows.iter().any(|row| {
        row.label() == "Device" && row.value().contains("GoXLR") && row.value().contains("SERIAL-1")
    }));
    assert!(
        rows.iter()
            .any(|row| row.label() == "Profiles" && row.value().contains("Personal"))
    );
    assert!(
        rows.iter()
            .any(|row| row.label() == "Mic profile" && row.value() == "Broadcast")
    );
    assert!(
        rows.iter()
            .any(|row| row.label() == "Headphone EQ" && row.value() == "Personal Phones")
    );
    assert!(
        rows.iter()
            .any(|row| row.label() == "Detected devices" && row.value() == "2")
    );
}

#[test]
fn diagnostics_status_rows_have_stable_labels_and_severity() {
    let row = DiagnosticsStatusRow::new(
        "IPC socket",
        "/tmp/goxlr.socket",
        DiagnosticsStatusSeverity::Info,
    );
    assert_eq!(row.label(), "IPC socket");
    assert_eq!(row.value(), "/tmp/goxlr.socket");
    assert_eq!(row.severity(), DiagnosticsStatusSeverity::Info);
    assert_eq!(DiagnosticsStatusSeverity::Ok.label(), "OK");
    assert_eq!(DiagnosticsStatusSeverity::Warning.label(), "Warning");
    assert_eq!(DiagnosticsStatusSeverity::Info.label(), "Info");
}

#[test]
fn diagnostics_log_entries_filter_recent_events_without_dispatching_commands() {
    let entries = vec![
        DiagnosticsLogEntry::new(
            "12:00:00",
            DiagnosticsStatusSeverity::Info,
            "Snapshot",
            "connected to daemon",
        ),
        DiagnosticsLogEntry::new(
            "12:00:01",
            DiagnosticsStatusSeverity::Warning,
            "IPC error",
            "socket refused",
        ),
        DiagnosticsLogEntry::new(
            "12:00:02",
            DiagnosticsStatusSeverity::Info,
            "Command",
            "Refresh status",
        ),
    ];

    let warnings = DiagnosticsLogEntry::filtered_rows(&entries, DiagnosticsLogFilter::WarningsOnly);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].category(), "IPC error");
    assert_eq!(warnings[0].message(), "socket refused");

    let all_recent = DiagnosticsLogEntry::recent_rows(&entries, 2, DiagnosticsLogFilter::All);
    assert_eq!(all_recent.len(), 2);
    assert_eq!(all_recent[0].category(), "IPC error");
    assert_eq!(all_recent[1].category(), "Command");
    assert!(all_recent.iter().all(|entry| entry.is_read_only()));
}

#[test]
fn diagnostics_layout_policy_exposes_read_only_log_viewer() {
    assert!(DiagnosticsLayoutPolicy::shows_read_only_log_viewer());
    assert!(
        DiagnosticsLayoutPolicy::log_panel_width() >= DiagnosticsLayoutPolicy::detail_panel_width()
    );
    assert!(DiagnosticsLayoutPolicy::log_row_height() >= 42.0);
    assert!(DiagnosticsLayoutPolicy::log_row_limit() >= 8);
}

#[test]
fn diagnostics_view_mode_is_available_from_quick_actions() {
    let mut quick = QuickActions::default();
    quick.set_view_mode(AppViewMode::Diagnostics);
    assert_eq!(quick.view_mode(), AppViewMode::Diagnostics);
    quick.toggle_view_mode();
    assert_eq!(quick.view_mode(), AppViewMode::QuickActions);
}

#[test]
fn about_view_mode_exposes_implemented_parity_summary() {
    let mut quick = QuickActions::default();
    quick.set_view_mode(AppViewMode::About);
    assert_eq!(quick.view_mode(), AppViewMode::About);
    quick.toggle_view_mode();
    assert_eq!(quick.view_mode(), AppViewMode::QuickActions);

    assert!(AboutLayoutPolicy::uses_read_only_summary_cards());
    assert!(AboutLayoutPolicy::panel_width() >= 420.0);
    assert!(AboutLayoutPolicy::panel_width() <= 560.0);

    let items = ImplementedParityItem::current_items();
    assert!(items.len() >= 8);
    assert!(
        items
            .iter()
            .any(|item| item.label() == "Mixer" && item.status_label() == "Implemented")
    );
    assert!(
        items
            .iter()
            .any(|item| item.label() == "Sampler" && item.status_label() == "Partial")
    );
    assert!(
        items
            .iter()
            .any(|item| item.label() == "Profiles" && item.description().contains("guarded"))
    );
    assert!(
        items
            .iter()
            .all(|item| !item.label().is_empty() && !item.description().is_empty())
    );
}

#[test]
fn routing_matrix_model_covers_web_ui_input_output_router() {
    assert_eq!(
        RoutingMatrixModel::inputs(),
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
    );
    assert_eq!(
        RoutingMatrixModel::outputs(),
        vec![
            OutputDevice::Headphones,
            OutputDevice::BroadcastMix,
            OutputDevice::ChatMic,
            OutputDevice::Sampler,
            OutputDevice::LineOut,
        ]
    );
    assert_eq!(
        RoutingMatrixModel::cells().len(),
        RoutingMatrixModel::inputs().len() * RoutingMatrixModel::outputs().len()
    );

    let cell = RoutingMatrixModel::cell(InputDevice::Music, OutputDevice::BroadcastMix);
    assert_eq!(cell.input_label(), "Music");
    assert_eq!(cell.output_label(), "Broadcast");
    assert_eq!(
        cell.command_for_enabled(true),
        PersonalCommand::SetRouter(InputDevice::Music, OutputDevice::BroadcastMix, true)
    );
    assert_eq!(
        cell.command_for_enabled(false),
        PersonalCommand::SetRouter(InputDevice::Music, OutputDevice::BroadcastMix, false)
    );
}

#[test]
fn routing_matrix_snapshot_exposes_live_route_state_for_indicators() {
    let mut snapshot = AppSnapshot::disconnected("unit test");
    snapshot.routing_matrix_routes = vec![
        RoutingMatrixRoute::new(InputDevice::Music, OutputDevice::BroadcastMix, true),
        RoutingMatrixRoute::new(InputDevice::Microphone, OutputDevice::ChatMic, false),
    ];

    assert_eq!(
        snapshot.routing_enabled_for(InputDevice::Music, OutputDevice::BroadcastMix),
        Some(true)
    );
    assert_eq!(
        snapshot.routing_enabled_for(InputDevice::Microphone, OutputDevice::ChatMic),
        Some(false)
    );
    assert_eq!(
        snapshot.routing_enabled_for(InputDevice::Game, OutputDevice::LineOut),
        None
    );
    assert_eq!(
        snapshot.routing_state_label(InputDevice::Music, OutputDevice::BroadcastMix),
        "Active"
    );
    assert_eq!(
        snapshot.routing_state_label(InputDevice::Microphone, OutputDevice::ChatMic),
        "Off"
    );
    assert_eq!(
        snapshot.routing_state_label(InputDevice::Game, OutputDevice::LineOut),
        "Unknown"
    );
}

#[test]
fn routing_state_badges_are_compact_centered_state_pills() {
    let active = RoutingStateBadge::for_state(Some(true));
    let off = RoutingStateBadge::for_state(Some(false));
    let unknown = RoutingStateBadge::for_state(None);

    assert_eq!(active.label(), "Active");
    assert_eq!(off.label(), "Off");
    assert_eq!(unknown.label(), "Unknown");
    assert!(RoutingStateBadge::min_width() >= 48.0);
    assert!(RoutingStateBadge::min_width() <= 56.0);
    assert_eq!(RoutingMatrixLayoutPolicy::cell_width(), 74.0);
    assert_eq!(RoutingMatrixLayoutPolicy::cell_height(), 40.0);
    assert_eq!(RoutingMatrixLayoutPolicy::badge_width(), 50.0);
    assert_eq!(RoutingMatrixLayoutPolicy::badge_height(), 15.0);
    assert_eq!(RoutingMatrixLayoutPolicy::button_width(), 28.0);
    assert_eq!(RoutingMatrixLayoutPolicy::button_height(), 15.0);
    assert_eq!(RoutingMatrixLayoutPolicy::grid_column_gap(), 4.0);
    assert_eq!(RoutingMatrixLayoutPolicy::grid_row_gap(), 1.0);
    assert_eq!(RoutingMatrixLayoutPolicy::badge_text_size(), 9.0);
    assert!(RoutingMatrixLayoutPolicy::cell_height() < 44.0);
    assert!(
        RoutingMatrixLayoutPolicy::button_height() <= RoutingMatrixLayoutPolicy::badge_height()
    );
    assert!(
        RoutingMatrixLayoutPolicy::badge_height() < RoutingMatrixLayoutPolicy::cell_height() / 2.0
    );
    assert!(
        RoutingMatrixLayoutPolicy::cell_height() <= RoutingMatrixLayoutPolicy::cell_width() * 0.55
    );
    assert!(!RoutingMatrixLayoutPolicy::badge_uses_available_height());
    assert!(RoutingMatrixLayoutPolicy::uses_compact_action_labels());
    assert!(RoutingMatrixLayoutPolicy::matrix_width_for_model() < 460.0);
    assert_ne!(active.fill(), off.fill());
    assert_ne!(active.stroke(), off.stroke());
    assert_ne!(unknown.stroke(), off.stroke());
    assert_ne!(active.text(), off.text());
}

#[test]
fn personal_command_maps_routing_matrix_to_backend_router_command() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetRouter(
            InputDevice::Game,
            OutputDevice::Headphones,
            true
        )),
        goxlr_ipc::GoXLRCommand::SetRouter(InputDevice::Game, OutputDevice::Headphones, true)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetRouter(
            InputDevice::Microphone,
            OutputDevice::ChatMic,
            false
        )),
        goxlr_ipc::GoXLRCommand::SetRouter(InputDevice::Microphone, OutputDevice::ChatMic, false)
    ));
}

#[test]
fn lighting_layout_policy_uses_cards_and_wrapping_panels_for_dense_editor() {
    assert_eq!(LightingLayoutPolicy::quick_theme_target_columns(), 4);
    assert_eq!(LightingLayoutPolicy::quick_theme_card_width(), 150.0);
    assert_eq!(LightingLayoutPolicy::quick_theme_card_height(), 116.0);
    assert_eq!(LightingLayoutPolicy::animation_control_grid_columns(), 2);
    assert!(LightingLayoutPolicy::compact_editor_panel_width() < 430.0);
    assert!(LightingLayoutPolicy::wide_editor_panel_width() < 520.0);
    assert!(LightingLayoutPolicy::wide_editor_panel_width() < 400.0);
    assert_eq!(LightingLayoutPolicy::panel_gap(), 8.0);
    assert!(
        LightingLayoutPolicy::quick_theme_card_width_for_available_width(760.0)
            <= LightingLayoutPolicy::quick_theme_card_width()
    );
    assert!(
        LightingLayoutPolicy::quick_theme_card_width_for_available_width(760.0)
            * (LightingLayoutPolicy::quick_theme_target_columns() as f32)
            < 680.0
    );
    assert!(LightingLayoutPolicy::uses_dense_editor_flow());
    assert!(LightingLayoutPolicy::theme_row_stays_compact_in_wide_windows());
    assert_eq!(LightingLayoutPolicy::editor_intro_width(), 960.0);
    assert!(LightingLayoutPolicy::wide_editor_panel_width() <= 360.0);
    assert!(
        LightingLayoutPolicy::balanced_editor_row_width()
            <= ContentLayoutPolicy::max_content_width()
    );
}

#[test]
fn effects_layout_policy_uses_wrapped_preset_cards_instead_of_skinny_grid() {
    assert_eq!(EffectsLayoutPolicy::quick_preset_target_columns(), 4);
    assert_eq!(EffectsLayoutPolicy::quick_preset_card_width(), 180.0);
    assert_eq!(EffectsLayoutPolicy::quick_preset_card_height(), 112.0);
    assert_eq!(EffectsLayoutPolicy::quick_preset_inner_height(), 88.0);
    assert_eq!(
        EffectsLayoutPolicy::quick_preset_row_cross_align(),
        egui::Align::Min
    );
    assert!(EffectsLayoutPolicy::quick_preset_cards_share_height());
    assert_eq!(EffectsLayoutPolicy::detail_panel_gap(), 8.0);
    assert!(EffectsLayoutPolicy::amount_panel_width() <= 360.0);
    assert!(EffectsLayoutPolicy::style_panel_width() <= 720.0);
    assert!(
        EffectsLayoutPolicy::quick_preset_inner_width()
            >= EffectsLayoutPolicy::quick_preset_command_label_min_width() * 2.0
    );
    assert_eq!(
        EffectsLayoutPolicy::quick_preset_card_width_for_available_width(760.0),
        EffectsLayoutPolicy::quick_preset_card_width()
    );
    assert!(
        EffectsLayoutPolicy::quick_preset_card_width_for_available_width(700.0)
            < EffectsLayoutPolicy::quick_preset_card_width()
    );
    let row_width = EffectsLayoutPolicy::quick_preset_card_width_for_available_width(760.0)
        * (EffectsLayoutPolicy::quick_preset_target_columns() as f32)
        + EffectsLayoutPolicy::detail_panel_gap()
            * ((EffectsLayoutPolicy::quick_preset_target_columns() - 1) as f32);
    assert!(row_width <= 760.0);
    assert_eq!(EffectsLayoutPolicy::style_panel_width(), 700.0);
    assert_eq!(EffectsLayoutPolicy::style_group_card_width(), 170.0);
    assert!(EffectsLayoutPolicy::style_button_min_width() >= 60.0);
}

#[test]
fn mixer_layout_policy_supports_fader_assignment_editor() {
    assert!(MicLayoutPolicy::uses_wrapped_panels());
    assert_eq!(MicLayoutPolicy::panel_width(), 360.0);
    assert_eq!(MicLayoutPolicy::panel_gap(), 8.0);
    assert!(MicLayoutPolicy::slider_width() <= 200.0);

    assert!(MixerLayoutPolicy::uses_wrapped_dashboard_panels());
    assert_eq!(MixerLayoutPolicy::panel_width(), 560.0);
    assert_eq!(MixerLayoutPolicy::channel_strip_width(), 94.0);
    assert_eq!(MixerLayoutPolicy::channel_strip_height(), 270.0);
    assert_eq!(MixerLayoutPolicy::channel_slider_height(), 190.0);
    assert!(MixerLayoutPolicy::uses_fader_assignment_editor());
    assert!(MixerLayoutPolicy::uses_compact_fader_assignment_cards());
    assert!(MixerLayoutPolicy::assignment_panel_width() > MixerLayoutPolicy::panel_width());
    assert_eq!(MixerLayoutPolicy::assignment_cards_per_row(), 2);
    assert!(
        MixerLayoutPolicy::assignment_card_width() * 2.0 + MixerLayoutPolicy::assignment_card_gap()
            <= MixerLayoutPolicy::assignment_panel_width()
    );
    assert!(MixerLayoutPolicy::assignment_button_width() < 100.0);
}

#[test]
fn fader_assignment_controls_expose_daily_channels_and_mute_targets() {
    let assignments = FaderAssignmentControl::daily_controls();
    assert_eq!(assignments.len(), 4);
    assert_eq!(assignments[0].fader(), FaderName::A);
    assert_eq!(assignments[0].label(), "Fader A");
    assert_eq!(assignments[0].default_channel(), ChannelName::Mic);
    assert!(
        assignments
            .iter()
            .all(|control| !control.label().is_empty() && !control.description().is_empty())
    );

    let channels = FaderAssignmentControl::daily_channels();
    assert_eq!(
        channels,
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
    );
    assert_eq!(
        assignments[1].assign_command(ChannelName::Music),
        PersonalCommand::SetFader(FaderName::B, ChannelName::Music)
    );

    let mute_controls = FaderMuteFunctionControl::daily_controls();
    assert_eq!(mute_controls.len(), 4);
    assert_eq!(mute_controls[2].fader(), FaderName::C);
    assert_eq!(mute_controls[2].default_function(), MuteFunction::ToStream);
    assert_eq!(
        FaderMuteFunctionControl::daily_functions(),
        vec![
            MuteFunction::All,
            MuteFunction::ToStream,
            MuteFunction::ToVoiceChat,
            MuteFunction::ToPhones,
        ]
    );
    assert_eq!(
        mute_controls[0].function_command(MuteFunction::ToVoiceChat),
        PersonalCommand::SetFaderMuteFunction(FaderName::A, MuteFunction::ToVoiceChat)
    );
}

#[test]
fn fader_assignment_commands_map_to_backend_commands() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(
            PersonalCommand::SetFader(FaderName::D, ChannelName::System,)
        ),
        goxlr_ipc::GoXLRCommand::SetFader(FaderName::D, ChannelName::System)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetFaderMuteFunction(
            FaderName::B,
            MuteFunction::ToStream,
        )),
        goxlr_ipc::GoXLRCommand::SetFaderMuteFunction(FaderName::B, MuteFunction::ToStream)
    ));
}

#[test]
fn monitor_mix_controls_expose_safe_output_selector() {
    assert!(MixerLayoutPolicy::uses_monitor_mix_selector());
    assert_eq!(MixerLayoutPolicy::monitor_mix_panel_width(), 520.0);
    assert_eq!(MixerLayoutPolicy::monitor_mix_button_width(), 118.0);

    let controls = MonitorMixControl::daily_controls();
    assert_eq!(controls.len(), 4);
    assert_eq!(
        controls
            .iter()
            .map(|control| control.output())
            .collect::<Vec<_>>(),
        vec![
            OutputDevice::Headphones,
            OutputDevice::BroadcastMix,
            OutputDevice::ChatMic,
            OutputDevice::LineOut,
        ]
    );
    assert_eq!(controls[0].label(), "Headphones");
    assert_eq!(
        controls[0].command(),
        PersonalCommand::SetMonitorMix(OutputDevice::Headphones)
    );
    assert!(
        controls
            .iter()
            .all(|control| !control.label().is_empty() && !control.description().is_empty())
    );
}

#[test]
fn monitor_mix_command_maps_to_backend_command() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetMonitorMix(OutputDevice::LineOut)),
        goxlr_ipc::GoXLRCommand::SetMonitorMix(OutputDevice::LineOut)
    ));
}

#[test]
fn submix_controls_expose_safe_daily_channel_actions() {
    assert!(MixerLayoutPolicy::uses_submix_controls());
    assert_eq!(MixerLayoutPolicy::submix_panel_width(), 640.0);
    assert_eq!(MixerLayoutPolicy::submix_button_width(), 96.0);

    let controls = SubmixChannelControl::daily_controls();
    assert_eq!(controls.len(), 8);
    assert_eq!(
        controls
            .iter()
            .map(|control| control.channel())
            .collect::<Vec<_>>(),
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
    );
    assert_eq!(controls[0].label(), "Mic");
    assert_eq!(controls[0].volume_presets(), vec![0, 50, 100]);
    assert_eq!(SubmixChannelControl::percent_to_raw_volume(0), 0);
    assert_eq!(SubmixChannelControl::percent_to_raw_volume(50), 127);
    assert_eq!(SubmixChannelControl::percent_to_raw_volume(75), 191);
    assert_eq!(SubmixChannelControl::percent_to_raw_volume(100), 255);
    assert_eq!(
        controls[0].volume_command(75),
        PersonalCommand::SetSubMixVolume(ChannelName::Mic, 191)
    );
    assert_eq!(
        controls[0].link_command(true),
        PersonalCommand::SetSubMixLinked(ChannelName::Mic, true)
    );
    assert!(
        controls
            .iter()
            .all(|control| !control.label().is_empty() && !control.description().is_empty())
    );
}

#[test]
fn live_submix_snapshots_expose_daemon_volume_link_and_output_mix_state() {
    let channels = vec![
        SubmixChannelSnapshot::new(ChannelName::Mic, "Mic", 127, true, 0.5),
        SubmixChannelSnapshot::new(ChannelName::Music, "Music", 255, false, 1.0),
    ];
    assert_eq!(channels[0].volume_percent(), 49);
    assert_eq!(channels[0].state_label(), "49% linked");
    assert_eq!(channels[1].volume_percent(), 100);
    assert_eq!(channels[1].state_label(), "100% unlinked");

    let outputs = vec![
        SubmixOutputSnapshot::new(OutputDevice::Headphones, "Headphones", Mix::A),
        SubmixOutputSnapshot::new(OutputDevice::BroadcastMix, "Broadcast", Mix::B),
    ];
    assert_eq!(outputs[0].state_label(), "Mix A");
    assert_eq!(outputs[1].state_label(), "Mix B");

    let mut snapshot = AppSnapshot::disconnected("offline");
    snapshot.submix_channels = channels;
    snapshot.submix_outputs = outputs;
    assert_eq!(
        snapshot.submix_channel_state(ChannelName::Mic).unwrap(),
        "49% linked"
    );
    assert_eq!(
        snapshot
            .submix_output_state(OutputDevice::BroadcastMix)
            .unwrap(),
        "Mix B"
    );
    assert_eq!(snapshot.submix_channel_state(ChannelName::Game), None);
}

#[test]
fn submix_output_controls_expose_safe_mix_routing() {
    let outputs = SubmixOutputMixControl::daily_controls();
    assert_eq!(outputs.len(), 4);
    assert_eq!(
        outputs
            .iter()
            .map(|control| control.output())
            .collect::<Vec<_>>(),
        vec![
            OutputDevice::Headphones,
            OutputDevice::BroadcastMix,
            OutputDevice::ChatMic,
            OutputDevice::LineOut,
        ]
    );
    assert_eq!(outputs[0].label(), "Headphones");
    assert_eq!(outputs[0].mixes(), vec![Mix::A, Mix::B]);
    assert_eq!(
        outputs[1].mix_command(Mix::B),
        PersonalCommand::SetSubMixOutputMix(OutputDevice::BroadcastMix, Mix::B)
    );
}

#[test]
fn submix_commands_map_to_backend_commands() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetSubMixEnabled(true)),
        goxlr_ipc::GoXLRCommand::SetSubMixEnabled(true)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetSubMixVolume(ChannelName::Music, 64)),
        goxlr_ipc::GoXLRCommand::SetSubMixVolume(ChannelName::Music, 64)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetSubMixLinked(ChannelName::Game, false)),
        goxlr_ipc::GoXLRCommand::SetSubMixLinked(ChannelName::Game, false)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetSubMixOutputMix(
            OutputDevice::LineOut,
            Mix::B
        )),
        goxlr_ipc::GoXLRCommand::SetSubMixOutputMix(OutputDevice::LineOut, Mix::B)
    ));
}

#[test]
fn hardware_scribble_controls_expose_safe_daily_fields() {
    assert!(MixerLayoutPolicy::uses_scribble_strip_editor());
    assert!(MixerLayoutPolicy::scribble_panel_width() <= MixerLayoutPolicy::panel_width());
    assert!(
        MixerLayoutPolicy::scribble_button_width()
            >= ContentLayoutPolicy::min_action_button_width()
    );

    let controls = HardwareScribbleControl::daily_controls();
    assert_eq!(controls.len(), 4);
    assert_eq!(controls[0].fader(), FaderName::A);
    assert_eq!(controls[0].label(), "Fader A scribble");
    assert_eq!(controls[0].default_text(), "Mic");
    assert_eq!(controls[0].default_number(), "1");
    assert!(
        controls
            .iter()
            .all(|control| !control.description().is_empty())
    );

    assert_eq!(
        HardwareScribbleControl::daily_icon_presets(),
        vec![
            None,
            Some("mic.png"),
            Some("music.png"),
            Some("person.png"),
            Some("scale.png")
        ]
    );
    assert_eq!(
        controls[2].text_command("Music"),
        PersonalCommand::SetScribbleText(FaderName::C, "Music".to_string())
    );
    assert_eq!(
        controls[2].number_command("3"),
        PersonalCommand::SetScribbleNumber(FaderName::C, "3".to_string())
    );
    assert_eq!(
        controls[1].icon_command(Some("music.png")),
        PersonalCommand::SetScribbleIcon(FaderName::B, Some("music.png".to_string()))
    );
    assert_eq!(
        controls[0].icon_command(None),
        PersonalCommand::SetScribbleIcon(FaderName::A, None)
    );
    assert_eq!(
        controls[3].invert_command(true),
        PersonalCommand::SetScribbleInvert(FaderName::D, true)
    );
}

#[test]
fn hardware_scribble_commands_map_to_backend_commands() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetScribbleIcon(FaderName::A, Some("mic.png".to_string()))),
        goxlr_ipc::GoXLRCommand::SetScribbleIcon(FaderName::A, Some(icon)) if icon == "mic.png"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetScribbleText(FaderName::B, "Chat".to_string())),
        goxlr_ipc::GoXLRCommand::SetScribbleText(FaderName::B, text) if text == "Chat"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetScribbleNumber(FaderName::C, "3".to_string())),
        goxlr_ipc::GoXLRCommand::SetScribbleNumber(FaderName::C, number) if number == "3"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetScribbleInvert(FaderName::D, true)),
        goxlr_ipc::GoXLRCommand::SetScribbleInvert(FaderName::D, true)
    ));
}

#[test]
fn routing_presets_provide_named_command_bundles_above_matrix() {
    let presets = RoutingPreset::daily_presets();
    assert_eq!(presets.len(), 3);
    assert_eq!(presets[0].name(), "Broadcast Mix");
    assert!(presets[0].description().contains("broadcast output"));
    assert!(presets[0].commands().contains(&PersonalCommand::SetRouter(
        InputDevice::Music,
        OutputDevice::BroadcastMix,
        true
    )));
    assert!(presets[1].commands().contains(&PersonalCommand::SetRouter(
        InputDevice::Microphone,
        OutputDevice::ChatMic,
        true
    )));
    assert!(presets[2].commands().contains(&PersonalCommand::SetRouter(
        InputDevice::Game,
        OutputDevice::LineOut,
        false
    )));
}

#[test]
fn active_audio_streams_parse_pactl_json_with_sink_labels_and_app_names() {
    let sinks_json = r#"
        [
          {"index": 88, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Speaker__sink", "description": "GoXLRMini System"},
          {"index": 84, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line2__sink", "description": "GoXLRMini Music"}
        ]
    "#;
    let inputs_json = r#"
        [
          {
            "index": 12,
            "sink": 88,
            "mute": false,
            "corked": false,
            "volume": {"front-left": {"value_percent": "55%"}, "front-right": {"value_percent": "55%"}},
            "properties": {"application.name": "Firefox", "media.name": "YouTube Music"}
          },
          {
            "index": 13,
            "sink": 84,
            "mute": true,
            "corked": true,
            "properties": {"media.name": "Unknown app stream"}
          }
        ]
    "#;

    let streams = ActiveAudioStreams::from_pactl_json(sinks_json, inputs_json).unwrap();

    assert_eq!(streams.summary(), "2 playback streams");
    assert_eq!(streams.streams[0].id, 12);
    assert_eq!(streams.streams[0].display_name, "Firefox — YouTube Music");
    assert_eq!(streams.streams[0].sink_label, "GoXLRMini System");
    assert_eq!(streams.streams[0].volume_percent.as_deref(), Some("55%"));
    assert!(!streams.streams[0].muted);
    assert!(!streams.streams[0].corked);
    assert_eq!(streams.streams[1].display_name, "Unknown app stream");
    assert_eq!(streams.streams[1].sink_label, "GoXLRMini Music");
    assert!(streams.streams[1].muted);
    assert!(streams.streams[1].corked);
}

#[test]
fn active_audio_streams_build_goxlr_route_targets_from_sink_names() {
    let sinks_json = r#"
        [
          {"index": 88, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Speaker__sink", "description": "GoXLRMini System"},
          {"index": 86, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line1__sink", "description": "GoXLRMini Game"},
          {"index": 84, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line2__sink", "description": "GoXLRMini Music"},
          {"index": 82, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Headphones__sink", "description": "GoXLRMini Chat"},
          {"index": 60, "name": "alsa_output.pci-0000_01_00.1.hdmi-stereo", "description": "HDMI"}
        ]
    "#;

    let streams = ActiveAudioStreams::from_pactl_json(sinks_json, "[]").unwrap();

    assert_eq!(
        streams.route_targets,
        vec![
            AudioRouteTarget::new(
                "System",
                "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Speaker__sink"
            ),
            AudioRouteTarget::new(
                "Game",
                "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line1__sink"
            ),
            AudioRouteTarget::new(
                "Music",
                "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line2__sink"
            ),
            AudioRouteTarget::new(
                "Chat",
                "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Headphones__sink"
            ),
        ]
    );
}

#[test]
fn ui_command_can_request_moving_audio_stream_to_sink() {
    assert_eq!(
        UiCommand::MoveAudioStream {
            stream_id: 12,
            sink_name: "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line2__sink".to_string(),
        },
        UiCommand::MoveAudioStream {
            stream_id: 12,
            sink_name: "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line2__sink".to_string(),
        }
    );
}

#[test]
fn ui_command_can_request_muting_and_unmuting_audio_stream() {
    assert_eq!(
        UiCommand::SetAudioStreamMute {
            stream_id: 12,
            muted: true,
        },
        UiCommand::SetAudioStreamMute {
            stream_id: 12,
            muted: true,
        }
    );
    assert_eq!(
        UiCommand::SetAudioStreamMute {
            stream_id: 12,
            muted: false,
        },
        UiCommand::SetAudioStreamMute {
            stream_id: 12,
            muted: false,
        }
    );
}

#[test]
fn audio_stream_exposes_numeric_volume_for_slider_controls() {
    let streams = ActiveAudioStreams::from_pactl_json(
        r#"[{"index":88,"name":"alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Speaker__sink","description":"GoXLRMini System"}]"#,
        r#"[{"index":12,"sink":88,"volume":{"front-left":{"value_percent":"73%"}},"properties":{"application.name":"Firefox"}}]"#,
    )
    .unwrap();

    assert_eq!(streams.streams[0].volume_percent_value(), Some(73));
}

#[test]
fn ui_command_can_request_per_stream_volume_changes() {
    assert_eq!(
        UiCommand::SetAudioStreamVolume {
            stream_id: 12,
            volume_percent: 67,
        },
        UiCommand::SetAudioStreamVolume {
            stream_id: 12,
            volume_percent: 67,
        }
    );
}

#[test]
fn external_audio_tool_model_lists_daily_routing_helpers() {
    assert_eq!(
        ExternalAudioTool::daily_helpers(),
        vec![ExternalAudioTool::Pavucontrol, ExternalAudioTool::Qpwgraph]
    );
    assert_eq!(ExternalAudioTool::Pavucontrol.label(), "Open pavucontrol");
    assert_eq!(ExternalAudioTool::Pavucontrol.command(), "pavucontrol");
    assert_eq!(ExternalAudioTool::Qpwgraph.label(), "Open qpwgraph");
    assert_eq!(ExternalAudioTool::Qpwgraph.command(), "qpwgraph");
}

#[test]
fn dashboard_copy_uses_clear_routing_and_configuration_labels() {
    assert_eq!(DashboardCopy::mixer_tab(), "Mixer");
    assert_eq!(DashboardCopy::configuration_tab(), "Config / Routing");
    assert_eq!(
        DashboardCopy::active_playback_heading(),
        "ACTIVE APPS / ROUTING"
    );
    assert_eq!(DashboardCopy::manual_route_label(), "Move now:");
    assert_eq!(DashboardCopy::persistent_route_label(), "Always route:");
}

#[test]
fn app_config_parses_persistent_audio_routing_rules() {
    let config = AppConfig::from_json_str(
        r#"
        {
          "scenes": [],
          "audio_routing_rules": [
            {"app": "Spotify", "route": "Music"},
            {"app": "Discord", "route": "Chat", "enabled": false}
          ]
        }
        "#,
    )
    .expect("routing rules should parse");

    assert_eq!(
        config.audio_routing_rules(),
        vec![
            AudioRoutingRule::new("Spotify", "Music"),
            AudioRoutingRule::disabled("Discord", "Chat"),
        ]
    );
}

#[test]
fn app_config_uses_default_audio_routing_rules_when_existing_config_omits_them() {
    let config = AppConfig::from_json_str(r#"{"scenes":[]}"#).unwrap();

    assert_eq!(
        config.audio_routing_rules(),
        vec![
            AudioRoutingRule::new("Spotify", "Music"),
            AudioRoutingRule::new("Discord", "Chat"),
        ]
    );
}

#[test]
fn active_stream_can_be_saved_as_persistent_routing_rule() {
    let path = temp_scene_config_path("save-stream-routing-rule");
    fs::write(
        &path,
        r#"{"scenes":[{"name":"Scene"}],"audio_routing_rules":[{"app":"Spotify","route":"Music"}]}"#,
    )
    .unwrap();
    let mut state = AppSceneConfig::load_or_default(path.clone());
    let streams = ActiveAudioStreams::from_pactl_json(
        r#"[
          {"index": 88, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Speaker__sink", "description": "GoXLRMini System"},
          {"index": 86, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line1__sink", "description": "GoXLRMini Game"}
        ]"#,
        r#"[
          {"index": 31, "sink": 88, "properties": {"application.name": "Firefox", "media.name": "YouTube Music"}}
        ]"#,
    )
    .unwrap();

    state
        .save_audio_routing_rule_for_stream(&streams.streams[0], "Game")
        .unwrap();

    let saved = AppConfig::from_json_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        saved.audio_routing_rules(),
        vec![
            AudioRoutingRule::new("Spotify", "Music"),
            AudioRoutingRule::new("Firefox", "Game"),
        ]
    );
    assert_eq!(
        state.config().audio_routing_rules(),
        saved.audio_routing_rules()
    );
    let _ = fs::remove_file(path);
}

#[test]
fn saving_scene_config_keeps_backup_of_previous_json() {
    let path = temp_scene_config_path("backup-before-save");
    let original =
        r#"{"scenes":[{"name":"Old"}],"audio_routing_rules":[{"app":"Spotify","route":"Music"}]}"#;
    fs::write(&path, original).unwrap();
    let mut state = AppSceneConfig::load_or_default(path.clone());

    state.save_config(AppConfig::default()).unwrap();

    let backup_path = state.backup_path();
    assert_eq!(fs::read_to_string(&backup_path).unwrap(), original);
    assert_ne!(fs::read_to_string(&path).unwrap(), original);
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(backup_path);
}

#[test]
fn routing_rule_editor_adds_edits_deletes_and_reorders_rules() {
    let config = AppConfig::from_json_str(
        r#"{"scenes":[],"audio_routing_rules":[{"app":"Spotify","route":"Music"},{"app":"Discord","route":"Chat"}]}"#,
    )
    .unwrap();
    let mut editor = RoutingRuleEditor::from_config(&config);

    assert_eq!(
        editor.rule_summaries(),
        vec!["Spotify -> Music", "Discord -> Chat"]
    );

    editor.set_selected_rule(0);
    editor.add_rule();
    assert_eq!(editor.selected_rule(), 1);
    editor.set_app("Firefox");
    editor.set_route("Music");
    editor.set_enabled(false);

    assert_eq!(
        editor.rule_summaries(),
        vec![
            "Spotify -> Music",
            "Firefox -> Music (disabled)",
            "Discord -> Chat"
        ]
    );

    editor.move_selected_rule_down();
    assert_eq!(
        editor.rule_summaries(),
        vec![
            "Spotify -> Music",
            "Discord -> Chat",
            "Firefox -> Music (disabled)"
        ]
    );
    assert_eq!(editor.selected_rule(), 2);

    editor.delete_selected_rule();
    assert_eq!(
        editor.rule_summaries(),
        vec!["Spotify -> Music", "Discord -> Chat"]
    );
}

#[test]
fn routing_rule_editor_saves_rules_to_config_file_and_updates_runtime_state() {
    let path = temp_scene_config_path("routing-editor-save");
    fs::write(
        &path,
        r#"{"scenes":[{"name":"Scene"}],"audio_routing_rules":[{"app":"Spotify","route":"Music"}]}"#,
    )
    .unwrap();
    let mut state = AppSceneConfig::load_or_default(path.clone());
    let mut editor = RoutingRuleEditor::from_config(state.config());

    editor.set_selected_rule(0);
    editor.set_app("Firefox");
    editor.set_route("Game");
    editor.set_enabled(false);
    editor.add_rule();
    editor.set_app("Discord");
    editor.set_route("Chat");
    editor.save_to(&mut state).unwrap();

    let saved = AppConfig::from_json_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        saved.audio_routing_rules(),
        vec![
            AudioRoutingRule::disabled("Firefox", "Game"),
            AudioRoutingRule::new("Discord", "Chat"),
        ]
    );
    assert_eq!(
        state.config().audio_routing_rules(),
        saved.audio_routing_rules()
    );
    let _ = fs::remove_file(path);
}

#[test]
fn active_audio_streams_plan_auto_route_moves_matching_apps_to_targets() {
    let streams = ActiveAudioStreams::from_pactl_json(
        r#"
        [
          {"index": 88, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Speaker__sink", "description": "GoXLRMini System"},
          {"index": 84, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line2__sink", "description": "GoXLRMini Music"},
          {"index": 82, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Headphones__sink", "description": "GoXLRMini Chat"}
        ]
        "#,
        r#"
        [
          {"index": 12, "sink": 88, "properties": {"application.name": "Spotify"}},
          {"index": 13, "sink": 84, "properties": {"application.name": "Discord"}}
        ]
        "#,
    )
    .unwrap();

    assert_eq!(
        streams.routing_moves(&[
            AudioRoutingRule::new("Spotify", "Music"),
            AudioRoutingRule::new("Discord", "Chat"),
        ]),
        vec![
            UiCommand::MoveAudioStream {
                stream_id: 12,
                sink_name: "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line2__sink".to_string(),
            },
            UiCommand::MoveAudioStream {
                stream_id: 13,
                sink_name: "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Headphones__sink"
                    .to_string(),
            },
        ]
    );
}

#[test]
fn active_audio_streams_do_not_auto_route_disabled_or_already_routed_rules() {
    let streams = ActiveAudioStreams::from_pactl_json(
        r#"
        [
          {"index": 84, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line2__sink", "description": "GoXLRMini Music"},
          {"index": 82, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Headphones__sink", "description": "GoXLRMini Chat"}
        ]
        "#,
        r#"
        [
          {"index": 12, "sink": 84, "properties": {"application.name": "Spotify"}},
          {"index": 13, "sink": 84, "properties": {"application.name": "Discord"}}
        ]
        "#,
    )
    .unwrap();

    assert!(
        streams
            .routing_moves(&[
                AudioRoutingRule::new("Spotify", "Music"),
                AudioRoutingRule::disabled("Discord", "Chat"),
            ])
            .is_empty()
    );
}

#[test]
fn active_audio_streams_report_empty_playback_clearly() {
    let streams = ActiveAudioStreams::from_pactl_json("[]", "[]").unwrap();

    assert!(streams.streams.is_empty());
    assert_eq!(streams.summary(), "No active playback streams");
}

#[test]
fn active_audio_streams_diff_persistent_rules_against_current_routes() {
    let streams = ActiveAudioStreams::from_pactl_json(
        r#"
        [
          {"index": 88, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Speaker__sink", "description": "GoXLRMini System"},
          {"index": 86, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line1__sink", "description": "GoXLRMini Game"},
          {"index": 84, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line2__sink", "description": "GoXLRMini Music"},
          {"index": 82, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Headphones__sink", "description": "GoXLRMini Chat"}
        ]
        "#,
        r#"
        [
          {"index": 12, "sink": 88, "properties": {"application.name": "Spotify"}},
          {"index": 13, "sink": 84, "properties": {"application.name": "Discord"}}
        ]
        "#,
    )
    .unwrap();

    let rows = streams.routing_rule_diffs(&[
        AudioRoutingRule::new("Spotify", "Music"),
        AudioRoutingRule::new("Discord", "Chat"),
        AudioRoutingRule::new("Firefox", "Game"),
        AudioRoutingRule::disabled("Steam", "Game"),
    ]);

    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].app(), "Spotify");
    assert_eq!(rows[0].desired_route(), "Music");
    assert_eq!(rows[0].current_route(), Some("System"));
    assert_eq!(rows[0].status(), RoutingRuleDiffStatus::NeedsMove);
    assert_eq!(rows[0].status_label(), "Move needed");
    assert_eq!(rows[0].summary(), "Spotify: System → Music");

    assert_eq!(rows[1].app(), "Discord");
    assert_eq!(rows[1].current_route(), Some("Music"));
    assert_eq!(rows[1].status(), RoutingRuleDiffStatus::NeedsMove);

    assert_eq!(rows[2].app(), "Firefox");
    assert_eq!(rows[2].current_route(), None);
    assert_eq!(rows[2].status(), RoutingRuleDiffStatus::WaitingForStream);
    assert_eq!(rows[2].status_label(), "Waiting");

    assert_eq!(rows[3].app(), "Steam");
    assert_eq!(rows[3].status(), RoutingRuleDiffStatus::Disabled);
    assert_eq!(rows[3].status_label(), "Disabled");
}

#[test]
fn active_audio_streams_diff_marks_matching_and_missing_targets() {
    let streams = ActiveAudioStreams::from_pactl_json(
        r#"
        [
          {"index": 84, "name": "alsa_output.usb-TC-Helicon_GoXLRMini-00.HiFi__Line2__sink", "description": "GoXLRMini Music"}
        ]
        "#,
        r#"
        [
          {"index": 12, "sink": 84, "properties": {"application.name": "Spotify"}}
        ]
        "#,
    )
    .unwrap();

    let rows = streams.routing_rule_diffs(&[
        AudioRoutingRule::new("Spotify", "Music"),
        AudioRoutingRule::new("OBS", "Broadcast"),
    ]);

    assert_eq!(rows[0].status(), RoutingRuleDiffStatus::Matched);
    assert_eq!(rows[0].status_label(), "Matched");
    assert_eq!(rows[0].summary(), "Spotify: Music ✓");

    assert_eq!(rows[1].status(), RoutingRuleDiffStatus::MissingTarget);
    assert_eq!(rows[1].status_label(), "No route target");
    assert_eq!(rows[1].summary(), "OBS: Broadcast target unavailable");
}

#[test]
fn ipc_socket_candidates_include_legacy_tmp_socket_for_installed_daemon() {
    let candidates = ipc_socket_path_candidates();

    assert!(!candidates.is_empty());
    assert!(
        candidates.iter().any(|path| path == "/tmp/goxlr.socket"),
        "expected legacy installed-daemon socket fallback in {candidates:?}"
    );
    assert_eq!(
        candidates
            .iter()
            .filter(|path| *path == "/tmp/goxlr.socket")
            .count(),
        1
    );
}

#[test]
fn channel_controls_cover_personal_mvp_channels() {
    let channels = ControlledChannel::mvp_channels();

    assert_eq!(
        channels
            .iter()
            .map(|channel| channel.channel)
            .collect::<Vec<_>>(),
        vec![
            ChannelName::Headphones,
            ChannelName::Music,
            ChannelName::Game,
            ChannelName::Chat,
        ]
    );
}

#[test]
fn safe_now_scene_builds_personal_safety_commands() {
    let commands = UiScene::safe_now().commands();

    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Music, 0)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Game, 0)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Chat, 0)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Headphones, 50)));
    assert!(commands.contains(&PersonalCommand::SetHeadphoneLimiterEnabled(true)));
    assert!(commands.contains(&PersonalCommand::SetClipGuardEnabled(true)));
}

#[test]
fn personal_scenes_are_available_in_button_order() {
    let names = UiScene::personal_scenes()
        .into_iter()
        .map(|scene| scene.name().to_string())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["Gaming", "Music", "Night", "Call", "Safe Now"]);
}

#[test]
fn named_personal_presets_bundle_routing_lighting_and_effects() {
    let presets = PersonalPreset::daily_presets();
    let names = presets
        .iter()
        .map(|preset| preset.name())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec!["Go Live", "Desktop Focus", "Late Night", "FX Panic"]
    );
    assert!(
        presets
            .iter()
            .all(|preset| !preset.description().is_empty() && preset.commands().len() >= 4)
    );

    let go_live = &presets[0];
    assert!(go_live.commands().contains(&PersonalCommand::SetRouter(
        InputDevice::Microphone,
        OutputDevice::BroadcastMix,
        true
    )));
    assert!(
        go_live
            .commands()
            .contains(&PersonalCommand::SetGlobalColour("FF1F1F".to_string()))
    );
    assert!(
        go_live
            .commands()
            .contains(&PersonalCommand::SetFXEnabled(false))
    );

    let desktop_focus = &presets[1];
    assert!(
        desktop_focus
            .commands()
            .contains(&PersonalCommand::SetRouter(
                InputDevice::Music,
                OutputDevice::Headphones,
                true
            ))
    );
    assert!(
        desktop_focus
            .commands()
            .contains(&PersonalCommand::SetGlobalColour("1F6FFF".to_string()))
    );

    let panic = presets.last().unwrap();
    assert!(panic.is_safety_preset());
    assert!(
        panic
            .commands()
            .contains(&PersonalCommand::SetFXEnabled(false))
    );
    assert!(
        panic
            .commands()
            .contains(&PersonalCommand::SetHardTuneEnabled(false))
    );
    assert!(
        panic
            .commands()
            .contains(&PersonalCommand::SetGlobalColour("404040".to_string()))
    );
}

#[test]
fn quick_actions_limit_named_personal_presets_for_dashboard_cards() {
    let presets = PersonalPreset::daily_presets();
    let selected = QuickActions::personal_preset_buttons(&presets);

    assert_eq!(selected.len(), 4);
    assert_eq!(selected[0].name(), "FX Panic");
    assert!(selected[0].is_safety_preset());
    assert_eq!(selected[1].name(), "Go Live");
}

#[test]
fn default_app_config_builds_the_existing_personal_scenes() {
    let names = AppConfig::default()
        .scenes()
        .into_iter()
        .map(|scene| scene.name().to_string())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["Gaming", "Music", "Night", "Call", "Safe Now"]);
    let gaming_commands = AppConfig::default().scenes()[0].commands();
    for command in UiScene::gaming().commands() {
        assert!(gaming_commands.contains(&command));
    }
}

#[test]
fn app_config_parses_editable_scene_volumes_and_eq_profiles() {
    let config = AppConfig::from_json_str(
        r#"
        {
          "scenes": [
            {
              "name": "Late Night Custom",
              "volumes": {
                "headphones": 42,
                "music": 12,
                "game": 34,
                "chat": 56
              },
              "clip_guard_enabled": true,
              "headphone_limiter_enabled": true,
              "headphone_eq_enabled": true,
              "headphone_eq_profile": "Soft Night"
            }
          ]
        }
        "#,
    )
    .expect("custom config should parse");

    let scenes = config.scenes();

    assert_eq!(
        scenes
            .iter()
            .map(|scene| scene.name().to_string())
            .collect::<Vec<_>>(),
        vec!["Late Night Custom"]
    );
    assert_eq!(
        scenes[0].commands(),
        vec![
            PersonalCommand::SetVolume(ChannelName::Headphones, 42),
            PersonalCommand::SetVolume(ChannelName::Music, 12),
            PersonalCommand::SetVolume(ChannelName::Game, 34),
            PersonalCommand::SetVolume(ChannelName::Chat, 56),
            PersonalCommand::SetClipGuardEnabled(true),
            PersonalCommand::SetHeadphoneLimiterEnabled(true),
            PersonalCommand::SetHeadphoneEqEnabled(true),
            PersonalCommand::LoadHeadphoneEqProfile("Soft Night".to_string()),
        ]
    );
}

#[test]
fn app_config_parses_mic_processing_and_safety_threshold_scene_actions() {
    let config = AppConfig::from_json_str(
        r#"
        {
          "scenes": [
            {
              "name": "Broadcast Mic",
              "mic_type": "Dynamic",
              "mic_gain": 58,
              "gate_enabled": true,
              "gate_threshold": -42,
              "gate_attenuation": 75,
              "gate_attack": 9,
              "gate_release": 20,
              "compressor_threshold": -18,
              "compressor_ratio": 9,
              "compressor_attack": 8,
              "compressor_release": 10,
              "compressor_makeup_gain": 6,
              "deesser": 45,
              "clip_guard_threshold": 12,
              "headphone_limiter_threshold": 87
            }
          ]
        }
        "#,
    )
    .expect("mic processing config should parse");

    assert_eq!(
        config.scenes()[0].commands(),
        vec![
            PersonalCommand::SetMicrophoneType(MicrophoneType::Dynamic),
            PersonalCommand::SetMicrophoneGain(MicrophoneType::Dynamic, 58),
            PersonalCommand::SetGateActive(true),
            PersonalCommand::SetGateThreshold(-42),
            PersonalCommand::SetGateAttenuation(75),
            PersonalCommand::SetGateAttack(GateTimes::Gate100ms),
            PersonalCommand::SetGateRelease(GateTimes::Gate250ms),
            PersonalCommand::SetCompressorThreshold(-18),
            PersonalCommand::SetCompressorRatio(CompressorRatio::Ratio4_0),
            PersonalCommand::SetCompressorAttack(CompressorAttackTime::Comp9ms),
            PersonalCommand::SetCompressorReleaseTime(CompressorReleaseTime::Comp115ms),
            PersonalCommand::SetCompressorMakeupGain(6),
            PersonalCommand::SetDeesser(45),
            PersonalCommand::SetClipGuardThreshold(12),
            PersonalCommand::SetHeadphoneLimiterThreshold(87),
        ]
    );
}

#[test]
fn mic_eq_editor_exposes_mini_and_full_band_controls() {
    let mini_bands = MicEqBandControl::mini_bands();
    assert_eq!(mini_bands.len(), 6);
    assert_eq!(mini_bands[0].label(), "90 Hz");
    assert_eq!(mini_bands[0].default_frequency_hz(), 90.0);
    assert_eq!(mini_bands[5].label(), "8 kHz");
    assert_eq!(mini_bands[5].default_frequency_hz(), 8000.0);
    assert_eq!(
        mini_bands[0].gain_command(3),
        PersonalCommand::SetEqMiniGain(MiniEqFrequencies::Equalizer90Hz, 3)
    );
    assert_eq!(
        mini_bands[0].frequency_command(95.0),
        PersonalCommand::SetEqMiniFreq(MiniEqFrequencies::Equalizer90Hz, 95.0)
    );

    let full_bands = MicEqBandControl::full_bands();
    assert_eq!(full_bands.len(), 10);
    assert_eq!(full_bands[0].label(), "31 Hz");
    assert_eq!(full_bands[0].default_frequency_hz(), 31.0);
    assert_eq!(full_bands[9].label(), "16 kHz");
    assert_eq!(full_bands[9].default_frequency_hz(), 16_000.0);
    assert_eq!(
        full_bands[9].gain_command(-2),
        PersonalCommand::SetEqGain(EqFrequencies::Equalizer16KHz, -2)
    );
    assert_eq!(
        full_bands[9].frequency_command(15_800.0),
        PersonalCommand::SetEqFreq(EqFrequencies::Equalizer16KHz, 15_800.0)
    );
    assert_eq!(MicLayoutPolicy::eq_panel_width(), 720.0);
}

#[test]
fn mic_setup_guidance_explains_gain_gate_compressor_and_live_meter_gap() {
    assert!(MicLayoutPolicy::uses_setup_guidance_cards());
    assert!(MicLayoutPolicy::setup_guide_panel_width() >= MicLayoutPolicy::panel_width());
    assert!(MicLayoutPolicy::setup_guide_panel_width() <= 460.0);
    assert!(MicLayoutPolicy::meter_placeholder_is_read_only());

    let steps = MicSetupGuideStep::daily_steps();
    assert_eq!(steps.len(), 4);
    assert_eq!(steps[0].label(), "1. Pick mic type");
    assert!(steps[0].description().contains("Dynamic"));
    assert_eq!(steps[1].label(), "2. Set gain before processing");
    assert!(steps[1].description().contains("peaks"));
    assert_eq!(steps[2].label(), "3. Close the gate gently");
    assert!(steps[2].description().contains("Gate threshold"));
    assert_eq!(steps[3].label(), "4. Add compression last");
    assert!(steps[3].description().contains("makeup gain"));
    assert!(steps.iter().all(|step| step.command().is_none()));

    let meter_note = MicSetupGuideStep::live_meter_status_note();
    assert!(meter_note.contains("not exposed"));
    assert!(meter_note.contains("read-only"));
}

#[test]
fn guarded_mic_profile_actions_require_confirmation_for_destructive_workflows() {
    let actions = MicProfileAction::guarded_daily_actions("Broadcast");
    let load = actions
        .iter()
        .find(|action| action.label() == "Load Broadcast")
        .expect("load action is exposed");
    let save_as = actions
        .iter()
        .find(|action| {
            action.command() == PersonalCommand::SaveMicProfileAs("Broadcast".to_string())
        })
        .expect("save-as action is exposed");
    let delete = actions
        .iter()
        .find(|action| {
            action.command() == PersonalCommand::DeleteMicProfile("Broadcast".to_string())
        })
        .expect("delete action is exposed");
    let save_current = actions
        .iter()
        .find(|action| action.command() == PersonalCommand::SaveMicProfile)
        .expect("save-current action is exposed");

    assert!(load.requires_confirmation());
    assert!(save_as.requires_confirmation());
    assert!(delete.requires_confirmation());
    assert!(save_current.requires_confirmation());
    assert_eq!(load.command_if_confirmed(false), None);
    assert_eq!(save_as.command_if_confirmed(false), None);
    assert_eq!(delete.command_if_confirmed(false), None);
    assert_eq!(save_current.command_if_confirmed(false), None);
    assert_eq!(
        save_current.command_if_confirmed(true),
        Some(PersonalCommand::SaveMicProfile)
    );
    assert_eq!(
        delete.command_if_confirmed(true),
        Some(PersonalCommand::DeleteMicProfile("Broadcast".to_string()))
    );
}

#[test]
fn advanced_effects_controls_cover_deeper_dsp_parameters() {
    let controls = EffectsAdvancedControl::daily_controls();
    assert!(controls.iter().any(|control| {
        control.label() == "Reverb decay"
            && control.command_for_default() == PersonalCommand::SetReverbDecay(1500)
    }));
    assert!(controls.iter().any(|control| {
        control.label() == "Echo feedback"
            && control.command_for_default() == PersonalCommand::SetEchoFeedback(35)
    }));
    assert!(controls.iter().any(|control| {
        control.label() == "Pitch character"
            && control.command_for_default() == PersonalCommand::SetPitchCharacter(50)
    }));
    assert!(controls.iter().any(|control| {
        control.label() == "Robot threshold"
            && control.command_for_default() == PersonalCommand::SetRobotThreshold(-40)
    }));
    assert!(controls.iter().any(|control| {
        control.label() == "Hard Tune source"
            && control.command_for_default()
                == PersonalCommand::SetHardTuneSource(HardTuneSource::Music)
    }));
}

#[test]
fn advanced_effects_controls_include_fuller_dsp_default_buttons() {
    let controls = EffectsAdvancedControl::daily_controls();
    assert!(controls.len() >= 24);

    let expected = [
        (
            "Reverb early level",
            PersonalCommand::SetReverbEarlyLevel(0),
        ),
        ("Reverb tail level", PersonalCommand::SetReverbTailLevel(0)),
        ("Reverb pre-delay", PersonalCommand::SetReverbPreDelay(25)),
        ("Reverb low colour", PersonalCommand::SetReverbLowColour(0)),
        (
            "Reverb high colour",
            PersonalCommand::SetReverbHighColour(0),
        ),
        (
            "Reverb high factor",
            PersonalCommand::SetReverbHighFactor(0),
        ),
        ("Reverb diffuse", PersonalCommand::SetReverbDiffuse(0)),
        ("Reverb mod speed", PersonalCommand::SetReverbModSpeed(0)),
        ("Reverb mod depth", PersonalCommand::SetReverbModDepth(0)),
        ("Echo tempo", PersonalCommand::SetEchoTempo(120)),
        ("Echo left delay", PersonalCommand::SetEchoDelayLeft(250)),
        ("Echo right delay", PersonalCommand::SetEchoDelayRight(375)),
        (
            "Echo left feedback",
            PersonalCommand::SetEchoFeedbackLeft(35),
        ),
        (
            "Echo right feedback",
            PersonalCommand::SetEchoFeedbackRight(35),
        ),
        ("Echo cross L→R", PersonalCommand::SetEchoFeedbackXFBLtoR(0)),
        ("Echo cross R→L", PersonalCommand::SetEchoFeedbackXFBRtoL(0)),
        (
            "Robot low gain",
            PersonalCommand::SetRobotGain(RobotRange::Low, 0),
        ),
        (
            "Robot mid frequency",
            PersonalCommand::SetRobotFreq(RobotRange::Medium, 60),
        ),
        (
            "Robot high width",
            PersonalCommand::SetRobotWidth(RobotRange::High, 50),
        ),
        ("Robot waveform", PersonalCommand::SetRobotWaveform(0)),
        ("Robot pulse width", PersonalCommand::SetRobotPulseWidth(50)),
        ("Robot dry mix", PersonalCommand::SetRobotDryMix(0)),
        ("Hard Tune amount", PersonalCommand::SetHardTuneAmount(50)),
        ("Hard Tune rate", PersonalCommand::SetHardTuneRate(50)),
        ("Hard Tune window", PersonalCommand::SetHardTuneWindow(200)),
    ];

    for (label, command) in expected {
        assert!(
            controls.iter().any(|control| {
                control.label() == label && control.command_for_default() == command
            }),
            "missing advanced Effects default control: {label}"
        );
    }
}

#[test]
fn headphone_eq_editor_exposes_preamp_and_ten_bands() {
    let bands = HeadphoneEqBandControl::ten_band_editor();
    assert_eq!(bands.len(), 10);
    assert_eq!(bands[0].index(), 0);
    assert_eq!(bands[0].label(), "31 Hz");
    assert_eq!(bands[0].default_frequency_hz(), 31.0);
    assert_eq!(bands[9].index(), 9);
    assert_eq!(bands[9].label(), "16 kHz");
    assert_eq!(bands[9].default_frequency_hz(), 16_000.0);
    assert_eq!(
        bands[2].gain_command(1.5),
        PersonalCommand::SetHeadphoneEqBandGain(2, 1.5)
    );
    assert_eq!(
        bands[2].frequency_command(128.0),
        PersonalCommand::SetHeadphoneEqBandFrequency(2, 128.0)
    );
    assert_eq!(
        bands[2].frequency_command(bands[2].default_frequency_hz()),
        PersonalCommand::SetHeadphoneEqBandFrequency(2, 125.0)
    );
    assert_eq!(
        bands[2].q_command(0.9),
        PersonalCommand::SetHeadphoneEqBandQ(2, 0.9)
    );
    assert!(HeadphoneEqLayoutPolicy::uses_guarded_profile_actions());
}

#[test]
fn headphone_eq_profile_actions_are_guarded_daily_workflows() {
    assert!(HeadphoneEqLayoutPolicy::uses_guarded_profile_actions());
    assert_eq!(HeadphoneEqLayoutPolicy::profile_panel_width(), 420.0);
    assert_eq!(HeadphoneEqLayoutPolicy::profile_button_width(), 150.0);

    let actions = HeadphoneEqProfileAction::guarded_daily_actions("Personal Phones");
    assert_eq!(actions.len(), 3);
    assert!(actions.iter().all(|action| action.requires_confirmation()));
    assert!(actions.iter().all(|action| !action.label().is_empty()));

    let load = actions
        .iter()
        .find(|action| action.label() == "Load Personal Phones")
        .unwrap();
    assert_eq!(
        load.command(),
        PersonalCommand::LoadHeadphoneEqProfile("Personal Phones".to_string())
    );
    assert_eq!(load.command_if_confirmed(false), None);
    assert_eq!(load.command_if_confirmed(true), Some(load.command()));

    let save = actions
        .iter()
        .find(|action| action.label() == "Save as Personal Phones")
        .unwrap();
    assert_eq!(
        save.command(),
        PersonalCommand::SaveHeadphoneEqProfile("Personal Phones".to_string())
    );
    assert_eq!(save.command_if_confirmed(false), None);
    assert_eq!(save.command_if_confirmed(true), Some(save.command()));

    let delete = actions
        .iter()
        .find(|action| action.label() == "Delete Personal Phones")
        .unwrap();
    assert_eq!(
        delete.command(),
        PersonalCommand::DeleteHeadphoneEqProfile("Personal Phones".to_string())
    );
    assert_eq!(delete.command_if_confirmed(false), None);
    assert_eq!(delete.command_if_confirmed(true), Some(delete.command()));
}

#[test]
fn headphone_eq_profile_actions_map_to_backend_commands() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::LoadHeadphoneEqProfile(
            "Personal Phones".to_string(),
        )),
        goxlr_ipc::GoXLRCommand::LoadHeadphoneEqProfile(profile) if profile == "Personal Phones"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SaveHeadphoneEqProfile(
            "Personal Phones".to_string(),
        )),
        goxlr_ipc::GoXLRCommand::SaveHeadphoneEqProfile(profile) if profile == "Personal Phones"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::DeleteHeadphoneEqProfile(
            "Personal Phones".to_string(),
        )),
        goxlr_ipc::GoXLRCommand::DeleteHeadphoneEqProfile(profile) if profile == "Personal Phones"
    ));
}

#[test]
fn headphone_eq_layout_uses_compact_five_by_two_grid() {
    assert_eq!(HeadphoneEqLayoutPolicy::grid_columns(), 5);
    assert_eq!(HeadphoneEqLayoutPolicy::grid_rows_for_band_count(10), 2);
    assert!(HeadphoneEqLayoutPolicy::uses_fixed_grid_rows());
    assert_eq!(HeadphoneEqLayoutPolicy::band_card_width(), 112.0);
    assert_eq!(HeadphoneEqLayoutPolicy::band_card_height(), 126.0);
    assert_eq!(HeadphoneEqLayoutPolicy::band_card_gap(), 10.0);
    assert!(HeadphoneEqLayoutPolicy::uses_equal_height_band_cards());
    let row_width = HeadphoneEqLayoutPolicy::band_card_width()
        * HeadphoneEqLayoutPolicy::grid_columns() as f32
        + HeadphoneEqLayoutPolicy::band_card_gap()
            * (HeadphoneEqLayoutPolicy::grid_columns() - 1) as f32;
    assert!(row_width <= HeadphoneEqLayoutPolicy::panel_width());
    assert!(HeadphoneEqLayoutPolicy::panel_width() <= 660.0);
    assert!(HeadphoneEqLayoutPolicy::panel_width() >= 600.0);
}

#[test]
fn sampler_layout_uses_compact_two_by_two_bank_cards() {
    assert!(SamplerLayoutPolicy::uses_bank_button_cards());
    assert!(SamplerLayoutPolicy::uses_two_by_two_slot_grid());
    assert_eq!(SamplerLayoutPolicy::bank_slot_columns(), 2);
    assert_eq!(SamplerLayoutPolicy::bank_slot_rows(), 2);
    assert_eq!(SamplerLayoutPolicy::bank_slot_card_width(), 156.0);
    assert_eq!(SamplerLayoutPolicy::bank_slot_card_height(), 132.0);
    let row_width = SamplerLayoutPolicy::bank_slot_card_width()
        * SamplerLayoutPolicy::bank_slot_columns() as f32
        + SamplerLayoutPolicy::bank_slot_gap()
            * (SamplerLayoutPolicy::bank_slot_columns() - 1) as f32;
    assert!(row_width <= SamplerLayoutPolicy::panel_width());
}

#[test]
fn dedicated_parity_tabs_expose_headphone_eq_and_sampler_pages() {
    let mut quick = QuickActions::default();

    quick.set_view_mode(AppViewMode::HeadphoneEq);
    assert_eq!(quick.view_mode(), AppViewMode::HeadphoneEq);
    quick.toggle_view_mode();
    assert_eq!(quick.view_mode(), AppViewMode::QuickActions);

    quick.set_view_mode(AppViewMode::Sampler);
    assert_eq!(quick.view_mode(), AppViewMode::Sampler);
    quick.toggle_view_mode();
    assert_eq!(quick.view_mode(), AppViewMode::QuickActions);
}

#[test]
fn sampler_page_exposes_bank_button_playback_actions() {
    let actions = SamplerAction::daily_bank_actions(SampleBank::A, SampleButtons::TopLeft);
    assert!(actions.iter().any(|action| {
        action.command() == PersonalCommand::SetActiveSamplerBank(SampleBank::A)
    }));
    assert!(actions.iter().any(|action| {
        action.command()
            == PersonalCommand::SetSamplerFunction(
                SampleBank::A,
                SampleButtons::TopLeft,
                SamplePlaybackMode::PlayStop,
            )
    }));
    assert!(actions.iter().any(|action| {
        action.command()
            == PersonalCommand::SetSamplerOrder(
                SampleBank::A,
                SampleButtons::TopLeft,
                SamplePlayOrder::Random,
            )
    }));
    assert!(actions.iter().any(|action| {
        action.command() == PersonalCommand::PlayNextSample(SampleBank::A, SampleButtons::TopLeft)
    }));
    assert!(SamplerLayoutPolicy::uses_bank_button_cards());
}

#[test]
fn sampler_page_exposes_safe_workflow_settings_and_trim_actions() {
    let settings = SamplerWorkflowSetting::safe_settings();
    assert!(settings.iter().any(|setting| {
        setting.command() == PersonalCommand::ClearSampleProcessError
            && setting.label() == "Clear process error"
    }));
    assert!(
        settings
            .iter()
            .any(|setting| { setting.command() == PersonalCommand::SetSamplerResetOnClear(true) })
    );
    assert!(
        settings
            .iter()
            .any(|setting| { setting.command() == PersonalCommand::SetSamplerResetOnClear(false) })
    );
    assert!(
        settings
            .iter()
            .any(|setting| { setting.command() == PersonalCommand::SetSamplerFadeDuration(250) })
    );
    assert!(
        settings
            .iter()
            .all(|setting| !setting.description().is_empty())
    );

    let trim_actions =
        SampleTrimAction::safe_trim_actions(SampleBank::B, SampleButtons::BottomRight, 0);
    assert!(trim_actions.iter().any(|action| {
        action.command()
            == PersonalCommand::SetSampleStartPercent(
                SampleBank::B,
                SampleButtons::BottomRight,
                0,
                0.0,
            )
    }));
    assert!(trim_actions.iter().any(|action| {
        action.command()
            == PersonalCommand::SetSampleStopPercent(
                SampleBank::B,
                SampleButtons::BottomRight,
                0,
                100.0,
            )
    }));
    assert!(trim_actions.iter().all(|action| !action.label().is_empty()));
    assert!(SamplerLayoutPolicy::exposes_file_import_controls());
}

#[test]
fn sampler_live_slot_snapshot_exposes_daemon_sample_indexes_and_actions() {
    let mut button_map = HashMap::new();
    button_map.insert(
        SampleButtons::TopLeft,
        SamplerButton {
            function: SamplePlaybackMode::PlayStop,
            order: SamplePlayOrder::Sequential,
            samples: vec![
                Sample {
                    name: "intro.wav".to_string(),
                    start_pct: 0.0,
                    stop_pct: 100.0,
                },
                Sample {
                    name: "sting.wav".to_string(),
                    start_pct: 12.5,
                    stop_pct: 87.5,
                },
            ],
            is_playing: true,
            is_recording: false,
        },
    );
    let mut banks = HashMap::new();
    banks.insert(SampleBank::A, button_map);
    let sampler = Sampler {
        processing_state: SampleProcessState {
            progress: None,
            last_error: None,
        },
        active_bank: SampleBank::A,
        clear_active: false,
        record_buffer: 0,
        banks,
    };

    let slots = SamplerSlotSnapshot::from_sampler(&sampler);
    assert_eq!(slots.len(), 1);
    let slot = &slots[0];
    assert_eq!(slot.bank(), SampleBank::A);
    assert_eq!(slot.button(), SampleButtons::TopLeft);
    assert_eq!(slot.function(), SamplePlaybackMode::PlayStop);
    assert_eq!(slot.order(), SamplePlayOrder::Sequential);
    assert_eq!(slot.sample_count(), 2);
    assert_eq!(slot.status_label(), "Playing");
    assert_eq!(slot.samples()[1].index(), 1);
    assert_eq!(slot.samples()[1].name(), "sting.wav");
    assert_eq!(slot.samples()[1].trim_label(), "12%–88%");

    let play_second = SamplerFileAction::play_by_index(SampleBank::A, SampleButtons::TopLeft, 1);
    assert_eq!(play_second.label(), "Play #2");
    assert_eq!(
        play_second.command_if_confirmed(false),
        Some(PersonalCommand::PlaySampleByIndex(
            SampleBank::A,
            SampleButtons::TopLeft,
            1,
        ))
    );
    let remove_second =
        SamplerFileAction::remove_by_index(SampleBank::A, SampleButtons::TopLeft, 1);
    assert_eq!(remove_second.label(), "Remove #2");
    assert!(remove_second.command_if_confirmed(false).is_none());
    assert_eq!(
        remove_second.command_if_confirmed(true),
        Some(PersonalCommand::RemoveSampleByIndex(
            SampleBank::A,
            SampleButtons::TopLeft,
            1,
        ))
    );
}

#[test]
fn sampler_loaded_sample_status_labels_cover_empty_ready_recording() {
    let sample = SamplerLoadedSample::new(0, "clip.wav", 0.0, 100.0);
    assert_eq!(sample.trim_label(), "0%–100%");

    let empty = SamplerSlotSnapshot::new(
        SampleBank::B,
        SampleButtons::BottomLeft,
        SamplePlaybackMode::PlayNext,
        SamplePlayOrder::Random,
        false,
        false,
        Vec::new(),
    );
    assert_eq!(empty.status_label(), "Empty");
    let ready = SamplerSlotSnapshot::new(
        SampleBank::B,
        SampleButtons::BottomLeft,
        SamplePlaybackMode::PlayNext,
        SamplePlayOrder::Random,
        false,
        false,
        vec![sample.clone()],
    );
    assert_eq!(ready.status_label(), "Ready");
    let recording = SamplerSlotSnapshot::new(
        SampleBank::B,
        SampleButtons::BottomLeft,
        SamplePlaybackMode::PlayNext,
        SamplePlayOrder::Random,
        true,
        true,
        vec![sample],
    );
    assert_eq!(recording.status_label(), "Recording");
}

#[test]
fn sampler_sample_browser_discovers_supported_audio_files_only() {
    assert_eq!(SamplerLayoutPolicy::sample_browser_panel_width(), 420.0);
    assert_eq!(SamplerLayoutPolicy::sample_browser_row_button_width(), 84.0);
    assert!(SamplerSampleBrowser::is_supported_audio_file(
        std::path::Path::new("clip.WAV")
    ));
    assert!(!SamplerSampleBrowser::is_supported_audio_file(
        std::path::Path::new("notes.txt")
    ));

    let dir = temp_test_dir("sampler-browser");
    fs::write(dir.join("zeta.mp3"), b"fake mp3").unwrap();
    fs::write(dir.join("alpha.WAV"), b"fake wav").unwrap();
    fs::write(dir.join("ignore.txt"), b"not audio").unwrap();
    fs::create_dir_all(dir.join("nested.flac")).unwrap();

    let browser = SamplerSampleBrowser::from_directory(&dir);
    let names = browser
        .rows()
        .iter()
        .map(|row| row.display_name())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["alpha.WAV", "zeta.mp3"]);
    assert!(browser.rows()[0].path().ends_with("alpha.WAV"));
    assert_eq!(browser.root(), dir.as_path());

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sampler_file_workflow_builds_guarded_add_remove_actions() {
    assert_eq!(SamplerLayoutPolicy::file_workflow_panel_width(), 420.0);
    assert_eq!(SamplerLayoutPolicy::file_workflow_button_width(), 120.0);

    assert!(
        SamplerFileAction::add_from_path(SampleBank::A, SampleButtons::TopLeft, "   ").is_none()
    );

    let add = SamplerFileAction::add_from_path(
        SampleBank::A,
        SampleButtons::TopLeft,
        "  /home/pc/samples/clip.wav  ",
    )
    .expect("non-empty sample path should produce an add action");
    assert_eq!(add.label(), "Add file");
    assert!(add.requires_confirmation());
    assert!(add.command_if_confirmed(false).is_none());
    assert_eq!(
        add.command_if_confirmed(true),
        Some(PersonalCommand::AddSample(
            SampleBank::A,
            SampleButtons::TopLeft,
            "/home/pc/samples/clip.wav".to_string(),
        ))
    );
    assert!(!add.description().is_empty());

    let remove = SamplerFileAction::remove_first(SampleBank::B, SampleButtons::BottomRight);
    assert_eq!(remove.label(), "Remove #1");
    assert!(remove.requires_confirmation());
    assert_eq!(
        remove.command_if_confirmed(true),
        Some(PersonalCommand::RemoveSampleByIndex(
            SampleBank::B,
            SampleButtons::BottomRight,
            0,
        ))
    );

    let play = SamplerFileAction::play_first(SampleBank::C, SampleButtons::TopRight);
    assert!(!play.requires_confirmation());
    assert_eq!(
        play.command_if_confirmed(false),
        Some(PersonalCommand::PlaySampleByIndex(
            SampleBank::C,
            SampleButtons::TopRight,
            0,
        ))
    );
}

#[test]
fn sampler_workflow_settings_map_to_backend_commands() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::ClearSampleProcessError),
        goxlr_ipc::GoXLRCommand::ClearSampleProcessError()
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetSamplerResetOnClear(true)),
        goxlr_ipc::GoXLRCommand::SetSamplerResetOnClear(true)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetSamplerFadeDuration(350)),
        goxlr_ipc::GoXLRCommand::SetSamplerFadeDuration(350)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetSampleStartPercent(
            SampleBank::C,
            SampleButtons::TopRight,
            1,
            12.5,
        )),
        goxlr_ipc::GoXLRCommand::SetSampleStartPercent(
            SampleBank::C,
            SampleButtons::TopRight,
            1,
            pct,
        ) if pct == 12.5
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetSampleStopPercent(
            SampleBank::C,
            SampleButtons::TopRight,
            1,
            87.5,
        )),
        goxlr_ipc::GoXLRCommand::SetSampleStopPercent(
            SampleBank::C,
            SampleButtons::TopRight,
            1,
            pct,
        ) if pct == 87.5
    ));
}

#[test]
fn sampler_file_workflow_maps_to_backend_commands() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::AddSample(
            SampleBank::A,
            SampleButtons::TopLeft,
            "/tmp/clip.wav".to_string(),
        )),
        goxlr_ipc::GoXLRCommand::AddSample(
            SampleBank::A,
            SampleButtons::TopLeft,
            path,
        ) if path == "/tmp/clip.wav"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::RemoveSampleByIndex(
            SampleBank::B,
            SampleButtons::BottomLeft,
            0,
        )),
        goxlr_ipc::GoXLRCommand::RemoveSampleByIndex(SampleBank::B, SampleButtons::BottomLeft, 0,)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::PlaySampleByIndex(
            SampleBank::C,
            SampleButtons::BottomRight,
            0,
        )),
        goxlr_ipc::GoXLRCommand::PlaySampleByIndex(SampleBank::C, SampleButtons::BottomRight, 0,)
    ));
}

#[test]
fn personal_command_maps_new_parity_chunks_to_backend_commands() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetEqGain(EqFrequencies::Equalizer1KHz, 4,)),
        goxlr_ipc::GoXLRCommand::SetEqGain(EqFrequencies::Equalizer1KHz, 4)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::LoadMicProfile(
            "Broadcast".to_string(),
            true,
        )),
        goxlr_ipc::GoXLRCommand::LoadMicProfile(_, true)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetReverbDecay(1200)),
        goxlr_ipc::GoXLRCommand::SetReverbDecay(1200)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetHeadphoneEqPreamp(-1.5)),
        goxlr_ipc::GoXLRCommand::SetHeadphoneEqPreamp(preamp) if preamp == -1.5
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::PlayNextSample(
            SampleBank::B,
            SampleButtons::BottomRight,
        )),
        goxlr_ipc::GoXLRCommand::PlayNextSample(SampleBank::B, SampleButtons::BottomRight)
    ));
}

#[test]
fn personal_command_maps_mic_processing_to_backend_commands() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetMicrophoneGain(
            MicrophoneType::Condenser,
            33,
        )),
        goxlr_ipc::GoXLRCommand::SetMicrophoneGain(MicrophoneType::Condenser, 33)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetGateRelease(GateTimes::Gate500ms)),
        goxlr_ipc::GoXLRCommand::SetGateRelease(GateTimes::Gate500ms)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetCompressorRatio(
            CompressorRatio::Ratio8_0,
        )),
        goxlr_ipc::GoXLRCommand::SetCompressorRatio(CompressorRatio::Ratio8_0)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetDeesser(64)),
        goxlr_ipc::GoXLRCommand::SetDeeser(64)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetHeadphoneLimiterThreshold(90)),
        goxlr_ipc::GoXLRCommand::SetHeadphoneLimiterThreshold(90)
    ));
}

#[test]
fn app_view_mode_has_dedicated_mic_effects_and_lighting_pages_for_parity_chunks() {
    let mut quick_actions = QuickActions::default();
    quick_actions.set_view_mode(AppViewMode::Mic);
    assert_eq!(quick_actions.view_mode(), AppViewMode::Mic);

    quick_actions.set_view_mode(AppViewMode::Effects);
    assert_eq!(quick_actions.view_mode(), AppViewMode::Effects);

    quick_actions.set_view_mode(AppViewMode::Lighting);
    assert_eq!(quick_actions.view_mode(), AppViewMode::Lighting);
}

#[test]
fn lighting_profile_actions_are_guarded_colour_only_workflows() {
    assert!(LightingLayoutPolicy::uses_guarded_profile_colour_actions());
    assert_eq!(LightingLayoutPolicy::profile_panel_width(), 420.0);
    assert_eq!(LightingLayoutPolicy::profile_button_width(), 170.0);

    let actions = LightingProfileAction::guarded_daily_actions("Personal");
    assert_eq!(actions.len(), 1);
    assert!(
        actions
            .iter()
            .all(LightingProfileAction::requires_confirmation)
    );
    assert!(
        actions
            .iter()
            .all(|action| !action.description().is_empty())
    );

    let action = &actions[0];
    assert_eq!(action.label(), "Load Personal lighting");
    assert_eq!(
        action.command(),
        PersonalCommand::LoadProfileColours("Personal".to_string())
    );
    assert_eq!(
        action.command_if_confirmed(false),
        None,
        "lighting-only profile load must require an explicit second click"
    );
    assert_eq!(
        action.command_if_confirmed(true),
        Some(PersonalCommand::LoadProfileColours("Personal".to_string()))
    );
}

#[test]
fn lighting_profile_colour_action_maps_to_backend_command() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::LoadProfileColours(
            "Personal".to_string()
        )),
        goxlr_ipc::GoXLRCommand::LoadProfileColours(profile) if profile == "Personal"
    ));
}

#[test]
fn lighting_quick_themes_cover_daily_visual_states() {
    let themes = LightingQuickTheme::daily_themes();
    let names = themes.iter().map(|theme| theme.name()).collect::<Vec<_>>();

    assert_eq!(
        names,
        vec!["Dim White", "Broadcast Red", "Cool Blue", "Lights Off"]
    );
    assert_eq!(
        themes[0].commands(),
        vec![
            PersonalCommand::SetAnimationMode(AnimationMode::Simple),
            PersonalCommand::SetGlobalColour("404040".to_string()),
            PersonalCommand::SetAllFaderColours("606060".to_string(), "202020".to_string()),
            PersonalCommand::SetButtonGroupColours(
                ButtonColourGroups::FaderMute,
                "404040".to_string(),
                Some("101010".to_string()),
            ),
            PersonalCommand::SetSimpleColour(SimpleColourTargets::Accent, "808080".to_string()),
        ]
    );
    assert_eq!(
        themes[1].commands(),
        vec![
            PersonalCommand::SetAnimationMode(AnimationMode::Simple),
            PersonalCommand::SetGlobalColour("FF1F1F".to_string()),
            PersonalCommand::SetAllFaderColours("FF3030".to_string(), "400000".to_string()),
            PersonalCommand::SetButtonGroupColours(
                ButtonColourGroups::EffectTypes,
                "FF3030".to_string(),
                Some("400000".to_string()),
            ),
            PersonalCommand::SetSimpleColour(SimpleColourTargets::Accent, "FF8080".to_string()),
        ]
    );
    assert_eq!(
        themes[3].commands(),
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
        ]
    );
}

#[test]
fn lighting_colour_editor_targets_cover_full_lighting_parity_controls() {
    let simple_targets = LightingSimpleColourTarget::all_targets();
    assert_eq!(simple_targets.len(), 6);
    assert_eq!(simple_targets[0].label(), "Global");
    assert_eq!(
        simple_targets[0].command_for_colour("ABCDEF"),
        PersonalCommand::SetSimpleColour(SimpleColourTargets::Global, "ABCDEF".to_string())
    );
    assert!(
        simple_targets
            .iter()
            .any(|target| target.target() == SimpleColourTargets::Scribble4)
    );

    let fader_targets = LightingFaderColourTarget::all_targets();
    assert_eq!(fader_targets.len(), 5);
    assert_eq!(fader_targets[0].label(), "All faders");
    assert_eq!(
        fader_targets[0].colour_command("112233", "445566"),
        PersonalCommand::SetAllFaderColours("112233".to_string(), "445566".to_string())
    );
    assert_eq!(
        fader_targets[1].colour_command("010203", "040506"),
        PersonalCommand::SetFaderColours(FaderName::A, "010203".to_string(), "040506".to_string(),)
    );
    assert_eq!(
        fader_targets[1].display_command(FaderDisplayStyle::GradientMeter),
        PersonalCommand::SetFaderDisplayStyle(FaderName::A, FaderDisplayStyle::GradientMeter)
    );

    let button_targets = LightingButtonColourTarget::daily_targets();
    assert!(
        button_targets
            .iter()
            .any(|target| target.label() == "Effect types")
    );
    assert!(
        button_targets
            .iter()
            .any(|target| target.label() == "Cough button")
    );
    assert_eq!(
        button_targets[0].off_style_command(ButtonColourOffStyle::DimmedColour2),
        PersonalCommand::SetButtonGroupOffStyle(
            ButtonColourGroups::FaderMute,
            ButtonColourOffStyle::DimmedColour2,
        )
    );

    let triple_targets = LightingTripleColourTarget::all_targets();
    assert_eq!(triple_targets.len(), 7);
    assert_eq!(
        triple_targets[0].colour_command("111111", "222222", "333333"),
        PersonalCommand::SetEncoderColour(
            EncoderColourTargets::Reverb,
            "111111".to_string(),
            "222222".to_string(),
            "333333".to_string(),
        )
    );
    assert!(
        triple_targets
            .iter()
            .any(|target| target.label() == "Sampler select C")
    );
}

#[test]
fn lighting_animation_controls_cover_modes_modifiers_and_waterfall() {
    let controls = LightingAnimationControl::practical_controls();
    let labels = controls
        .iter()
        .map(|control| control.label())
        .collect::<Vec<_>>();

    assert_eq!(labels, vec!["Mode", "Mod 1", "Mod 2", "Waterfall"]);
    assert_eq!(
        controls[0].command_for_value(1),
        PersonalCommand::SetAnimationMode(AnimationMode::RainbowDark)
    );
    assert_eq!(
        controls[1].command_for_value(200),
        PersonalCommand::SetAnimationMod1(100)
    );
    assert_eq!(
        controls[2].command_for_value(42),
        PersonalCommand::SetAnimationMod2(42)
    );
    assert_eq!(
        controls[3].command_for_value(0),
        PersonalCommand::SetAnimationWaterfall(WaterfallDirection::Down)
    );
    assert_eq!(
        controls[3].command_for_value(1),
        PersonalCommand::SetAnimationWaterfall(WaterfallDirection::Up)
    );
}

#[test]
fn personal_command_maps_lighting_controls_to_backend_commands() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetAnimationMode(AnimationMode::Ripple)),
        goxlr_ipc::GoXLRCommand::SetAnimationMode(AnimationMode::Ripple)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetGlobalColour("ABCDEF".to_string())),
        goxlr_ipc::GoXLRCommand::SetGlobalColour(colour) if colour == "ABCDEF"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetAllFaderColours(
            "112233".to_string(),
            "445566".to_string(),
        )),
        goxlr_ipc::GoXLRCommand::SetAllFaderColours(top, bottom)
            if top == "112233" && bottom == "445566"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetButtonGroupColours(
            ButtonColourGroups::EffectSelector,
            "AA0000".to_string(),
            Some("110000".to_string()),
        )),
        goxlr_ipc::GoXLRCommand::SetButtonGroupColours(
            ButtonColourGroups::EffectSelector,
            colour_one,
            Some(colour_two),
        ) if colour_one == "AA0000" && colour_two == "110000"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetSimpleColour(
            SimpleColourTargets::Accent,
            "00AAFF".to_string(),
        )),
        goxlr_ipc::GoXLRCommand::SetSimpleColour(SimpleColourTargets::Accent, colour)
            if colour == "00AAFF"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetAnimationMod1(77)),
        goxlr_ipc::GoXLRCommand::SetAnimationMod1(77)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetAnimationWaterfall(
            WaterfallDirection::Up,
        )),
        goxlr_ipc::GoXLRCommand::SetAnimationWaterfall(WaterfallDirection::Up)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetFaderColours(
            FaderName::B,
            "010101".to_string(),
            "020202".to_string(),
        )),
        goxlr_ipc::GoXLRCommand::SetFaderColours(FaderName::B, top, bottom)
            if top == "010101" && bottom == "020202"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetButtonColours(
            Button::Cough,
            "101010".to_string(),
            Some("202020".to_string()),
        )),
        goxlr_ipc::GoXLRCommand::SetButtonColours(Button::Cough, colour_one, Some(colour_two))
            if colour_one == "101010" && colour_two == "202020"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetEncoderColour(
            EncoderColourTargets::Pitch,
            "111111".to_string(),
            "222222".to_string(),
            "333333".to_string(),
        )),
        goxlr_ipc::GoXLRCommand::SetEncoderColour(
            EncoderColourTargets::Pitch,
            colour_one,
            colour_two,
            colour_three,
        ) if colour_one == "111111" && colour_two == "222222" && colour_three == "333333"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetSampleOffStyle(
            SamplerColourTargets::SamplerSelectB,
            ButtonColourOffStyle::Colour2,
        )),
        goxlr_ipc::GoXLRCommand::SetSampleOffStyle(
            SamplerColourTargets::SamplerSelectB,
            ButtonColourOffStyle::Colour2,
        )
    ));
}

#[test]
fn effects_quick_presets_cover_daily_voice_fx_controls() {
    let presets = EffectsQuickPreset::daily_presets();
    let names = presets
        .iter()
        .map(|preset| preset.name())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec!["FX Off", "Clean Reverb", "Robot Fun", "Hard Tune"]
    );
    assert_eq!(
        presets[0].commands(),
        vec![
            PersonalCommand::SetFXEnabled(false),
            PersonalCommand::SetMegaphoneEnabled(false),
            PersonalCommand::SetRobotEnabled(false),
            PersonalCommand::SetHardTuneEnabled(false),
        ]
    );
    assert_eq!(
        presets[1].commands(),
        vec![
            PersonalCommand::SetActiveEffectPreset(EffectBankPresets::Preset1),
            PersonalCommand::SetFXEnabled(true),
            PersonalCommand::SetReverbStyle(ReverbStyle::RealPlate),
            PersonalCommand::SetReverbAmount(28),
            PersonalCommand::SetEchoStyle(EchoStyle::Quarter),
            PersonalCommand::SetEchoAmount(0),
        ]
    );
    assert_eq!(
        presets[2].commands(),
        vec![
            PersonalCommand::SetActiveEffectPreset(EffectBankPresets::Preset2),
            PersonalCommand::SetFXEnabled(true),
            PersonalCommand::SetRobotEnabled(true),
            PersonalCommand::SetRobotStyle(RobotStyle::Robot1),
            PersonalCommand::SetMegaphoneEnabled(false),
            PersonalCommand::SetHardTuneEnabled(false),
        ]
    );
    assert_eq!(
        presets[3].commands(),
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
        ]
    );
}

#[test]
fn effects_preset_actions_are_guarded_daily_workflows() {
    assert!(EffectsLayoutPolicy::uses_guarded_preset_management());
    assert_eq!(EffectsLayoutPolicy::preset_management_panel_width(), 520.0);
    assert_eq!(EffectsLayoutPolicy::preset_management_button_width(), 150.0);

    let actions = EffectPresetAction::guarded_daily_actions("Personal");
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[0].label(), "Load Personal");
    assert_eq!(
        actions[0].command(),
        PersonalCommand::LoadEffectPreset("Personal".to_string())
    );
    assert_eq!(
        actions[1].command(),
        PersonalCommand::RenameActiveEffectPreset("Personal".to_string())
    );
    assert_eq!(
        actions[2].command(),
        PersonalCommand::SaveActiveEffectPreset
    );
    assert!(actions.iter().all(|action| action.requires_confirmation()));
    assert_eq!(actions[0].command_if_confirmed(false), None);
    assert_eq!(
        actions[0].command_if_confirmed(true),
        Some(PersonalCommand::LoadEffectPreset("Personal".to_string()))
    );
}

#[test]
fn effect_preset_actions_map_to_backend_commands() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::LoadEffectPreset("Personal".to_string())),
        goxlr_ipc::GoXLRCommand::LoadEffectPreset(profile) if profile == "Personal"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::RenameActiveEffectPreset(
            "Personal".to_string()
        )),
        goxlr_ipc::GoXLRCommand::RenameActivePreset(profile) if profile == "Personal"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SaveActiveEffectPreset),
        goxlr_ipc::GoXLRCommand::SaveActivePreset()
    ));
}

#[test]
fn effects_amount_controls_cover_full_slider_parity() {
    let controls = EffectsAmountControl::full_controls();
    let labels = controls
        .iter()
        .map(|control| control.label())
        .collect::<Vec<_>>();

    assert_eq!(
        labels,
        vec![
            "Reverb amount",
            "Echo amount",
            "Pitch amount",
            "Gender amount",
            "Megaphone amount",
        ]
    );
    assert_eq!(controls[0].range(), 0..=100);
    assert_eq!(controls[2].range(), -50..=50);
    assert_eq!(
        controls[0].command_for_value(37),
        PersonalCommand::SetReverbAmount(37)
    );
    assert_eq!(
        controls[1].command_for_value(44),
        PersonalCommand::SetEchoAmount(44)
    );
    assert_eq!(
        controls[2].command_for_value(-12),
        PersonalCommand::SetPitchAmount(-12)
    );
    assert_eq!(
        controls[3].command_for_value(18),
        PersonalCommand::SetGenderAmount(18)
    );
    assert_eq!(
        controls[4].command_for_value(63),
        PersonalCommand::SetMegaphoneAmount(63)
    );
}

#[test]
fn effects_style_groups_cover_full_button_parity() {
    let groups = EffectsStyleGroup::full_groups();
    let labels = groups.iter().map(|group| group.label()).collect::<Vec<_>>();

    assert_eq!(
        labels,
        vec![
            "Reverb style",
            "Echo style",
            "Pitch style",
            "Gender style",
            "Megaphone style",
            "Robot style",
            "Hard tune style",
        ]
    );
    assert_eq!(groups[0].commands().len(), 6);
    assert!(
        groups[0]
            .commands()
            .contains(&PersonalCommand::SetReverbStyle(ReverbStyle::HockeyArena))
    );
    assert!(
        groups[1]
            .commands()
            .contains(&PersonalCommand::SetEchoStyle(EchoStyle::PingPong))
    );
    assert!(
        groups[4]
            .commands()
            .contains(&PersonalCommand::SetMegaphoneStyle(MegaphoneStyle::Radio))
    );
    assert!(
        groups[5]
            .commands()
            .contains(&PersonalCommand::SetRobotStyle(RobotStyle::Robot3))
    );
    assert!(
        groups[6]
            .commands()
            .contains(&PersonalCommand::SetHardTuneStyle(HardTuneStyle::Natural))
    );
}

#[test]
fn personal_command_maps_effects_controls_to_backend_commands() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetActiveEffectPreset(
            EffectBankPresets::Preset4
        )),
        goxlr_ipc::GoXLRCommand::SetActiveEffectPreset(EffectBankPresets::Preset4)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetFXEnabled(true)),
        goxlr_ipc::GoXLRCommand::SetFXEnabled(true)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetReverbStyle(ReverbStyle::Chapel)),
        goxlr_ipc::GoXLRCommand::SetReverbStyle(ReverbStyle::Chapel)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetEchoAmount(42)),
        goxlr_ipc::GoXLRCommand::SetEchoAmount(42)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetRobotEnabled(true)),
        goxlr_ipc::GoXLRCommand::SetRobotEnabled(true)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetHardTuneStyle(HardTuneStyle::Hard)),
        goxlr_ipc::GoXLRCommand::SetHardTuneStyle(HardTuneStyle::Hard)
    ));
}

#[test]
fn app_scene_config_reload_uses_updated_scene_file() {
    let path = temp_scene_config_path("reload");
    fs::write(
        &path,
        r#"{"scenes":[{"name":"First","volumes":{"music":11}}]}"#,
    )
    .unwrap();
    let mut state = AppSceneConfig::load_or_default(path.clone());

    assert_eq!(state.scene_names(), vec!["First"]);
    assert_eq!(state.reload_error(), None);

    fs::write(
        &path,
        r#"{"scenes":[{"name":"Second","volumes":{"music":22}}]}"#,
    )
    .unwrap();

    state.reload();

    assert_eq!(state.scene_names(), vec!["Second"]);
    assert_eq!(
        state.scenes()[0].commands(),
        vec![PersonalCommand::SetVolume(ChannelName::Music, 22)]
    );
    assert_eq!(state.reload_error(), None);
    let _ = fs::remove_file(path);
}

#[test]
fn app_scene_config_reload_keeps_previous_scenes_on_parse_error() {
    let path = temp_scene_config_path("bad-reload");
    fs::write(
        &path,
        r#"{"scenes":[{"name":"Good","volumes":{"chat":33}}]}"#,
    )
    .unwrap();
    let mut state = AppSceneConfig::load_or_default(path.clone());

    fs::write(&path, "not json").unwrap();
    state.reload();

    assert_eq!(state.scene_names(), vec!["Good"]);
    assert!(state.reload_error().unwrap().contains("failed to parse"));
    let _ = fs::remove_file(path);
}

#[test]
fn scene_editor_saves_updated_scene_to_config_file() {
    let path = temp_scene_config_path("editor-save");
    fs::write(
        &path,
        r#"{"scenes":[{"name":"Original","volumes":{"music":11,"game":22}}]}"#,
    )
    .unwrap();
    let mut state = AppSceneConfig::load_or_default(path.clone());
    let mut editor = SceneEditor::from_config(state.config());

    editor.set_selected_scene(0);
    editor.set_scene_name("Edited");
    editor.set_volume(ChannelName::Music, Some(66));
    editor.set_volume(ChannelName::Game, None);
    editor.set_headphone_eq_profile(Some("Edited EQ".to_string()));
    editor.save_to(&mut state).unwrap();

    let saved = AppConfig::from_json_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved.scenes[0].name, "Edited");
    assert_eq!(saved.scenes[0].volumes.music, Some(66));
    assert_eq!(saved.scenes[0].volumes.game, None);
    assert_eq!(
        saved.scenes[0].headphone_eq_profile.as_deref(),
        Some("Edited EQ")
    );
    assert_eq!(state.scene_names(), vec!["Edited"]);
    assert_eq!(
        state.scenes()[0].commands(),
        vec![
            PersonalCommand::SetVolume(ChannelName::Music, 66),
            PersonalCommand::LoadHeadphoneEqProfile("Edited EQ".to_string()),
        ]
    );
    let _ = fs::remove_file(path);
}

#[test]
fn scene_editor_exposes_optional_bool_actions_as_unset_set_true_or_set_false() {
    let config = AppConfig::from_json_str(
        r#"{"scenes":[{"name":"Scene","clip_guard_enabled":true,"headphone_limiter_enabled":false}]}"#,
    )
    .unwrap();
    let mut editor = SceneEditor::from_config(&config);

    assert_eq!(editor.clip_guard_action(), OptionalBoolAction::SetTrue);
    assert_eq!(
        editor.headphone_limiter_action(),
        OptionalBoolAction::SetFalse
    );
    assert_eq!(editor.headphone_eq_action(), OptionalBoolAction::Unset);

    editor.set_clip_guard_action(OptionalBoolAction::Unset);
    editor.set_headphone_limiter_action(OptionalBoolAction::SetTrue);
    editor.set_headphone_eq_action(OptionalBoolAction::SetFalse);

    let scene = editor.selected_scene_config().unwrap();
    assert_eq!(scene.clip_guard_enabled, None);
    assert_eq!(scene.headphone_limiter_enabled, Some(true));
    assert_eq!(scene.headphone_eq_enabled, Some(false));
}

#[test]
fn scene_editor_can_clear_headphone_eq_profile_action() {
    let config =
        AppConfig::from_json_str(r#"{"scenes":[{"name":"Scene","headphone_eq_profile":"Music"}]}"#)
            .unwrap();
    let mut editor = SceneEditor::from_config(&config);

    editor.set_headphone_eq_profile_action_enabled(false);

    assert_eq!(
        editor.selected_scene_config().unwrap().headphone_eq_profile,
        None
    );
}

#[test]
fn scene_editor_adds_new_scene_after_selected_scene() {
    let config =
        AppConfig::from_json_str(r#"{"scenes":[{"name":"First"},{"name":"Second"}]}"#).unwrap();
    let mut editor = SceneEditor::from_config(&config);

    editor.set_selected_scene(0);
    editor.add_scene();

    assert_eq!(editor.scene_names(), vec!["First", "New Scene", "Second"]);
    assert_eq!(editor.selected_scene(), 1);
    assert_eq!(
        editor.selected_scene_config().unwrap().volumes,
        Default::default()
    );
}

#[test]
fn scene_editor_deletes_selected_scene_and_keeps_one_empty_scene() {
    let config = AppConfig::from_json_str(
        r#"{"scenes":[{"name":"First"},{"name":"Second"},{"name":"Third"}]}"#,
    )
    .unwrap();
    let mut editor = SceneEditor::from_config(&config);

    editor.set_selected_scene(1);
    editor.delete_selected_scene();
    assert_eq!(editor.scene_names(), vec!["First", "Third"]);
    assert_eq!(editor.selected_scene(), 1);

    editor.delete_selected_scene();
    editor.delete_selected_scene();
    assert_eq!(editor.scene_names(), vec!["New Scene"]);
    assert_eq!(editor.selected_scene(), 0);
}

#[test]
fn scene_editor_moves_selected_scene_up_and_down() {
    let config = AppConfig::from_json_str(
        r#"{"scenes":[{"name":"First"},{"name":"Second"},{"name":"Third"}]}"#,
    )
    .unwrap();
    let mut editor = SceneEditor::from_config(&config);

    editor.set_selected_scene(1);
    editor.move_selected_scene_up();
    assert_eq!(editor.scene_names(), vec!["Second", "First", "Third"]);
    assert_eq!(editor.selected_scene(), 0);

    editor.move_selected_scene_down();
    editor.move_selected_scene_down();
    assert_eq!(editor.scene_names(), vec!["First", "Third", "Second"]);
    assert_eq!(editor.selected_scene(), 2);
}

#[test]
fn gaming_scene_prioritizes_game_and_chat() {
    let commands = UiScene::gaming().commands();

    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Game, 85)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Chat, 70)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Music, 35)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Headphones, 75)));
    assert!(commands.contains(&PersonalCommand::SetHeadphoneLimiterEnabled(true)));
}

#[test]
fn music_scene_prioritizes_music_and_eq() {
    let commands = UiScene::music().commands();

    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Music, 85)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Game, 30)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Chat, 35)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Headphones, 80)));
    assert!(commands.contains(&PersonalCommand::SetHeadphoneLimiterEnabled(true)));
    assert!(commands.contains(&PersonalCommand::SetHeadphoneEqEnabled(true)));
    assert!(commands.contains(&PersonalCommand::LoadHeadphoneEqProfile(
        "Music".to_string()
    )));
}

#[test]
fn night_scene_keeps_headphones_quiet_and_safety_enabled() {
    let commands = UiScene::night().commands();

    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Music, 35)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Game, 35)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Chat, 45)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Headphones, 55)));
    assert!(commands.contains(&PersonalCommand::SetHeadphoneLimiterEnabled(true)));
    assert!(commands.contains(&PersonalCommand::SetHeadphoneEqEnabled(true)));
    assert!(commands.contains(&PersonalCommand::LoadHeadphoneEqProfile(
        "Night".to_string()
    )));
}

#[test]
fn call_scene_prioritizes_chat_and_reduces_media() {
    let commands = UiScene::call_scene().commands();

    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Chat, 85)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Music, 15)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Game, 20)));
    assert!(commands.contains(&PersonalCommand::SetVolume(ChannelName::Headphones, 70)));
    assert!(commands.contains(&PersonalCommand::SetHeadphoneLimiterEnabled(true)));
    assert!(commands.contains(&PersonalCommand::SetClipGuardEnabled(true)));
}

#[test]
fn quick_actions_default_to_dashboard_and_toggle_to_full_editor() {
    let mut quick_actions = QuickActions::default();

    assert_eq!(quick_actions.view_mode(), AppViewMode::QuickActions);

    quick_actions.toggle_view_mode();
    assert_eq!(quick_actions.view_mode(), AppViewMode::Full);

    quick_actions.toggle_view_mode();
    assert_eq!(quick_actions.view_mode(), AppViewMode::QuickActions);
}

#[test]
fn quick_actions_prefers_safe_now_then_first_four_scenes() {
    let scenes = vec![
        UiScene::new("Gaming", Vec::new()),
        UiScene::new("Music", Vec::new()),
        UiScene::new("Night", Vec::new()),
        UiScene::new("Call", Vec::new()),
        UiScene::new("Safe Now", Vec::new()),
        UiScene::new("Extra", Vec::new()),
    ];

    let names = QuickActions::scene_buttons(&scenes)
        .into_iter()
        .map(|scene| scene.name().to_string())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["Safe Now", "Gaming", "Music", "Night", "Call"]);
}

#[test]
fn quick_actions_include_safety_commands_for_fast_access() {
    let commands = QuickActions::safety_commands();

    assert_eq!(
        commands,
        vec![
            PersonalCommand::SetClipGuardEnabled(true),
            PersonalCommand::SetHeadphoneLimiterEnabled(true),
            PersonalCommand::SetHeadphoneEqEnabled(true),
        ]
    );
}

#[test]
fn mini_window_mode_defaults_to_normal_window() {
    let mode = MiniWindowMode::default();

    assert!(!mode.is_mini());
    assert!(!mode.always_on_top());
    assert_eq!(mode.window_action(), WindowAction::NormalSize);
}

#[test]
fn mini_window_mode_enters_quick_actions_and_resizes_window() {
    let mut mode = MiniWindowMode::default();
    let mut quick_actions = QuickActions::default();

    let action = mode.toggle_mini_window(&mut quick_actions);

    assert!(mode.is_mini());
    assert!(mode.always_on_top());
    assert_eq!(quick_actions.view_mode(), AppViewMode::QuickActions);
    assert_eq!(action, WindowAction::MiniSize);
    assert_eq!(mode.window_action(), WindowAction::MiniSize);
}

#[test]
fn mini_window_mode_restores_full_view_and_normal_size() {
    let mut mode = MiniWindowMode::default();
    let mut quick_actions = QuickActions::default();

    mode.toggle_mini_window(&mut quick_actions);
    let action = mode.toggle_mini_window(&mut quick_actions);

    assert!(!mode.is_mini());
    assert!(!mode.always_on_top());
    assert_eq!(quick_actions.view_mode(), AppViewMode::Full);
    assert_eq!(action, WindowAction::NormalSize);
}

#[test]
fn tray_menu_has_stable_action_order_and_labels() {
    let model = TrayMenuModel::default();

    assert_eq!(
        model.items(),
        vec![
            (TrayAction::ShowFull, "Show full window"),
            (TrayAction::ShowMini, "Show mini window"),
            (TrayAction::SafeNow, "Safe Now"),
            (TrayAction::Gaming, "Gaming"),
            (TrayAction::Music, "Music"),
            (TrayAction::Refresh, "Refresh"),
            (TrayAction::Quit, "Quit"),
        ]
    );
}

#[test]
fn tray_menu_maps_actions_to_ui_commands_and_window_actions() {
    let mut mini_window = MiniWindowMode::default();
    let mut quick_actions = QuickActions::default();
    let model = TrayMenuModel::default();

    assert_eq!(
        model.handle_action(TrayAction::SafeNow, &mut mini_window, &mut quick_actions),
        vec![UiCommand::ApplyScene(UiScene::safe_now())]
    );
    assert_eq!(
        model.handle_action(TrayAction::Refresh, &mut mini_window, &mut quick_actions),
        vec![UiCommand::Refresh]
    );
    assert_eq!(
        model.handle_action(TrayAction::ShowMini, &mut mini_window, &mut quick_actions),
        vec![UiCommand::ApplyWindow(WindowAction::MiniSize)]
    );
    assert!(mini_window.is_mini());
    assert_eq!(quick_actions.view_mode(), AppViewMode::QuickActions);
    assert_eq!(
        model.handle_action(TrayAction::ShowFull, &mut mini_window, &mut quick_actions),
        vec![UiCommand::ApplyWindow(WindowAction::NormalSize)]
    );
}

#[test]
fn volume_debouncer_waits_until_channel_has_been_idle() {
    let mut debouncer = VolumeDebouncer::new(Duration::from_millis(150));

    debouncer.queue(ChannelName::Music, 10, Duration::from_millis(0));
    debouncer.queue(ChannelName::Music, 20, Duration::from_millis(50));

    assert_eq!(
        debouncer.drain_ready(Duration::from_millis(149)),
        Vec::new()
    );
    assert_eq!(
        debouncer.drain_ready(Duration::from_millis(200)),
        vec![UiCommand::Send(PersonalCommand::SetVolume(
            ChannelName::Music,
            20
        ))]
    );
}

#[test]
fn volume_debouncer_coalesces_per_channel_independently() {
    let mut debouncer = VolumeDebouncer::new(Duration::from_millis(100));

    debouncer.queue(ChannelName::Music, 10, Duration::from_millis(0));
    debouncer.queue(ChannelName::Game, 30, Duration::from_millis(25));
    debouncer.queue(ChannelName::Music, 20, Duration::from_millis(50));

    assert_eq!(
        debouncer.drain_ready(Duration::from_millis(130)),
        vec![UiCommand::Send(PersonalCommand::SetVolume(
            ChannelName::Game,
            30
        ))]
    );
    assert_eq!(
        debouncer.drain_ready(Duration::from_millis(150)),
        vec![UiCommand::Send(PersonalCommand::SetVolume(
            ChannelName::Music,
            20
        ))]
    );
}

#[test]
fn system_settings_page_exposes_safe_daily_device_controls() {
    let mut quick = QuickActions::default();
    quick.set_view_mode(AppViewMode::System);
    assert_eq!(quick.view_mode(), AppViewMode::System);

    let actions = SystemSettingsAction::daily_controls();
    assert_eq!(actions.len(), 18);
    assert!(
        actions
            .iter()
            .any(|action| action.command() == PersonalCommand::SetMuteHoldDuration(500))
    );
    assert!(
        actions
            .iter()
            .any(|action| action.command() == PersonalCommand::SetCoughIsHold(true))
    );
    assert!(
        actions
            .iter()
            .any(|action| action.command() == PersonalCommand::SetCoughIsHold(false))
    );
    assert!(actions.iter().any(|action| action.command()
        == PersonalCommand::SetCoughMuteFunction(MuteFunction::ToVoiceChat)));
    assert!(
        actions.iter().any(|action| action.command()
            == PersonalCommand::SetCoughMuteFunction(MuteFunction::ToStream))
    );
    assert!(
        actions
            .iter()
            .any(|action| action.command()
                == PersonalCommand::SetCoughMuteFunction(MuteFunction::All))
    );
    assert!(
        actions
            .iter()
            .any(|action| action.command() == PersonalCommand::SetVCMuteAlsoMuteCM(true))
    );
    assert!(
        actions
            .iter()
            .any(|action| action.command() == PersonalCommand::SetMonitorWithFx(true))
    );
    assert!(
        actions
            .iter()
            .any(|action| action.command() == PersonalCommand::SetLockFaders(true))
    );
    assert!(
        actions
            .iter()
            .any(|action| action.command() == PersonalCommand::SetVodMode(VodMode::StreamNoMusic))
    );
    assert!(
        actions
            .iter()
            .all(|action| !action.label().is_empty() && !action.description().is_empty())
    );
}

#[test]
fn main_profile_actions_are_guarded_named_slot_workflows() {
    assert!(SystemLayoutPolicy::uses_guarded_main_profile_actions());
    assert_eq!(SystemLayoutPolicy::profile_panel_width(), 420.0);
    assert_eq!(SystemLayoutPolicy::profile_button_width(), 150.0);

    let actions = MainProfileAction::guarded_daily_actions("Personal");
    assert_eq!(actions.len(), 5);
    assert!(actions.iter().all(MainProfileAction::requires_confirmation));
    assert!(
        actions
            .iter()
            .all(|action| !action.description().is_empty())
    );

    let load = actions
        .iter()
        .find(|action| {
            action.command() == PersonalCommand::LoadProfile("Personal".to_string(), true)
        })
        .expect("load profile action");
    assert_eq!(load.label(), "Load Personal");
    assert_eq!(
        load.command_if_confirmed(false),
        None,
        "profile load must require an explicit second click"
    );
    assert_eq!(
        load.command_if_confirmed(true),
        Some(PersonalCommand::LoadProfile("Personal".to_string(), true))
    );

    assert!(
        actions
            .iter()
            .any(|action| action.command() == PersonalCommand::SaveProfile)
    );
    assert!(
        actions.iter().any(
            |action| action.command() == PersonalCommand::SaveProfileAs("Personal".to_string())
        )
    );
    assert!(
        actions
            .iter()
            .any(|action| action.command() == PersonalCommand::NewProfile("Personal".to_string()))
    );
    assert!(
        actions.iter().any(
            |action| action.command() == PersonalCommand::DeleteProfile("Personal".to_string())
        )
    );
}

#[test]
fn profile_browser_lists_available_files_and_builds_guarded_row_actions() {
    let root = temp_scene_config_path("profile-browser");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("Broadcast.goxlr"), "{}").unwrap();
    fs::write(root.join("Personal.goxlr"), "{}").unwrap();
    fs::write(root.join("ignore.txt"), "nope").unwrap();

    let browser = ProfileBrowser::from_directory(
        ProfileBrowserKind::Main,
        Some("Personal"),
        Some(root.as_path()),
    );
    assert_eq!(browser.title(), "Profile browser");
    assert_eq!(browser.rows().len(), 2);
    assert_eq!(browser.rows()[0].name(), "Broadcast");
    assert!(!browser.rows()[0].is_active());
    assert_eq!(browser.rows()[1].name(), "Personal");
    assert!(browser.rows()[1].is_active());

    let personal_actions = browser.rows()[1].actions();
    assert!(personal_actions.iter().any(|action| {
        action.label() == "Load"
            && action.command() == PersonalCommand::LoadProfile("Personal".to_string(), true)
    }));
    assert!(personal_actions.iter().any(|action| {
        action.label() == "Load lighting"
            && action.command() == PersonalCommand::LoadProfileColours("Personal".to_string())
    }));
    assert!(personal_actions.iter().any(|action| {
        action.label() == "Delete"
            && action.command() == PersonalCommand::DeleteProfile("Personal".to_string())
            && action.requires_confirmation()
    }));

    fs::remove_dir_all(root).ok();
}

#[test]
fn profile_browser_supports_mic_and_effect_preset_workflows() {
    let mic = ProfileBrowser::from_names(
        ProfileBrowserKind::Mic,
        Some("Broadcast"),
        vec!["Broadcast".to_string(), "Podcast".to_string()],
    );
    assert_eq!(mic.rows()[0].kind(), ProfileBrowserKind::Mic);
    assert!(mic.rows()[0].actions().iter().any(|action| {
        action.command() == PersonalCommand::LoadMicProfile("Broadcast".to_string(), true)
    }));
    assert!(mic.rows()[0].actions().iter().any(|action| {
        action.command() == PersonalCommand::DeleteMicProfile("Broadcast".to_string())
    }));

    let effects = ProfileBrowser::from_names(
        ProfileBrowserKind::EffectsPreset,
        None,
        vec!["Big Verb".to_string()],
    );
    let actions = effects.rows()[0].actions();
    assert!(actions.iter().any(
        |action| action.command() == PersonalCommand::LoadEffectPreset("Big Verb".to_string())
    ));
    assert!(actions.iter().any(|action| {
        action.command() == PersonalCommand::RenameActiveEffectPreset("Big Verb".to_string())
    }));
}

#[test]
fn main_profile_actions_map_to_backend_commands() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::NewProfile("Personal".to_string())),
        goxlr_ipc::GoXLRCommand::NewProfile(profile) if profile == "Personal"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::LoadProfile("Personal".to_string(), true)),
        goxlr_ipc::GoXLRCommand::LoadProfile(profile, true) if profile == "Personal"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SaveProfile),
        goxlr_ipc::GoXLRCommand::SaveProfile()
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SaveProfileAs("Personal".to_string())),
        goxlr_ipc::GoXLRCommand::SaveProfileAs(profile) if profile == "Personal"
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::DeleteProfile("Personal".to_string())),
        goxlr_ipc::GoXLRCommand::DeleteProfile(profile) if profile == "Personal"
    ));
}

#[test]
fn system_settings_commands_map_to_backend_device_settings() {
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetMuteHoldDuration(750)),
        goxlr_ipc::GoXLRCommand::SetMuteHoldDuration(750)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetVCMuteAlsoMuteCM(true)),
        goxlr_ipc::GoXLRCommand::SetVCMuteAlsoMuteCM(true)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetMonitorWithFx(false)),
        goxlr_ipc::GoXLRCommand::SetMonitorWithFx(false)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetLockFaders(true)),
        goxlr_ipc::GoXLRCommand::SetLockFaders(true)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetCoughIsHold(true)),
        goxlr_ipc::GoXLRCommand::SetCoughIsHold(true)
    ));
    assert!(matches!(
        goxlr_ipc::GoXLRCommand::from(PersonalCommand::SetCoughMuteFunction(
            MuteFunction::ToStream
        )),
        goxlr_ipc::GoXLRCommand::SetCoughMuteFunction(MuteFunction::ToStream)
    ));
}

#[test]
fn system_layout_policy_keeps_settings_cards_compact_and_safe() {
    assert!(SystemLayoutPolicy::panel_width() >= 360.0);
    assert!(SystemLayoutPolicy::panel_width() <= 420.0);
    assert!(SystemLayoutPolicy::button_width() >= ContentLayoutPolicy::min_action_button_width());
    assert!(SystemLayoutPolicy::uses_wrapped_cards());
    assert!(SystemLayoutPolicy::destructive_actions_are_omitted_from_daily_controls());
}

#[test]
fn disconnected_snapshot_has_human_readable_status() {
    let snapshot = AppSnapshot::disconnected("daemon unavailable");

    assert!(!snapshot.connected);
    assert_eq!(snapshot.device_serial.as_deref(), None);
    assert_eq!(snapshot.status_line(), "Disconnected: daemon unavailable");
}

#[test]
fn device_selection_selects_first_available_device_by_default() {
    let mut selection = DeviceSelection::default();

    selection.sync_available_devices(vec!["serial-b".to_string(), "serial-a".to_string()]);

    assert_eq!(selection.selected_serial(), Some("serial-a"));
    assert_eq!(selection.available_serials(), vec!["serial-a", "serial-b"]);
}

#[test]
fn device_selection_keeps_selected_device_when_still_available() {
    let mut selection = DeviceSelection::default();
    selection.sync_available_devices(vec!["serial-a".to_string(), "serial-b".to_string()]);
    selection.select_serial("serial-b");

    selection.sync_available_devices(vec!["serial-b".to_string(), "serial-c".to_string()]);

    assert_eq!(selection.selected_serial(), Some("serial-b"));
    assert_eq!(selection.available_serials(), vec!["serial-b", "serial-c"]);
}

#[test]
fn device_selection_falls_back_when_selected_device_disappears() {
    let mut selection = DeviceSelection::default();
    selection.sync_available_devices(vec!["serial-a".to_string(), "serial-b".to_string()]);
    selection.select_serial("serial-b");

    selection.sync_available_devices(vec!["serial-c".to_string()]);

    assert_eq!(selection.selected_serial(), Some("serial-c"));
}
