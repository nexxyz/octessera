use super::*;
use crate::native_runner::modulation_process::materialize_endpoint;
use crate::native_runner::modulation_target::Endpoint;
use std::collections::BTreeMap;

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
pub(crate) fn fx_storage_display_codec_materializes_bus_and_global_slots() {
    for (fx_type, param, storage, global_supported) in [
        ("eq", "midQ", 2.5, true),
        ("filter_lfo", "q", 6.0, false),
        ("duck", "threshold", 0.08, false),
        ("compressor", "ratio", 4.0, true),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let mut params = json!({ "mixPct": 100 });
        params[param] = json!(storage);
        runner.fx_buses[0].slot3_type = fx_type.into();
        runner.fx_buses[0].slot3_params = params.clone();
        if global_supported {
            runner.global_fx_slots[0] = fx_type.into();
            runner.global_fx_params[0] = params.clone();
        }
        runner.link_lfos[0].enabled = true;
        runner.link_lfos[0].target = Some(fx_binding(
            &format!("mixer.buses.0.slot3.params.{param}"),
            0.0,
            2000.0,
            1.0,
        ));
        if global_supported {
            runner.link_lfos[1].enabled = true;
            runner.link_lfos[1].target = Some(fx_binding(
                &format!("mixer.master.slots.0.params.{param}"),
                0.0,
                2000.0,
                1.0,
            ));
        }
        runner.transport.transport = RuntimeTransportState::Playing;
        runner
            .recompose_lfo_audio(false)
            .unwrap_or_else(|error| panic!("{fx_type} {param}: {error}"));
        let commands = runner.outbox.drain_audio_commands();
        assert!(
            commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot {
                    bus_index: 0,
                    slot_index: 2,
                    params,
                    ..
                } if params.get(param) == Some(&json!(storage))
            )),
            "{fx_type} {param}: {commands:?}"
        );
        if global_supported {
            assert!(
                commands.iter().any(|command| matches!(
                    command,
                    RuntimeAudioCommand::SetGlobalFxSlot {
                        slot_index: 0,
                        params,
                        ..
                    } if params.get(param) == Some(&json!(storage))
                )),
                "{fx_type} {param}: {commands:?}"
            );
        }
    }

    for (param, storage, display) in [
        ("midQ", 2.5, 250.0),
        ("q", 6.0, 600.0),
        ("threshold", 0.08, 8.0),
        ("ratio", 4.0, 8.0),
        ("rateHz", 4.05, 405.0),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.fx_buses[0].slot3_type = "tremolo".into();
        runner.global_fx_slots[0] = "compressor".into();
        let mut bus_params = json!({});
        bus_params[param] = json!(storage);
        runner.fx_buses[0].slot3_params = bus_params;
        let mut global_params = json!({});
        global_params[param] = json!(storage);
        runner.global_fx_params[0] = global_params;
        let mut effective = BTreeMap::new();
        effective.insert(format!("mixer.buses.0.slot3.params.{param}"), display);
        effective.insert(format!("mixer.master.slots.0.params.{param}"), display);
        let bus_command = materialize_endpoint(
            &runner,
            &Endpoint::FxBusSlot {
                bus_index: 0,
                slot: 2,
            },
            &effective,
        )
        .unwrap();
        let global_command =
            materialize_endpoint(&runner, &Endpoint::GlobalFxSlot { slot: 0 }, &effective).unwrap();
        assert!(matches!(
            bus_command,
            RuntimeAudioCommand::SetFxBusSlot { params, .. }
                if params.get(param) == Some(&json!(storage))
        ));
        assert!(matches!(
            global_command,
            RuntimeAudioCommand::SetGlobalFxSlot { params, .. }
                if params.get(param) == Some(&json!(storage))
        ));
    }
}

#[test]
pub(crate) fn config_commit_reconciles_old_and_candidate_endpoints_transactionally() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].volume = 50;
    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].phase_pulses = 24;
    runner.link_lfos[0].target = Some(volume_binding());
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.recompose_lfo_audio(false).unwrap();
    let _ = runner.outbox.drain_audio_commands();

    let mut changed = runner.config_payload();
    changed["runtimeConfig"]["instruments"][0]["mixer"]["volume"] = json!(30);
    runner.apply_config_payload(changed).unwrap();
    let committed = runner.outbox.drain_audio_commands();
    assert!(
        committed.iter().any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: 0,
                volume_pct: Some(80.0),
                ..
            }
        )),
        "{committed:?}"
    );

    let before_failure = runner.config_payload();
    let mut invalid = before_failure.clone();
    invalid["runtimeConfig"]["linkLfos"] = json!([]);
    assert!(runner.apply_config_payload(invalid).is_err());
    assert_eq!(runner.config_payload(), before_failure);
    assert!(runner.transient_lfo_overlay_for_key("instruments.0.mixer.volume"));

    let mut retargeted = before_failure.clone();
    retargeted["runtimeConfig"]["linkLfos"][0]["target"] = json!({
        "key": "instruments.1.mixer.volume",
        "label": "Volume",
        "kind": "number",
        "min": 0,
        "max": 100,
        "step": 1,
        "invert": false
    });
    runner.apply_config_payload(retargeted).unwrap();
    let retarget_commands = runner.outbox.drain_audio_commands();
    assert!(
        retarget_commands.iter().any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: 1,
                ..
            }
        )),
        "{retarget_commands:?}"
    );
    assert!(!runner.transient_lfo_overlay_for_key("instruments.0.mixer.volume"));
    assert!(runner.transient_lfo_overlay_for_key("instruments.1.mixer.volume"));

    let mut disabled = runner.config_payload();
    disabled["runtimeConfig"]["linkLfos"][0]["target"] = Value::Null;
    runner.apply_config_payload(disabled).unwrap();
    let _ = runner.outbox.drain_audio_commands();
    assert!(!runner.transient_lfo_overlay_for_key("instruments.1.mixer.volume"));
}

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

