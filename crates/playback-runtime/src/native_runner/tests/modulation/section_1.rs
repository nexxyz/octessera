use super::*;

#[test]
pub(crate) fn pulses_value_lanes_load_into_runner_and_menu_curve_edits_apply() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["layers"][0]["pulses"]["x"]["velocity"] = json!({
        "enabled": true,
        "from": 12,
        "to": 99,
        "gridOffset": 2,
        "curve": "curve"
    });
    payload["runtimeConfig"]["layers"][0]["pulses"]["y"]["filterResonance"] = json!({
        "enabled": true,
        "from": 3,
        "to": 77,
        "gridOffset": -1,
        "curve": "linear"
    });

    runner.apply_config_payload(payload).unwrap();

    assert!(runner.pulses_layers[0].x_velocity.enabled);
    assert_eq!(runner.pulses_layers[0].x_velocity.from, 12);
    assert_eq!(runner.pulses_layers[0].x_velocity.to, 99);
    assert_eq!(runner.pulses_layers[0].x_velocity.grid_offset, 2);
    assert_eq!(runner.pulses_layers[0].x_velocity.curve, "curve");
    assert!(runner.pulses_layers[0].y_filter_resonance.enabled);

    runner.menu.rebuild(runner.menu_config());
    runner.menu.turn_key("layers.0.pulses.x.velocity.curve", -1);
    runner.apply_menu_state().unwrap();
    assert_eq!(runner.pulses_layers[0].x_velocity.curve, "linear");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["layers"][0]["pulses"]["x"]["velocity"]["curve"],
        "linear"
    );
}

#[test]
pub(crate) fn link_lfo_payload_defaults_round_trips_and_rejects_non_live_target() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let payload = runner.config_payload();
    assert_eq!(
        payload["runtimeConfig"]["layers"][0]["linkLfo"]["enabled"],
        false
    );
    assert!(payload["runtimeConfig"]["layers"][0]["linkLfo"]["target"].is_null());
    assert_eq!(
        payload["runtimeConfig"]["layers"][0]["linkLfo"]["period"],
        "1/1"
    );
    assert_eq!(
        payload["runtimeConfig"]["layers"][0]["linkLfo"]["depthPct"],
        100
    );

    let mut payload = payload;
    payload["runtimeConfig"]["layers"][0]["linkLfo"] = json!({
        "enabled": true,
        "target": { "key": "sound.noteLengthMs", "kind": "number", "min": 30, "max": 2000, "userMin": 100, "userMax": 300 },
        "period": "1/4",
        "depthPct": 100
    });
    runner.apply_config_payload(payload).unwrap();
    assert!(!runner.pulses_layers[0].link_lfo.enabled);

    let round_trip = runner.config_payload();
    assert_eq!(
        round_trip["runtimeConfig"]["layers"][0]["linkLfo"]["enabled"],
        false
    );
    runner.transport = RuntimeTransportState::Playing;
    let before = runner.config_payload();
    runner.advance_algorithm(6).unwrap();
    assert_eq!(runner.config_payload(), before);
    runner.transport = RuntimeTransportState::Paused;
    runner.advance_algorithm(6).unwrap();
    assert_eq!(runner.pulses_layers[0].link_lfo.phase_pulses, 0);
    runner.reset_transport_position();
    assert_eq!(runner.pulses_layers[0].link_lfo.phase_pulses, 0);

    let mut legacy = runner.config_payload();
    legacy["runtimeConfig"]["layers"][0]
        .as_object_mut()
        .unwrap()
        .remove("linkLfo");
    runner.apply_config_payload(legacy).unwrap();
    assert!(!runner.pulses_layers[0].link_lfo.enabled);
    assert_eq!(runner.pulses_layers[0].link_lfo.period, "1/1");
    assert_eq!(runner.pulses_layers[0].link_lfo.depth_pct, 100);
}

