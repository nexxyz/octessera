use super::*;
use crate::native_runner::modulation_audio::instrument_modulation_audio_command;
use crate::native_runner::modulation_target::{classify_key, TargetMode, TargetValueKind};

fn numeric_binding(key: &str, min: f64, max: f64) -> NativeParamBinding {
    NativeParamBinding {
        key: key.into(),
        label: Some(key.into()),
        kind: "number".into(),
        min: Some(min),
        max: Some(max),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    }
}

#[test]
fn instrument_scalar_target_matrix_excludes_fm_enums_and_structural_sample_controls() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "fm".into();
    runner.menu.rebuild(runner.menu_config());
    let tone = runner.resolve_aux_auto_map("", Some("instruments.0.fm.index"), None);
    assert_eq!(
        tone.iter()
            .map(|slot| slot.as_ref().unwrap().turn.as_ref().unwrap().key.as_str())
            .collect::<Vec<_>>(),
        [
            "instruments.0.fm.index",
            "instruments.0.fm.filter.cutoffHz",
            "instruments.0.fm.filter.resonance",
            "instruments.0.fm.amp.gainPct"
        ]
    );
    let index_env =
        runner.resolve_aux_auto_map("", Some("instruments.0.fm.indexEnv.attackMs"), None);
    assert_eq!(
        index_env[0].as_ref().unwrap().turn.as_ref().unwrap().key,
        "instruments.0.fm.indexEnv.attackMs"
    );
    for field in [
        "fm.index",
        "fm.amp.gainPct",
        "fm.filter.cutoffHz",
        "fm.filter.resonance",
    ] {
        let key = format!("instruments.0.{field}");
        assert!(supported_param_binding_key(&key));
        assert_eq!(classify_key(&key).unwrap().0, TargetValueKind::Numeric);
        assert_eq!(classify_key(&key).unwrap().1, TargetMode::Numeric);
        assert!(crate::native_runner::is_live_link_lfo_target_for_picker(
            &key
        ));
    }
    for field in ["fm.ratio", "fm.filter.type"] {
        let key = format!("instruments.0.{field}");
        assert!(!supported_param_binding_key(&key));
        assert!(classify_key(&key).is_none());
    }
    for field in [
        "sample.tuneSemis",
        "sample.amp.gainPct",
        "sample.amp.velocitySensitivityPct",
    ] {
        let key = format!("instruments.0.{field}");
        assert!(supported_param_binding_key(&key));
        assert!(!crate::native_runner::is_live_link_lfo_target_for_picker(
            &key
        ));
        assert!(matches!(
            instrument_modulation_audio_command(0, field, &json!(75)),
            Some(RuntimeAudioCommand::SetSampleBankParam { .. })
        ));
    }
    for field in [
        "synth.osc1.levelPct",
        "synth.osc1.detuneCents",
        "synth.osc1.pulseWidthPct",
        "synth.osc2.levelPct",
        "synth.osc2.detuneCents",
        "synth.osc2.pulseWidthPct",
        "synth.amp.gainPct",
    ] {
        assert!(matches!(
            instrument_modulation_audio_command(0, field, &json!(50)),
            Some(RuntimeAudioCommand::SetSynthParam { .. })
        ));
    }
}

#[test]
fn fm_live_link_lfo_uses_scalar_endpoint_and_release_restores_base() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "fm".into();
    runner.link_lfos[0].enabled = true;
    runner.link_lfos[0].target = Some(numeric_binding("instruments.0.fm.index", 0.0, 100.0));
    runner.link_lfos[0].phase_pulses = 3;
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.recompose_lfo_audio(false).unwrap();
    let commands = runner.outbox.drain_audio_commands();
    assert!(commands.iter().all(|command| matches!(command, RuntimeAudioCommand::SetFmParam { instrument_slot: 0, path, .. } if path == "fm.index")), "{commands:?}");
    assert_eq!(commands.len(), 1);
    runner.link_lfos[0].enabled = false;
    runner.recompose_lfo_audio(false).unwrap();
    assert!(
        matches!(&runner.outbox.drain_audio_commands()[..], [RuntimeAudioCommand::SetFmParam { instrument_slot: 0, path, value: 50.0, .. }] if path == "fm.index")
    );
}

