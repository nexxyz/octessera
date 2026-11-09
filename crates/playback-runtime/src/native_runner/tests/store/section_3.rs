use super::*;

#[test]
pub(crate) fn patch_load_brings_its_own_aux_layout_including_system_targets() {
    let mut source = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    source.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: Some(NativeMenuAction::PlatformEffect("midi.panic".into())),
    });
    source.aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some("sound.noteLengthMs".into()),
        press_action: Some(NativeMenuAction::PlatformEffect("play.fx.map".into())),
    });
    source.shift_aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("displayBrightness".into()),
        press_action: None,
    });
    let patch = source.patch_payload().unwrap();
    assert_eq!(
        patch["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"],
        "masterVolume"
    );
    assert!(source.device_config_payload().unwrap()["runtimeConfig"]
        .get("auxBindings")
        .is_none());

    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.aux_bindings[2] = Some(NativeAuxBinding {
        turn_key: Some("gridBrightness".into()),
        press_action: None,
    });
    loaded.shift_aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: None,
    });
    loaded.apply_patch_payload_preserving_device(patch).unwrap();

    assert_eq!(loaded.aux_bindings, source.aux_bindings);
    assert_eq!(loaded.shift_aux_bindings, source.shift_aux_bindings);
}

#[test]
pub(crate) fn device_config_apply_keeps_the_patch_aux_layout() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let device = runner.device_config_payload().unwrap();
    runner.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: None,
    });
    runner.shift_aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some("transport.bpm".into()),
        press_action: None,
    });
    let bindings = (
        runner.aux_bindings.clone(),
        runner.shift_aux_bindings.clone(),
    );

    runner
        .apply_device_config_payload_preserving_patch(device)
        .unwrap();

    assert_eq!(
        (
            runner.aux_bindings.clone(),
            runner.shift_aux_bindings.clone()
        ),
        bindings
    );
}

#[test]
pub(crate) fn preset_load_preserves_local_midi_selection_without_select_effects() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.midi.enabled = true;
    runner.midi.output_id = Some("local-out".into());
    runner.midi.input_id = Some("local-in".into());

    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: "Patch".into(),
                payload: Some(json!({
                    "runtimeConfig": {
                        "midi": {
                            "enabled": false,
                            "outId": "preset-out",
                            "inId": "preset-in"
                        },
                        "layers": [{ "build": { "behaviorId": "sequencer" } }]
                    }
                })),
            },
        })
        .unwrap();

    assert!(runner.midi.enabled);
    assert_eq!(runner.midi.output_id.as_deref(), Some("local-out"));
    assert_eq!(runner.midi.input_id.as_deref(), Some("local-in"));
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::MidiSelectOutput { .. }
                    | RuntimePlatformEffect::MidiSelectInput { .. }
            ))
    )));
}

#[test]
pub(crate) fn patch_load_swaps_active_engine_state_to_loaded_behavior() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: "Keys".into(),
                payload: Some(json!({
                    "runtimeConfig": {
                        "activeLayerIndex": 0,
                        "layers": [{ "build": { "behaviorId": "keys" } }]
                    }
                })),
            },
        })
        .unwrap();

    let snapshot = runner.snapshot().unwrap();
    assert_eq!(snapshot["activeBehavior"], "keys");
    assert_eq!(snapshot["gridInteraction"], "momentary");
}

#[test]
pub(crate) fn patch_envelope_device_fields_do_not_override_local_device_config() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.display.ui.display_brightness = 2;
    runner.audio_io.audio_outputs = AudioOutputSet::from_flags(true, true, false).unwrap();

    runner
        .apply_patch_payload_preserving_device(json!({
            "kind": "octessera.patch",
            "schemaVersion": 2,
            "runtimeConfig": {
                "activeBehavior": "sequencer",
                "layers": [{ "build": { "behaviorId": "sequencer" } }],
                "displayBrightness": 15,
                "usb": { "midiOutEnabled": true }
            }
        }))
        .unwrap();

    assert_eq!(runner.active_behavior().id(), "sequencer");
    assert_eq!(runner.display.ui.display_brightness, 2);
    assert_eq!(
        runner.audio_io.audio_outputs,
        AudioOutputSet::from_flags(true, true, false).unwrap()
    );
}

#[test]
pub(crate) fn recovery_and_backup_payloads_are_patch_only() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let effect = runner.platform_effect_for_action("system.reboot").unwrap();
    let payload = match effect {
        Some(RuntimePlatformEffect::StoreSaveRecovery { payload }) => payload,
        _ => panic!("unexpected effect"),
    };
    assert_eq!(payload["kind"], "octessera.patch");
    assert_eq!(payload["schemaVersion"], 2);
    assert!(payload["runtimeConfig"].get("usb").is_none());
    assert!(payload["runtimeConfig"].get("midi").is_none());

    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.config_dirty = true;
    let messages = runner.messages_with_snapshot().unwrap();
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveBackup { payload }
                    if payload["kind"] == "octessera.patch"
                        && payload["schemaVersion"] == 2
                        && payload["runtimeConfig"].get("usb").is_none()
                        && payload["runtimeConfig"].get("midi").is_none()
            ))
    )));
}
