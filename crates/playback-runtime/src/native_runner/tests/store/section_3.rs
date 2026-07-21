use super::*;

#[test]
pub(crate) fn patch_envelope_round_trips_and_preserves_local_aux_bindings() {
    let mut source = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    source
        .apply_config_payload(json!({
            "runtimeConfig": {
                "activeBehavior": "sequencer",
                "layers": [{ "worlds": { "behaviorId": "sequencer" } }]
            }
        }))
        .unwrap();
    let payload = source.patch_payload();

    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("displayBrightness".into()),
        press_action: Some(NativeMenuAction::PlatformEffect("midi.panic".into())),
    });
    loaded.aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some("displayBrightness".into()),
        press_action: Some(NativeMenuAction::PlatformEffect("midi.panic".into())),
    });
    loaded
        .apply_patch_payload_preserving_device(payload)
        .unwrap();

    assert_eq!(loaded.behavior.id(), "sequencer");
    let binding = loaded.aux_bindings[0].as_ref().expect("local aux binding");
    assert_eq!(binding.turn_key.as_deref(), Some("displayBrightness"));
    assert!(matches!(
        binding.press_action.as_ref(),
        Some(NativeMenuAction::PlatformEffect(action)) if action == "midi.panic"
    ));
}

#[test]
pub(crate) fn preset_load_preserves_local_midi_selection_without_select_effects() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.midi_enabled = true;
    runner.selected_midi_output_id = Some("local-out".into());
    runner.selected_midi_input_id = Some("local-in".into());

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
                        "layers": [{ "worlds": { "behaviorId": "sequencer" } }]
                    }
                })),
            },
        })
        .unwrap();

    assert!(runner.midi_enabled);
    assert_eq!(runner.selected_midi_output_id.as_deref(), Some("local-out"));
    assert_eq!(runner.selected_midi_input_id.as_deref(), Some("local-in"));
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
pub(crate) fn patch_and_device_apply_split_mixed_aux_binding_sides() {
    let mut source = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    source.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("sound.noteLengthMs".into()),
        press_action: Some(NativeMenuAction::BehaviorAction("clear".into())),
    });
    source.aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some("displayBrightness".into()),
        press_action: Some(NativeMenuAction::PlatformEffect("midi.panic".into())),
    });
    let patch = source.patch_payload();
    let device = source.device_config_payload();
    assert!(!patch["runtimeConfig"]["auxBindings"]["aux1"].is_null());
    assert!(patch["runtimeConfig"]["auxBindings"]["aux2"].is_null());
    assert!(device["runtimeConfig"]["auxBindings"]["aux1"].is_null());
    assert!(!device["runtimeConfig"]["auxBindings"]["aux2"].is_null());

    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("displayBrightness".into()),
        press_action: Some(NativeMenuAction::PlatformEffect("midi.panic".into())),
    });
    loaded.aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some("displayBrightness".into()),
        press_action: Some(NativeMenuAction::PlatformEffect("midi.panic".into())),
    });
    loaded.apply_patch_payload_preserving_device(patch).unwrap();
    let binding = loaded.aux_bindings[0].as_ref().unwrap();
    assert_eq!(binding.turn_key.as_deref(), Some("sound.noteLengthMs"));
    assert!(matches!(
        binding.press_action.as_ref(),
        Some(NativeMenuAction::BehaviorAction(action)) if action == "clear"
    ));
    let device_binding = loaded.aux_bindings[1].as_ref().unwrap();
    assert_eq!(
        device_binding.turn_key.as_deref(),
        Some("displayBrightness")
    );

    loaded
        .apply_device_config_payload_preserving_patch(device)
        .unwrap();
    let musical_binding = loaded.aux_bindings[0].as_ref().unwrap();
    assert_eq!(
        musical_binding.turn_key.as_deref(),
        Some("sound.noteLengthMs")
    );
    let device_binding = loaded.aux_bindings[1].as_ref().unwrap();
    assert_eq!(
        device_binding.turn_key.as_deref(),
        Some("displayBrightness")
    );
}