#[test]
fn runner_only_sample_modulation_does_not_replace_audio_slot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    for (field, min, max) in [
        ("sample.baseVelocity", 1.0, 127.0),
        ("sample.velocityLevels.high", 1.0, 127.0),
        ("sample.velocityLevels.medium", 1.0, 127.0),
        ("sample.velocityLevels.low", 1.0, 127.0),
    ] {
        runner.param_mods[0].x[0] =
            Some(numeric_binding(&format!("instruments.0.{field}"), min, max));
        runner.apply_runtime_modulation(
            &[platform_core::CellTriggerIntent {
                x: 7,
                y: 0,
                degree: 0,
                kind: platform_core::CellTriggerKind::Activate,
            }],
            0,
        );
        assert!(
            !runner
                .outbox
                .drain_audio_commands()
                .iter()
                .any(|command| matches!(
                    command,
                    RuntimeAudioCommand::SetInstrumentSlot { .. }
                        | RuntimeAudioCommand::SetAudioConfig { .. }
                )),
            "{field}"
        );
    }
    let mut slot_binding = numeric_binding("instruments.0.sample.selectedSlot", 1.0, 8.0);
    slot_binding.kind = "enum".into();
    slot_binding.options = (1..=8).map(|index| index.to_string()).collect();
    runner.param_mods[0].x[0] = Some(slot_binding);
    runner.apply_runtime_modulation(
        &[platform_core::CellTriggerIntent {
            x: 7,
            y: 0,
            degree: 0,
            kind: platform_core::CellTriggerKind::Activate,
        }],
        0,
    );
    assert_eq!(runner.instruments[0].selected_sample_slot, 7);
    assert!(!runner
        .outbox
        .drain_audio_commands()
        .iter()
        .any(|command| matches!(
            command,
            RuntimeAudioCommand::SetInstrumentSlot { .. }
                | RuntimeAudioCommand::SetAudioConfig { .. }
        )));
}

#[test]
fn inert_sample_bindings_reject_new_actions_but_legacy_config_still_loads() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let inert = [
        "sample.ampEnv.attackMs",
        "sample.ampEnv.decayMs",
        "sample.ampEnv.sustainPct",
        "sample.ampEnv.releaseMs",
        "sample.filterEnv.attackMs",
        "sample.filterEnv.decayMs",
        "sample.filterEnv.sustainPct",
        "sample.filterEnv.releaseMs",
        "sample.filter.type",
        "sample.filter.envAmountPct",
        "sample.filter.keyTrackingPct",
    ];
    for field in inert {
        let key = format!("instruments.0.{field}");
        let binding = crate::native_menu::NativeParamBindingSpec {
            key: key.clone(),
            label: Some(field.into()),
            kind: if field.ends_with(".type") {
                "enum"
            } else {
                "number"
            }
            .into(),
            min: Some(0),
            max: Some(100),
            step: Some(1),
            user_min: None,
            user_max: None,
            options: if field.ends_with(".type") {
                vec!["lowpass".into(), "highpass".into()]
            } else {
                vec![]
            },
            invert: false,
        };
        for target in [
            "aux:0:turn",
            "shiftAux:0:turn",
            "xy:x",
            "param:0:x:0",
            "linkLfos.0.target",
        ] {
            runner
                .execute_menu_action(NativeMenuAction::SetParamBinding {
                    target: target.into(),
                    binding: binding.clone(),
                })
                .unwrap();
            assert!(runner.aux_bindings[0].is_none(), "{target} {field}");
            assert!(runner.shift_aux_bindings[0].is_none(), "{target} {field}");
            assert!(runner.xy_x_binding.is_none(), "{target} {field}");
            assert!(runner.param_mods[0].x[0].is_none(), "{target} {field}");
            assert!(runner.link_lfos[0].target.is_none(), "{target} {field}");
        }
    }

    let legacy_key = "instruments.0.sample.ampEnv.attackMs";
    let mut legacy = runner.config_payload();
    legacy["runtimeConfig"]["instruments"][0]["type"] = json!("sampler");
    legacy["runtimeConfig"]["auxBindings"]["aux1"] =
        json!({ "turnKey": legacy_key, "pressAction": null });
    legacy["runtimeConfig"]["xy"]["x"] = json!({
        "key": legacy_key, "kind": "number", "min": 0, "max": 5000, "step": 5, "invert": false
    });
    runner.apply_config_payload(legacy).unwrap();
    assert_eq!(
        runner.aux_bindings[0].as_ref().unwrap().turn_key.as_deref(),
        Some(legacy_key)
    );
    assert_eq!(runner.xy_x_binding.as_ref().unwrap().key, legacy_key);
    runner.active_play_mode = "xy".into();
    runner.xy_smoothing_ms = 0;
    let _ = runner.messages_with_snapshot().unwrap();
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "grid_press", "x": 7, "y": 0 }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(!messages.iter().any(|message| matches!(message,
        RunnerMessage::AudioCommands { commands } if commands.iter().any(|command| matches!(command,
            RuntimeAudioCommand::SetInstrumentSlot { .. } | RuntimeAudioCommand::SetAudioConfig { .. }
        ))
    )));

    runner.menu.rebuild(runner.menu_config());
    assert!(runner
        .menu
        .focus_item_key("instruments.0.sample.ampEnv.attackMs"));
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "button_fn", "pressed": true }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "aux2" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(runner.aux_bindings[1].is_none());
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("Mapping rejected: unsupported target")
    );
}
