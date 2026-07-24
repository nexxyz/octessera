use super::*;

fn volume_binding() -> NativeParamBinding {
    NativeParamBinding {
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
    }
}

fn fx_binding(key: &str, min: f64, max: f64, step: f64) -> NativeParamBinding {
    NativeParamBinding {
        key: key.into(),
        label: Some(key.into()),
        kind: "number".into(),
        min: Some(min),
        max: Some(max),
        step: Some(step),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    }
}

#[test]
pub(crate) fn patch_transaction_discards_held_xy_source_and_captured_base() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].volume = 50;
    runner.xy_x_binding = Some(volume_binding());
    runner.active_sparks_mode = "xy".into();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(runner.instruments[0].volume, 100);

    let mut patch = runner.config_payload();
    patch["kind"] = json!("octessera.patch");
    patch["schemaVersion"] = json!(2);
    patch["runtimeConfig"]["instruments"][0]["mixer"]["volume"] = json!(30);
    patch["runtimeConfig"]["xy"]["x"] = Value::Null;
    runner.apply_patch_payload_preserving_device(patch).unwrap();

    assert_eq!(runner.instruments[0].volume, 30);
}

#[test]
pub(crate) fn config_transaction_resamples_active_xy_against_candidate_inversion() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].volume = 50;
    runner.xy_touch = NativeXyTouch {
        x: 0.25,
        y: 0.5,
        display_x: 0.25,
        display_y: 0.5,
        active: true,
    };
    let mut config = runner.config_payload();
    config["runtimeConfig"]["xy"]["x"] = json!({
        "key": "instruments.0.mixer.volume",
        "label": "Volume",
        "kind": "number",
        "min": 0,
        "max": 100,
        "step": 1,
        "invert": false
    });
    config["runtimeConfig"]["xy"]["xInvert"] = json!(true);
    runner.apply_config_payload(config).unwrap();

    assert_eq!(runner.instruments[0].volume, 75);
    assert!(runner.xy_touch.active);
}

#[test]
pub(crate) fn config_load_captures_changed_xy_owner_then_clear_restores_loaded_base() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[1].volume = 20;
    runner.xy_touch = NativeXyTouch {
        x: 1.0,
        y: 0.5,
        display_x: 1.0,
        display_y: 0.5,
        active: true,
    };

    let mut config = runner.config_payload();
    config["runtimeConfig"]["instruments"][1]["mixer"]["volume"] = json!(30);
    config["runtimeConfig"]["xy"]["x"] = json!({
        "key": "instruments.1.mixer.volume",
        "label": "Volume",
        "kind": "number",
        "min": 0,
        "max": 100,
        "step": 1,
        "invert": false
    });
    runner.apply_config_payload(config).unwrap();

    assert_eq!(runner.instruments[1].volume, 100);
    runner.set_param_binding_target("xy:x", None);
    assert_eq!(runner.instruments[1].volume, 30);
}

#[test]
pub(crate) fn structural_instrument_modulation_emits_one_slot_command() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "synth".into();
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "instruments.0.synth.filter.type".into(),
        label: Some("Filter Type".into()),
        kind: "enum".into(),
        min: None,
        max: None,
        step: None,
        user_min: None,
        user_max: None,
        options: vec!["lowpass".into(), "highpass".into()],
        invert: false,
    });
    runner.apply_runtime_modulation(
        &[platform_core::CellTriggerIntent {
            x: 7,
            y: 0,
            degree: 0,
            kind: platform_core::CellTriggerKind::Activate,
        }],
        0,
    );
    let commands = runner.outbox.drain_audio_commands();

    assert_eq!(
        commands
            .iter()
            .filter(|command| matches!(
                command,
                RuntimeAudioCommand::SetInstrumentSlot {
                    instrument_slot: 0,
                    ..
                }
            ))
            .count(),
        1
    );
    assert!(!commands
        .iter()
        .any(|command| matches!(command, RuntimeAudioCommand::SetSynthParam { .. })));
}

#[test]
pub(crate) fn supported_synth_modulation_emits_direct_command_and_midi_emits_none() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "synth".into();
    runner.param_mods[0].x[0] = Some(fx_binding(
        "instruments.0.synth.filter.cutoffHz",
        0.0,
        255.0,
        1.0,
    ));
    runner.apply_runtime_modulation(
        &[platform_core::CellTriggerIntent {
            x: 7,
            y: 0,
            degree: 0,
            kind: platform_core::CellTriggerKind::Activate,
        }],
        0,
    );
    let synth_commands = runner.outbox.drain_audio_commands();
    assert!(synth_commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetSynthParam {
            instrument_slot: 0,
            path,
            ..
        } if path == "synth.filter.cutoffHz"
    )));
    assert!(!synth_commands
        .iter()
        .any(|command| matches!(command, RuntimeAudioCommand::SetInstrumentSlot { .. })));

    runner.instruments[0].kind = "midi".into();
    runner.param_mods[0].x[0] = Some(volume_binding());
    runner.apply_runtime_modulation(
        &[platform_core::CellTriggerIntent {
            x: 7,
            y: 0,
            degree: 0,
            kind: platform_core::CellTriggerKind::Activate,
        }],
        0,
    );
    assert!(runner.outbox.drain_audio_commands().is_empty());
}

#[test]
pub(crate) fn synth_to_midi_type_modulation_emits_slot_removal_before_midi_skip() {
    assert_type_modulation_emits_midi_slot_update("synth");
}

#[test]
pub(crate) fn sampler_to_midi_type_modulation_emits_slot_removal_before_midi_skip() {
    assert_type_modulation_emits_midi_slot_update("sampler");
}

fn assert_type_modulation_emits_midi_slot_update(previous_kind: &str) {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = previous_kind.into();
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "instruments.0.type".into(),
        label: Some("Type".into()),
        kind: "enum".into(),
        min: None,
        max: None,
        step: None,
        user_min: None,
        user_max: None,
        options: vec!["synth".into(), "sampler".into(), "midi".into()],
        invert: false,
    });

    runner.apply_runtime_modulation(
        &[platform_core::CellTriggerIntent {
            x: 7,
            y: 0,
            degree: 0,
            kind: platform_core::CellTriggerKind::Activate,
        }],
        0,
    );

    let commands = runner.outbox.drain_audio_commands();
    assert_eq!(runner.instruments[0].kind, "midi");
    assert!(commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot: 0,
            config
        } if config["type"] == "midi"
    )));
    assert!(!commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetSynthParam { .. } | RuntimeAudioCommand::SetSampleBankParam { .. }
    )));
}
