use super::*;

pub(super) fn playing_default() -> NativeRunner {
    let payload: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    for pressed in [true, false] {
        runner
            .send(HostMessage::DeviceInput {
                input: json!({"type": "button_s", "pressed": pressed}),
                request_snapshot: None,
            })
            .unwrap();
    }
    assert_eq!(runner.transport.transport, RuntimeTransportState::Playing);
    runner
}

pub(super) fn assert_music_only(messages: &[RunnerMessage]) {
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::Snapshot { .. } | RunnerMessage::OledFrame { .. }
    )));
    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::RuntimeStatus { .. })));
}

#[test]
fn playing_pulses_zero_xy_and_aux_edit_defer_presentation_without_losing_autosave() {
    let mut runner = playing_default();
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("instruments.0.synth.osc1.levelPct".into()),
        press_action: None,
    });
    runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type": "button_fn", "pressed": true}),
            request_snapshot: Some(false),
        })
        .unwrap();
    let marks = runner.fast_autosave_marks;
    let result = runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type": "encoder_turn", "id": "aux1", "delta": 2}),
            request_snapshot: Some(false),
        })
        .unwrap();
    runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type": "button_fn", "pressed": false}),
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_music_only(&result);
    assert!(runner.display_scene_pending());
    assert_eq!(runner.instruments[0].synth_config["osc1"]["levelPct"], 82);
    assert!(runner.fast_autosave_marks > marks);
    assert!(runner.pending.pending_autosave_payload_due_at.is_some());
    let menu = runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type": "encoder_press", "id": "main"}),
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_music_only(&menu);
    assert!(runner.display_scene_pending());
    let grid = runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type": "grid_press", "x": 3, "y": 3}),
            request_snapshot: None,
        })
        .unwrap();
    assert_music_only(&grid);
    for pulses in [0, 24, 0] {
        let messages = runner
            .send_music_first(HostMessage::TransportPulseStep {
                pulses,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(true),
            })
            .unwrap();
        assert_music_only(&messages);
    }
}

#[test]
fn playing_main_press_help_open_close_and_scalar_edit_are_presentation_deferred() {
    let mut runner = playing_default();
    for (index, input) in [
        json!({"type":"button_shift","pressed":true}),
        json!({"type":"button_fn","pressed":true}),
        json!({"type":"encoder_press","id":"main"}),
    ]
    .into_iter()
    .enumerate()
    {
        let messages = runner
            .send_music_first(HostMessage::DeviceInput {
                input,
                request_snapshot: Some(false),
            })
            .unwrap();
        assert!(
            !messages
                .iter()
                .any(|message| matches!(message, RunnerMessage::Snapshot { .. })),
            "help flow index={index}: {messages:?}"
        );
    }
    assert_eq!(
        runner.display.help_popup.as_ref().unwrap().title,
        "Help: Build"
    );
    assert!(runner.display_scene_pending());
    for (index, input) in [
        json!({"type":"button_fn","pressed":false}),
        json!({"type":"button_shift","pressed":false}),
        json!({"type":"encoder_press","id":"main"}),
    ]
    .into_iter()
    .enumerate()
    {
        let messages = runner
            .send_music_first(HostMessage::DeviceInput {
                input,
                request_snapshot: Some(false),
            })
            .unwrap();
        assert!(
            !messages
                .iter()
                .any(|message| matches!(message, RunnerMessage::Snapshot { .. })),
            "help close index={index}: {messages:?}"
        );
    }
    assert!(runner.display.help_popup.is_none());

    for _ in 0..4 {
        let turn = runner
            .send_music_first(HostMessage::DeviceInput {
                input: json!({"type":"encoder_turn","id":"main","delta":1}),
                request_snapshot: Some(false),
            })
            .unwrap();
        assert_music_only(&turn);
    }
    assert_eq!(runner.menu.current_label(), Some("System"));
    let enter_system = runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type":"encoder_press","id":"main"}),
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_music_only(&enter_system);
    for _ in 0..4 {
        runner
            .send_music_first(HostMessage::DeviceInput {
                input: json!({"type":"encoder_turn","id":"main","delta":1}),
                request_snapshot: Some(false),
            })
            .unwrap();
    }
    assert_eq!(runner.menu.current_label(), Some("Audio"));
    let enter_audio = runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type":"encoder_press","id":"main"}),
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_music_only(&enter_audio);
    assert_eq!(runner.menu.current_label(), Some("Master Vol"));
    let enter_edit = runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type":"encoder_press","id":"main"}),
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_music_only(&enter_edit);
    let volume_before = runner.display.ui.master_volume;
    let edit = runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type":"encoder_turn","id":"main","delta":1}),
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_music_only(&edit);
    assert!(edit
        .iter()
        .any(|message| matches!(message, RunnerMessage::AudioCommands { .. })));
    assert_ne!(runner.display.ui.master_volume, volume_before);
    let leave = runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type":"encoder_press","id":"main"}),
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_music_only(&leave);
}