#[test]
pub(crate) fn resolver_reads_owner_state_without_behavior_serialization() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].target = Some(volume_binding());
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.behavior_state_serialization_calls.set(0);
    runner.recompose_lfo_audio(false).unwrap();
    runner.instruments[0].volume = 40;
    runner.recompose_lfo_audio(false).unwrap();
    assert_eq!(runner.behavior_state_serialization_calls.get(), 0);
}

#[test]
pub(crate) fn overlaid_endpoint_sibling_edit_rematerializes_complete_fx_slot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.fx_buses[0].slot3_type = "eq".into();
    runner.fx_buses[0].slot3_params = json!({ "midQ": 2.5, "q": 6.0, "mixPct": 100 });
    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].target = Some(fx_binding(
        "mixer.buses.0.slot3.params.midQ",
        25.0,
        2000.0,
        25.0,
    ));
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.recompose_lfo_audio(false).unwrap();
    let _ = runner.outbox.drain_audio_commands();

    runner.fx_buses[0].slot3_params["q"] = json!(7.0);
    runner.recompose_lfo_audio(false).unwrap();
    assert!(runner
        .outbox
        .drain_audio_commands()
        .iter()
        .any(|command| matches!(
            command,
            RuntimeAudioCommand::SetFxBusSlot { params, .. }
                if params.get("midQ") == Some(&json!(2.5))
                    && params.get("q") == Some(&json!(7.0))
        )));
}

fn setup_eq_lfo_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.fx_buses[0].slot3_type = "eq".into();
    runner.fx_buses[0].slot3_params = json!({ "midQ": 2.5, "lowGainDb": 0, "mixPct": 100 });
    runner.global_fx_slots[0] = "eq".into();
    runner.global_fx_params[0] = json!({ "midQ": 2.5, "lowGainDb": 0, "mixPct": 100 });
    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].target = Some(fx_binding(
        "mixer.buses.0.slot3.params.midQ",
        25.0,
        2000.0,
        25.0,
    ));
    runner.link_lfos[1].enabled = true;
    runner.link_lfos[1].target = Some(fx_binding(
        "mixer.master.slots.0.params.midQ",
        25.0,
        2000.0,
        25.0,
    ));
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.recompose_lfo_audio(false).unwrap();
    let _ = runner.outbox.drain_audio_commands();
    runner
}

#[test]
pub(crate) fn physical_fx_menu_sibling_edits_rematerialize_owned_endpoints() {
    let mut runner = setup_eq_lfo_runner();
    runner.menu.rebuild(runner.menu_config());
    assert!(runner
        .menu
        .set_number_value_for_key("mixer.buses.0.slot3.params.lowGainDb", 12));
    runner
        .apply_or_schedule_menu_key("mixer.buses.0.slot3.params.lowGainDb")
        .unwrap();
    assert!(runner
        .outbox
        .drain_audio_commands()
        .iter()
        .any(|command| matches!(
            command,
            RuntimeAudioCommand::SetFxBusSlot { params, .. }
                if params.get("midQ") == Some(&json!(2.5))
                    && params.get("lowGainDb") == Some(&json!(6.0))
        )));

    assert!(runner
        .menu
        .set_number_value_for_key("mixer.master.slots.0.params.lowGainDb", 12));
    runner
        .apply_or_schedule_menu_key("mixer.master.slots.0.params.lowGainDb")
        .unwrap();
    assert!(runner
        .outbox
        .drain_audio_commands()
        .iter()
        .any(|command| matches!(
            command,
            RuntimeAudioCommand::SetGlobalFxSlot { params, .. }
                if params.get("midQ") == Some(&json!(2.5))
                    && params.get("lowGainDb") == Some(&json!(6.0))
        )));
}

#[test]
pub(crate) fn binding_fx_sibling_edits_rematerialize_owned_endpoints() {
    let mut runner = setup_eq_lfo_runner();
    runner.active_sparks_mode = "xy".into();
    let binding = |key: &str| NativeParamBinding {
        key: key.into(),
        label: Some("Low Gain".into()),
        kind: "number".into(),
        min: Some(-12.0),
        max: Some(12.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    };
    runner.xy_x_binding = Some(binding("mixer.buses.0.slot3.params.lowGainDb"));
    runner.xy_y_binding = Some(binding("mixer.master.slots.0.params.lowGainDb"));
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 7 }),
            request_snapshot: None,
        })
        .unwrap();
    let commands = messages
        .into_iter()
        .find_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => Some(commands),
            _ => None,
        })
        .unwrap_or_default();
    assert!(
        commands.iter().any(|command| matches!(
            command,
            RuntimeAudioCommand::SetFxBusSlot { params, .. }
                if params.get("midQ") == Some(&json!(2.5))
                    && params.get("lowGainDb") == Some(&json!(6.0))
        )),
        "{commands:?}"
    );
    assert!(commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetGlobalFxSlot { params, .. }
            if params.get("midQ") == Some(&json!(2.5))
                && params.get("lowGainDb") == Some(&json!(6.0))
    )));
}