#[test]
pub(crate) fn aux_platform_effect_clicks_split_musical_from_device_actions() {
    let mut source = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    source.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: None,
        press_action: Some(NativeMenuAction::PlatformEffect("sample.assign:0:1".into())),
    });
    source.aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: None,
        press_action: Some(NativeMenuAction::PlatformEffect("sparks.fx.map".into())),
    });
    source.aux_bindings[2] = Some(NativeAuxBinding {
        turn_key: None,
        press_action: Some(NativeMenuAction::PlatformEffect("midi.panic".into())),
    });

    let patch = source.patch_payload();
    let device = source.device_config_payload();
    assert!(!patch["runtimeConfig"]["auxBindings"]["aux1"].is_null());
    assert!(!patch["runtimeConfig"]["auxBindings"]["aux2"].is_null());
    assert!(patch["runtimeConfig"]["auxBindings"]["aux3"].is_null());
    assert!(device["runtimeConfig"]["auxBindings"]["aux1"].is_null());
    assert!(device["runtimeConfig"]["auxBindings"]["aux2"].is_null());
    assert!(!device["runtimeConfig"]["auxBindings"]["aux3"].is_null());

    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.aux_bindings[2] = Some(NativeAuxBinding {
        turn_key: None,
        press_action: Some(NativeMenuAction::PlatformEffect("midi.panic".into())),
    });
    loaded.apply_patch_payload_preserving_device(patch).unwrap();

    assert!(matches!(
        loaded.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.press_action.as_ref()),
        Some(NativeMenuAction::PlatformEffect(action)) if action == "sample.assign:0:1"
    ));
    assert!(matches!(
        loaded.aux_bindings[1]
            .as_ref()
            .and_then(|binding| binding.press_action.as_ref()),
        Some(NativeMenuAction::PlatformEffect(action)) if action == "sparks.fx.map"
    ));
    assert!(matches!(
        loaded.aux_bindings[2]
            .as_ref()
            .and_then(|binding| binding.press_action.as_ref()),
        Some(NativeMenuAction::PlatformEffect(action)) if action == "midi.panic"
    ));
}

#[test]
pub(crate) fn patch_load_preserves_device_aux_turn_keys() {
    let source = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let patch = source.patch_payload();

    let mut loaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    loaded.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: None,
    });
    loaded.aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some("sound.audioOutputBufferFrames".into()),
        press_action: None,
    });

    loaded.apply_patch_payload_preserving_device(patch).unwrap();

    assert_eq!(
        loaded.aux_bindings[0]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("masterVolume")
    );
    assert_eq!(
        loaded.aux_bindings[1]
            .as_ref()
            .and_then(|binding| binding.turn_key.as_deref()),
        Some("sound.audioOutputBufferFrames")
    );
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
                        "layers": [{ "worlds": { "behaviorId": "keys" } }]
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
    runner.ui.display_brightness = 21;
    runner.usb_audio_out = "both".into();
    runner.audio_output_buffer_frames = 256;

    runner
        .apply_patch_payload_preserving_device(json!({
            "kind": "octessera.patch",
            "schemaVersion": 1,
            "runtimeConfig": {
                "activeBehavior": "sequencer",
                "layers": [{ "worlds": { "behaviorId": "sequencer" } }],
                "displayBrightness": 99,
                "usb": { "audioOut": "uac2" },
                "sound": { "audioOutputBufferFrames": 1024 }
            }
        }))
        .unwrap();

    assert_eq!(runner.behavior.id(), "sequencer");
    assert_eq!(runner.ui.display_brightness, 21);
    assert_eq!(runner.usb_audio_out, "both");
    assert_eq!(runner.audio_output_buffer_frames, 256);
}

#[test]
pub(crate) fn recovery_usb_reboot_and_backup_remain_full_payloads() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for action in ["system.reboot", "usb.applyReboot"] {
        let effect = runner.platform_effect_for_action(action).unwrap();
        let payload = match effect {
            RuntimePlatformEffect::StoreSaveRecovery { payload }
            | RuntimePlatformEffect::UsbApplyReboot { payload } => payload,
            _ => panic!("unexpected effect"),
        };
        assert!(payload["kind"].is_null());
        assert!(!payload["runtimeConfig"]["usb"].is_null());
        assert!(!payload["runtimeConfig"]["midi"].is_null());
    }

    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.config_dirty = true;
    let messages = runner.messages_with_snapshot().unwrap();
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::StoreSaveBackup { payload }
                    if payload["kind"].is_null()
                        && !payload["runtimeConfig"]["usb"].is_null()
                        && !payload["runtimeConfig"]["midi"].is_null()
            ))
    )));
}