#[test]
fn music_first_queues_initial_config_before_pulse_notes_and_preserves_display_edges() {
    let mut runner = playing_default();
    let now = Instant::now();
    runner.test_set_display_time(now);
    runner.audio_config_revision += 1;
    let messages = runner
        .send_music_first(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: None,
        })
        .unwrap();
    assert_music_only(&messages);
    let config = messages.iter().position(|message| matches!(message, RunnerMessage::AudioCommands { commands }
        if commands.iter().any(|command| matches!(command, RuntimeAudioCommand::SetAudioConfig { .. })))).unwrap();
    let note = messages
        .iter()
        .position(|message| {
            matches!(
                message,
                RunnerMessage::MusicalEvents { .. } | RunnerMessage::DrumHits { .. }
            )
        })
        .unwrap();
    assert!(config < note);
    assert!(runner.display.transients.snapshot_pending());
    assert!(runner.display_scene_pending());
    let shown_scene = runner.capture_display_scene().unwrap();
    let shown_generation = shown_scene.generation();
    let shown = shown_scene.into_snapshot();
    assert_eq!(shown["transportFlash"], "beat");
    assert_eq!(shown["eventDotOn"], true);
    assert!(runner.display.transients.snapshot_pending());
    runner.acknowledge_display_scene(shown_generation);
    assert!(!runner.display.transients.snapshot_pending());
    assert!(!runner.display_scene_pending());
    runner.test_set_display_time(now + Duration::from_millis(200));
    let expired = runner
        .send_music_first(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: None,
        })
        .unwrap();
    assert_music_only(&expired);
    assert!(runner.display.transients.snapshot_pending());
    let scene = runner.capture_display_scene().unwrap().into_snapshot();
    assert_eq!(scene["transportFlash"], "none");
    assert_eq!(scene["eventDotOn"], false);
    let prior_revision = runner.last_snapshot_audio_config_revision;
    runner.audio_config_revision += 1;
    let _ = runner.capture_display_scene().unwrap();
    assert_eq!(runner.last_snapshot_audio_config_revision, prior_revision);
}

#[test]
fn explicit_request_survives_capture_until_scene_is_accepted() {
    let mut runner = playing_default();
    let initial = runner.capture_display_scene().unwrap();
    runner.acknowledge_display_scene(initial.generation());
    assert!(!runner.display_scene_pending());
    let suppressed = runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type": "encoder_turn", "id": "main", "delta": 0}),
            request_snapshot: Some(false),
        })
        .unwrap();
    assert_music_only(&suppressed);
    assert!(!runner.display_scene_pending());
    let messages = runner
        .send_music_first(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    assert_music_only(&messages);
    assert!(runner.display_scene_pending());
    let requested = runner.capture_display_scene().unwrap();
    assert!(runner.display_scene_pending());
    runner.acknowledge_display_scene(requested.generation());
    assert!(!runner.display_scene_pending());
}