#[test]
pub(crate) fn link_lfo_rejects_non_numeric_targets() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["layers"][0]["linkLfo"] = json!({
        "enabled": true,
        "target": { "key": "sound.voiceStealingMode", "kind": "enum", "options": ["none"] },
        "period": "1/1",
        "depthPct": 100
    });
    runner.apply_config_payload(payload).unwrap();
    assert!(!runner.pulses_layers[0].link_lfo.enabled);
    assert!(runner.pulses_layers[0].link_lfo.target.is_none());

    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["layers"][0]["linkLfo"] = json!({
        "enabled": true,
        "target": { "key": "layers.0.linkLfo.depthPct", "kind": "number", "min": 0, "max": 100 },
        "period": "1/1",
        "depthPct": 100
    });
    runner.apply_config_payload(payload).unwrap();
    assert!(!runner.pulses_layers[0].link_lfo.enabled);
    assert!(runner.pulses_layers[0].link_lfo.target.is_none());
}

#[test]
pub(crate) fn link_lfo_menu_rows_apply_with_keyed_fast_path() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.pulses_layers[0].link_lfo.target = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("layers.0.linkLfo.depthPct"));
    runner.menu.state.editing = true;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -25, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.pulses_layers[0].link_lfo.depth_pct, 75);

    assert!(runner.menu.focus_item_key("layers.0.linkLfo.period"));
    runner.menu.state.editing = true;
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.pulses_layers[0].link_lfo.period, "1/1T");

    assert!(runner
        .menu
        .focus_item_key("layers.0.linkLfo.target.rangeMin"));
    runner.menu.state.editing = true;
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 7, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(
        runner.pulses_layers[0]
            .link_lfo
            .target
            .as_ref()
            .unwrap()
            .user_min,
        Some(7.0)
    );
}

#[test]
pub(crate) fn link_lfo_invert_and_audio_command_emit_on_transport_pulse() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["layers"][0]["linkLfo"] = json!({
        "enabled": true,
        "target": {
            "key": "instruments.0.mixer.volume",
            "label": "Volume",
            "kind": "number",
            "min": 0,
            "max": 100,
            "step": 1,
            "invert": true
        },
        "period": "1/4",
        "depthPct": 100
    });
    runner.apply_config_payload(payload).unwrap();
    runner.transport = RuntimeTransportState::Playing;

    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert_eq!(runner.instruments[0].volume, 100);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetInstrumentMixer {
                    instrument_slot: 0,
                    volume_pct: Some(volume),
                    pan_pos: None,
                } if (*volume - 0.0).abs() < f32::EPSILON
            ))
    )));
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveDefault { .. }
            ))
    )));
}

#[test]
pub(crate) fn link_lfo_fx_bus_volume_emits_fast_mixer_command() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["layers"][0]["linkLfo"] = json!({
        "enabled": true,
        "target": {
            "key": "mixer.buses.0.volume",
            "label": "Volume",
            "kind": "number",
            "min": 0,
            "max": 100,
            "step": 1,
            "invert": true
        },
        "period": "1/4",
        "depthPct": 100
    });
    runner.apply_config_payload(payload).unwrap();
    runner.transport = RuntimeTransportState::Playing;

    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 6,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert_eq!(runner.fx_buses[0].volume_pct, 100);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusMixer {
                    bus_index: 0,
                    pan_pos: None,
                    volume_pct: Some(volume),
                } if (*volume - 0.0).abs() < f32::EPSILON
            ))
    )));
}

#[test]
pub(crate) fn link_lfo_fx_param_is_transient_and_suppresses_repeated_values() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.auto_save_default = true;
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot3"] =
        json!({ "type": "delay", "params": { "feedback": 0.25, "timeMs": 250, "mixPct": 40 } });
    payload["runtimeConfig"]["layers"][0]["linkLfo"] = json!({
        "enabled": true,
        "target": {
            "key": "mixer.buses.0.slot3.params.feedback",
            "label": "Feedback",
            "kind": "number",
            "min": 0,
            "max": 100,
            "step": 1
        },
        "period": "1/4",
        "depthPct": 0
    });
    runner.apply_config_payload(payload).unwrap();
    runner.transport = RuntimeTransportState::Playing;
    let before = runner.config_payload();

    let messages = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    assert_eq!(runner.config_payload(), before);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands }
            if commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot { bus_index: 0, slot_index: 2, params, .. }
                    if params.get("feedback") == Some(&json!(0.5))
            ))
    )));
    let repeat = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(!repeat.iter().any(|message| matches!(
        message,
        RunnerMessage::AudioCommands { commands } if commands.iter().any(|command| matches!(
            command,
            RuntimeAudioCommand::SetFxBusSlot { bus_index: 0, slot_index: 2, .. }
        ))
    )));
}

