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
    runner.xy.smoothing_ms = 0;
    runner.instruments[0].volume = 50;
    runner.xy.x_binding = Some(volume_binding());
    runner.active_play_mode = ActivePlayMode::Page(PlayMode::Xy);
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
    runner.xy.smoothing_ms = 0;
    runner.instruments[0].volume = 50;
    runner.xy.touch = NativeXyTouch {
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
    assert!(runner.xy.touch.active);
}

#[test]
pub(crate) fn config_load_captures_changed_xy_owner_then_clear_restores_loaded_base() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy.smoothing_ms = 0;
    runner.instruments[1].volume = 20;
    runner.xy.touch = NativeXyTouch {
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
    runner.instruments[0].kind = InstrumentType::Synth;
    runner.layers[0].param_mods.x[0] = Some(NativeParamBinding {
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
        &[platform_core::TileTriggerIntent {
            x: 7,
            y: 0,
            degree: 0,
            kind: platform_core::TileTriggerKind::Activate,
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
    runner.instruments[0].kind = InstrumentType::Synth;
    runner.layers[0].param_mods.x[0] = Some(fx_binding(
        "instruments.0.synth.filter.cutoffHz",
        0.0,
        255.0,
        1.0,
    ));
    runner.apply_runtime_modulation(
        &[platform_core::TileTriggerIntent {
            x: 7,
            y: 0,
            degree: 0,
            kind: platform_core::TileTriggerKind::Activate,
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

    runner.instruments[0].kind = InstrumentType::Midi;
    runner.layers[0].param_mods.x[0] = Some(volume_binding());
    runner.apply_runtime_modulation(
        &[platform_core::TileTriggerIntent {
            x: 7,
            y: 0,
            degree: 0,
            kind: platform_core::TileTriggerKind::Activate,
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
    runner.instruments[0].kind = InstrumentType::from_wire_name(previous_kind).unwrap();
    runner.layers[0].param_mods.x[0] = Some(NativeParamBinding {
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
        &[platform_core::TileTriggerIntent {
            x: 7,
            y: 0,
            degree: 0,
            kind: platform_core::TileTriggerKind::Activate,
        }],
        0,
    );

    let commands = runner.outbox.drain_audio_commands();
    assert_eq!(runner.instruments[0].kind, InstrumentType::Midi);
    assert!(commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot: 0,
            config, ..
        } if config["type"] == "midi"
    )));
    assert!(!commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetSynthParam { .. } | RuntimeAudioCommand::SetSampleBankParam { .. }
    )));
}

#[test]
pub(crate) fn modulation_between_menu_steps_reaches_the_engine_unrounded_and_saves_whole_steps() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert_eq!(runner.instruments[0].kind, InstrumentType::Synth);
    let source = |lane| {
        crate::native_runner::modulation::source::ModulationSourceId::layer_axis(
            0,
            crate::native_runner::modulation::source::ModulationAxis::X,
            lane,
        )
        .unwrap()
    };
    let cutoff = fx_binding("instruments.0.synth.filter.cutoffHz", 0.0, 255.0, 1.0);
    let resonance = fx_binding("instruments.0.synth.filter.resonance", 0.0, 100.0, 1.0);
    runner.set_runtime_source_input(source(0), cutoff, 100.4 / 255.0);
    runner.set_runtime_source_input(source(1), resonance, 0.4025);
    runner.process_dirty_modulation_step(false).unwrap();

    let commands = runner.outbox.drain_audio_commands();
    let sent = |path: &str| {
        commands.iter().find_map(|command| match command {
            RuntimeAudioCommand::SetSynthParam {
                instrument_slot: 0,
                path: sent,
                value,
                ..
            } if sent == path => Some(f64::from(*value)),
            _ => None,
        })
    };
    let cutoff_hz = sent("synth.filter.cutoffHz").expect("cutoff command");
    assert!((cutoff_hz - 20.0 * 1000.0_f64.powf(100.4 / 255.0)).abs() < 0.01);
    let resonance = sent("synth.filter.resonance").expect("resonance command");
    assert!((resonance - 40.25).abs() < 1.0e-4);
    let saved = &runner.instruments[0].synth_config["filter"];
    let saved_cutoff = (20.0 * 1000.0_f64.powf(100.0 / 255.0)).round();
    assert_eq!(saved["cutoffHz"].as_f64(), Some(saved_cutoff));
    assert_eq!(saved["resonance"].as_f64(), Some(40.0));
}

#[test]
pub(crate) fn fx_modulation_between_menu_steps_reaches_the_engine_unrounded() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.fx_buses[0].slots[0].fx_type = "delay".into();
    runner.fx_buses[0].slots[0].params = json!({ "timeMs": 250, "feedback": 0.35, "mixPct": 35 });
    let source = crate::native_runner::modulation::source::ModulationSourceId::layer_axis(
        0,
        crate::native_runner::modulation::source::ModulationAxis::X,
        0,
    )
    .unwrap();
    let mix = fx_binding("mixer.buses.0.slot1.params.mixPct", 0.0, 100.0, 1.0);
    runner.set_runtime_source_input(source, mix, 0.333);
    runner.process_dirty_modulation_step(false).unwrap();

    let commands = runner.outbox.drain_audio_commands();
    let sent = commands.iter().find_map(|command| match command {
        RuntimeAudioCommand::SetFxBusParam {
            bus_index: 0,
            slot_index: 0,
            param: realtime_engine::synth::FxParamId::MixPct,
            value,
            ..
        } => Some(*value),
        _ => None,
    });
    assert!((sent.expect("mix command") - 33.3).abs() < 1.0e-4);
    assert_eq!(runner.fx_buses[0].slots[0].params["mixPct"], json!(33));
}

#[test]
pub(crate) fn a_binding_left_behind_by_an_fx_type_change_keeps_the_patch_saveable() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.fx_buses[0].slots[0].fx_type = "delay".into();
    runner.fx_buses[0].slots[0].params = json!({ "timeMs": 250, "feedback": 0.35, "mixPct": 35 });
    let source = crate::native_runner::modulation::source::ModulationSourceId::layer_axis(
        0,
        crate::native_runner::modulation::source::ModulationAxis::X,
        0,
    )
    .unwrap();
    let feedback = fx_binding("mixer.buses.0.slot1.params.feedback", 0.0, 95.0, 1.0);
    runner.set_runtime_source_input(source, feedback.clone(), 0.2);
    runner.process_dirty_modulation_step(false).unwrap();
    runner.fx_buses[0].slots[0].fx_type = "tremolo".into();
    runner.fx_buses[0].slots[0].params = crate::native_runner::fx_default_params("tremolo");

    runner.set_runtime_source_input(source, feedback, 0.7);
    runner.process_dirty_modulation_step(false).unwrap();

    assert_eq!(
        runner.fx_buses[0].slots[0].params,
        crate::native_runner::fx_default_params("tremolo")
    );
    crate::native_runner::split_local_system_patch_documents(&runner.config_payload()).unwrap();
}

#[test]
pub(crate) fn releasing_a_hold_after_an_fx_type_change_keeps_the_new_effects_value() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.fx_buses[0].slots[0].fx_type = "delay".into();
    runner.fx_buses[0].slots[0].params = json!({ "timeMs": 250, "feedback": 0.35, "mixPct": 35 });
    let source = crate::native_runner::modulation::source::ModulationSourceId::layer_axis(
        0,
        crate::native_runner::modulation::source::ModulationAxis::X,
        0,
    )
    .unwrap();
    let mix = fx_binding("mixer.buses.0.slot1.params.mixPct", 0.0, 100.0, 1.0);
    runner.set_runtime_source_input(source, mix, 0.8);
    runner.process_dirty_modulation_step(false).unwrap();

    let type_key = "mixer.buses.0.slot1.type";
    assert!(runner.menu.set_enum_value_for_key(type_key, "reverb"));
    runner.apply_or_schedule_menu_key(type_key).unwrap();
    assert_eq!(runner.fx_buses[0].slots[0].params["mixPct"], json!(30));

    runner.clear_runtime_source_input(source);
    runner.process_dirty_modulation_step(false).unwrap();
    assert_eq!(runner.fx_buses[0].slots[0].params["mixPct"], json!(30));
}