#[test]
fn external_clock_resync_uses_music_first_only_while_already_playing() {
    let mut runner = playing_default();
    let mut legacy = playing_default();
    runner.midi_enabled = true;
    runner.midi_clock_in_enabled = true;
    runner.transport.sync_source = SyncSource::External;
    runner.transport.current_ppqn_pulse = 95;
    runner.transport.pending_resync = true;
    let initial = runner.capture_display_scene().unwrap();
    runner.acknowledge_display_scene(initial.generation());
    let generation = runner.display.transients.generation();
    let no_clock = runner
        .send_music_first(HostMessage::MidiRealtimeClock { pulses: 0 })
        .unwrap();
    assert_music_only(&no_clock);
    assert_eq!(runner.display.transients.generation(), generation);
    assert!(!runner.display_scene_pending());
    legacy.midi_enabled = true;
    legacy.midi_clock_in_enabled = true;
    legacy.transport.sync_source = SyncSource::External;
    legacy.transport.current_ppqn_pulse = 95;
    legacy.transport.pending_resync = true;
    let messages = runner
        .send_music_first(HostMessage::MidiRealtimeClock { pulses: 2 })
        .unwrap();
    let expected = legacy
        .send(HostMessage::MidiRealtimeClock { pulses: 2 })
        .unwrap();
    let musical = |messages: Vec<RunnerMessage>| {
        messages
            .into_iter()
            .filter(|message| {
                matches!(
                    message,
                    RunnerMessage::MusicalEvents { .. }
                        | RunnerMessage::MidiEvents { .. }
                        | RunnerMessage::DrumHits { .. }
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(musical(messages.clone()), musical(expected));
    assert_music_only(&messages);
    assert!(!runner.transport.pending_resync);
    assert!(messages.iter().any(|message| matches!(message,
        RunnerMessage::RuntimeStatus { status } if !status.pending_resync && status.current_ppqn_pulse == 1
    )));
    let ignored = runner
        .send_music_first(HostMessage::MidiRealtimeClock { pulses: 0 })
        .unwrap();
    assert_music_only(&ignored);
    let stopped = runner
        .send_music_first(HostMessage::MidiRealtimeStop)
        .unwrap();
    assert!(stopped
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
}

#[test]
fn transport_button_retains_synchronous_fallback_while_playing() {
    let mut runner = playing_default();
    let messages = runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type": "button_s", "pressed": true}),
            request_snapshot: None,
        })
        .unwrap();
    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    assert_ne!(runner.transport.transport, RuntimeTransportState::Playing);
    assert!(!runner.pending.presentation_deferred);
    let mut special = playing_default();
    let action = special
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type": "behavior_action", "actionType": "spawnGlider"}),
            request_snapshot: None,
        })
        .unwrap();
    assert!(action
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
}

#[test]
fn presented_error_keeps_synchronous_input_path_while_playing() {
    let mut runner = playing_default();
    runner.display.runtime_error_presentation = Some(NativeRuntimeErrorPresentation {
        title: "Fault".into(),
        lines: vec!["Try Back".into()],
    });
    let messages = runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type": "encoder_turn", "id": "main", "delta": 1}),
            request_snapshot: None,
        })
        .unwrap();
    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    assert!(!runner.pending.presentation_deferred);
}

#[test]
fn music_first_defer_guard_resets_on_dispatch_failure() {
    let mut runner = playing_default();
    runner.pending.pending_menu_apply = Some(PendingMenuApply {
        due_at: Instant::now(),
        key: "unknown.test.key".into(),
    });
    let failed = runner.send_music_first(HostMessage::TransportPulseStep {
        pulses: 0,
        source: SyncSource::Internal,
        at_ppqn_pulse: None,
        request_snapshot: None,
    });
    assert!(failed.is_err());
    assert!(!runner.pending.presentation_deferred);
    assert!(!runner.pending.external_autosave_deferred);
    let snapshot = runner
        .send(HostMessage::DeviceInput {
            input: json!({"type": "encoder_turn", "id": "aux1", "delta": 1}),
            request_snapshot: None,
        })
        .unwrap();
    assert!(snapshot
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
}

#[test]
fn stale_scene_ack_keeps_new_beat_expiry_and_explicit_request_due() {
    let mut runner = playing_default();
    let start = Instant::now();
    runner.test_set_display_time(start + Duration::from_millis(91));
    let old = runner.capture_display_scene().unwrap();
    let old_token = old.generation();
    runner
        .send_music_first(HostMessage::TransportPulseStep {
            pulses: 24,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    runner.acknowledge_display_scene(old_token);
    assert!(runner.display_scene_pending());
    let beat = runner.capture_display_scene().unwrap();
    let beat_token = beat.generation();
    assert_eq!(beat.into_snapshot()["transportFlash"], "beat");
    runner.acknowledge_display_scene(beat_token);
    assert!(!runner.display_scene_pending());
    runner.test_set_display_time(start + Duration::from_millis(300));
    runner
        .send_music_first(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    runner.acknowledge_display_scene(beat_token);
    assert!(runner.display_scene_pending());
    let expired = runner.capture_display_scene().unwrap();
    let expired_token = expired.generation();
    assert_eq!(expired.into_snapshot()["transportFlash"], "none");
    runner
        .send_music_first(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap();
    runner.acknowledge_display_scene(expired_token);
    assert!(runner.display_scene_pending());
    let requested = runner.capture_display_scene().unwrap();
    runner.acknowledge_display_scene(requested.generation());
    assert!(!runner.display_scene_pending());
    let old_menu = runner.capture_display_scene().unwrap();
    runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type": "encoder_turn", "id": "main", "delta": 1}),
            request_snapshot: Some(false),
        })
        .unwrap();
    runner.acknowledge_display_scene(old_menu.generation());
    assert!(runner.display_scene_pending());
}
