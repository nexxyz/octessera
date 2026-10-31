use super::*;

#[test]
pub(crate) fn fast_dispatch_preserves_instrument_dynamic_and_structural_precedence() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let dynamic_key = "instruments.0.synth.amp.gainPct";
    assert!(runner.menu.focus_item_key(dynamic_key));
    runner.menu.turn_key(dynamic_key, -10);
    runner.apply_or_schedule_menu_key(dynamic_key).unwrap();

    assert_eq!(runner.instruments[0].synth_gain_pct, 70);
    assert_eq!(runner.audio_config_revision, 0);

    let structural_key = "instruments.0.type";
    assert!(runner.menu.focus_item_key(structural_key));
    runner.menu.turn_key(structural_key, 1);
    runner.apply_or_schedule_menu_key(structural_key).unwrap();

    assert_eq!(runner.instruments[0].kind, "sampler");
    assert_eq!(runner.audio_config_revision, 0);
    assert!(runner.pending.pending_menu_apply.is_none());
}

#[test]
pub(crate) fn grouped_runtime_state_keeps_transport_snapshot_and_deferred_flush_parity() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.transport.current_ppqn_pulse = 48;
    runner.pending.pending_autosave_payload_due_at = Some(std::time::Instant::now());

    let messages = runner.flush_deferred_menu_apply().unwrap();
    let status = messages.iter().find_map(|message| match message {
        RunnerMessage::RuntimeStatus { status } => Some(status),
        _ => None,
    });

    assert!(status.is_some_and(|status| {
        status.transport == RuntimeTransportState::Playing && status.current_ppqn_pulse == 48
    }));
    assert!(runner.pending.pending_autosave_payload_due_at.is_none());
}

#[test]
pub(crate) fn host_dispatch_preserves_effect_audio_snapshot_status_order() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .outbox
        .push_platform_effect(RuntimePlatformEffect::MidiPanic);
    runner
        .outbox
        .push_audio_command(RuntimeAudioCommand::SamplePreview {
            instrument_slot: 0,
            sample_slot: 0,
            path: "kick.wav".into(),
            velocity: 100,
        });

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "other" }),
            request_snapshot: Some(true),
        })
        .unwrap();

    let platform_index = messages
        .iter()
        .position(|message| matches!(message, RunnerMessage::PlatformEffects { .. }))
        .unwrap();
    let audio_index = messages
        .iter()
        .position(|message| matches!(message, RunnerMessage::AudioCommands { .. }))
        .unwrap();
    let snapshot_index = messages
        .iter()
        .position(|message| matches!(message, RunnerMessage::Snapshot { .. }))
        .unwrap();
    let status_index = messages
        .iter()
        .position(|message| matches!(message, RunnerMessage::RuntimeStatus { .. }))
        .unwrap();

    assert!(platform_index < audio_index);
    assert!(audio_index < snapshot_index);
    assert!(snapshot_index < status_index);
}

#[test]
pub(crate) fn input_dispatch_owns_temporary_snapshot_suppression() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "other" }),
            request_snapshot: Some(false),
        })
        .unwrap();

    assert!(!messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::Snapshot { .. })));
    assert!(messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::RuntimeStatus { .. })));
    assert!(!runner.pending.suppress_snapshot_response);
}

#[test]
pub(crate) fn level_edits_read_the_focused_row_instead_of_searching_the_menu_tree() {
    for key in [
        "displayBrightness",
        "buttonBrightness",
        "gridBrightness",
        "masterVolume",
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        assert!(runner.menu.focus_item_key(key));
        runner.menu.turn_key(key, -3);
        let focused = runner.menu.number_for_key(key).unwrap();
        let mut decoy = runner.menu.item_for_key(key).unwrap();
        let crate::native_menu::NativeMenuValue::Number { value, .. } = &mut decoy.value else {
            panic!("{key} is not a number row");
        };
        *value = if focused == 50 { 60 } else { 50 };
        runner.menu.root.children.insert(0, decoy);
        runner.menu.state.stack[0] += 1;

        runner.apply_or_schedule_menu_key(key).unwrap();

        let ui = &runner.display.ui;
        let applied = match key {
            "displayBrightness" => ui.display_brightness,
            "buttonBrightness" => ui.button_brightness,
            "gridBrightness" => ui.grid_brightness,
            _ => ui.master_volume,
        };
        assert_eq!(i32::from(applied), focused, "{key}");
    }
}