#[test]
pub(crate) fn link_lfo_transport_reset_restores_base_fx_param() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot3"] =
        json!({ "type": "delay", "params": { "feedback": 0.25, "timeMs": 250, "mixPct": 40 } });
    payload["runtimeConfig"]["layers"][0]["linkLfo"] = json!({
        "enabled": true,
        "target": { "key": "mixer.buses.0.slot3.params.feedback", "kind": "number", "min": 0, "max": 100, "step": 1 },
        "period": "1/4",
        "depthPct": 0
    });
    runner.apply_config_payload(payload).unwrap();
    runner.transport = RuntimeTransportState::Playing;
    let _ = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();

    runner.reset_transport_position();
    let commands = runner.outbox.drain_audio_commands();

    assert!(commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetFxBusSlot { bus_index: 0, slot_index: 2, params, .. }
            if params.get("feedback") == Some(&json!(0.25))
    )));
}

#[test]
pub(crate) fn link_lfo_config_change_restores_base_and_rejects_unsafe_fx_targets() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut payload = runner.config_payload();
    payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"] =
        json!({ "type": "delay", "params": { "feedback": 0.25, "timeMs": 250, "mixPct": 40 } });
    payload["runtimeConfig"]["layers"][0]["linkLfo"] = json!({
        "enabled": true,
        "target": { "key": "mixer.buses.0.slot1.params.feedback", "kind": "number", "min": 0, "max": 100, "step": 1 },
        "period": "1/4",
        "depthPct": 0
    });
    runner.apply_config_payload(payload).unwrap();
    runner.transport = RuntimeTransportState::Playing;
    let _ = runner
        .send(HostMessage::TransportPulseStep {
            pulses: 1,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
    let mut next = runner.config_payload();
    next["runtimeConfig"]["layers"][0]["linkLfo"] = json!({
        "enabled": true,
        "target": { "key": "mixer.buses.0.slot1.params.timeMs", "kind": "number", "min": 1, "max": 2000, "step": 1 },
        "period": "1/4",
        "depthPct": 100
    });

    runner.apply_config_payload(next).unwrap();
    let commands = runner.outbox.drain_audio_commands();

    assert!(commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetFxBusSlot { bus_index: 0, slot_index: 0, params, .. }
            if params.get("feedback") == Some(&json!(0.25))
    )));
    assert!(!runner.pulses_layers[0].link_lfo.enabled);
    assert!(runner.pulses_layers[0].link_lfo.target.is_none());

    let mut source_runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut source_payload = source_runner.config_payload();
    source_payload["runtimeConfig"]["mixer"]["buses"][0]["slot1"] = json!({
        "type": "duck",
        "params": { "source": "I1", "threshold": 0.08, "amountPct": 60, "attackMs": 8, "releaseMs": 160 }
    });
    source_payload["runtimeConfig"]["layers"][0]["linkLfo"] = json!({
        "enabled": true,
        "target": { "key": "mixer.buses.0.slot1.params.source", "kind": "number", "min": 0, "max": 100, "step": 1 },
        "period": "1/4",
        "depthPct": 100
    });

    source_runner.apply_config_payload(source_payload).unwrap();

    assert!(!source_runner.pulses_layers[0].link_lfo.enabled);
    assert!(source_runner.pulses_layers[0].link_lfo.target.is_none());
}

#[test]
pub(crate) fn link_lfo_action_rejects_unsafe_target_without_mapped_toast() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.set_param_binding_target(
        "layers.0.linkLfo.target",
        Some(NativeParamBinding {
            key: "instruments.0.synth.filter.cutoffHz".into(),
            label: Some("Cutoff".into()),
            kind: "number".into(),
            min: Some(0.0),
            max: Some(127.0),
            step: Some(1.0),
            user_min: None,
            user_max: None,
            options: vec![],
            invert: false,
        }),
    );

    assert!(runner.pulses_layers[0].link_lfo.target.is_none());
    assert!(runner
        .toast
        .as_ref()
        .is_some_and(|toast| toast.message == "LFO target not live"));
}
