use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use goxlr_personal_ui::{
    ActiveAudioStreams, AppConfig, AppSceneConfig, AppSnapshot, AppViewMode, AudioRouteTarget,
    AudioRoutingRule, ControlledChannel, DashboardCopy, DeviceSelection, ExternalAudioTool,
    MiniWindowMode, OptionalBoolAction, PersonalCommand, QuickActions, RoutingRuleEditor,
    SceneEditor, TrayAction, TrayMenuModel, UiCommand, UiScene, VolumeDebouncer, WindowAction,
    ipc_socket_path_candidates,
};
use goxlr_types::{
    ChannelName, CompressorAttackTime, CompressorRatio, CompressorReleaseTime, GateTimes,
    MicrophoneType,
};

fn temp_scene_config_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("goxlr-personal-ui-{name}-{nonce}.json"))
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
fn app_view_mode_has_dedicated_mic_page_for_parity_chunks() {
    let mut quick_actions = QuickActions::default();
    quick_actions.set_view_mode(AppViewMode::Mic);

    assert_eq!(quick_actions.view_mode(), AppViewMode::Mic);
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
