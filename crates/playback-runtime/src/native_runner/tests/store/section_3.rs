use super::*;
use crate::native_menu::PlatformAction;

#[test]
pub(crate) fn patch_load_brings_its_own_aux_layout_including_system_targets() {
    let mut source = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    source.aux_bindings[0] = Some(NativeAuxBinding {
        turn_key: Some("masterVolume".into()),
        press_action: Some(NativeMenuAction::PlatformEffect(PlatformAction::MidiPanic)),
    });
    source.aux_bindings[1] = Some(NativeAuxBinding {
        turn_key: Some("sound.noteLengthMs".into()),
        press_action: Some(NativeMenuAction::PlatformEffect(PlatformAction::PlayFxMap)),
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
                payload: Some(patch_document(json!({
                    "runtimeConfig": {
                        "midi": {
                            "enabled": false,
                            "outId": "preset-out",
                            "inId": "preset-in"
                        },
                        "layers": [{ "build": { "behaviorId": "sequencer" } }]
                    }
                }))),
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
                payload: Some(patch_document(json!({
                    "runtimeConfig": {
                        "activeLayerIndex": 0,
                        "layers": [{ "build": { "behaviorId": "keys" } }]
                    }
                }))),
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
        .apply_patch_payload_preserving_device(patch_document(json!({
            "kind": "octessera.patch",
            "schemaVersion": 2,
            "runtimeConfig": {
                "activeBehavior": "sequencer",
                "layers": [{ "build": { "behaviorId": "sequencer" } }],
                "displayBrightness": 15,
                "usb": { "midiOutEnabled": true }
            }
        })))
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
    let effect = runner.recovery_save_effect().unwrap();
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

#[test]
pub(crate) fn preset_name_edit_counts_multi_byte_characters_as_one_slot() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.patch_store.preset_draft_name = "Café".into();
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("system.draftName"));
    let edited_row = |runner: &NativeRunner| {
        let snapshot = runner.menu.snapshot();
        let row = snapshot.rows[snapshot.selected_row.unwrap()]
            .clone()
            .unwrap();
        (row.value, row.text_cursor)
    };
    let turn = json!({ "type": "encoder_turn", "id": "main", "delta": 1 });
    let delete = [
        json!({ "type": "button_shift", "pressed": true }),
        json!({ "type": "button_a", "pressed": true }),
        json!({ "type": "button_a", "pressed": false }),
        json!({ "type": "button_shift", "pressed": false }),
    ];

    device_input(
        &mut runner,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert_eq!(edited_row(&runner), ("Café".into(), Some(5)));

    device_input(&mut runner, turn.clone());
    assert_eq!(edited_row(&runner), ("CaféA".into(), Some(5)));

    for input in delete.clone() {
        device_input(&mut runner, input);
    }
    assert_eq!(edited_row(&runner), ("CafA".into(), Some(3)));

    device_input(&mut runner, turn);
    assert_eq!(edited_row(&runner), ("CafB".into(), Some(3)));

    for _ in 0..3 {
        for input in delete.clone() {
            device_input(&mut runner, input);
        }
    }
    assert_eq!(edited_row(&runner), ("B".into(), Some(0)));
}

#[test]
fn preset_save_mode_keeps_its_overwrite_wire_string() {
    for wire in [
        json!({"type": "store_save_preset", "name": "A", "payload": {}, "mode": "overwrite"}),
        json!({"type": "store_save_preset", "name": "A", "payload": {}, "mode": null}),
    ] {
        let effect: RuntimePlatformEffect = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(&effect).unwrap(), wire);
    }
    let unset: RuntimePlatformEffect =
        serde_json::from_value(json!({"type": "store_save_preset", "name": "A", "payload": {}}))
            .unwrap();
    assert!(matches!(
        unset,
        RuntimePlatformEffect::StoreSavePreset { mode: None, .. }
    ));
    assert!(serde_json::from_value::<RuntimePlatformEffect>(
        json!({"type": "store_save_preset", "name": "A", "payload": {}, "mode": "append"})
    )
    .is_err());
}

#[test]
fn default_save_mode_keeps_its_deferred_wire_string() {
    for wire in [
        json!({"type": "store_save_default", "payload": {}, "mode": "deferred"}),
        json!({"type": "store_save_default", "payload": {}, "mode": null}),
    ] {
        let effect: RuntimePlatformEffect = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(&effect).unwrap(), wire);
    }
    let unset: RuntimePlatformEffect =
        serde_json::from_value(json!({"type": "store_save_default", "payload": {}})).unwrap();
    assert!(matches!(
        unset,
        RuntimePlatformEffect::StoreSaveDefault { mode: None, .. }
    ));
    assert!(serde_json::from_value::<RuntimePlatformEffect>(
        json!({"type": "store_save_default", "payload": {}, "mode": "later"})
    )
    .is_err());
}

#[test]
pub(crate) fn rename_onto_another_preset_asks_to_overwrite_it_and_saves_with_overwrite() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.patch_store.preset_names = vec!["Alpha".into(), "Beta".into()];
    runner.menu.rebuild(runner.menu_config());
    assert!(runner.menu.focus_item_key("preset.renamePick.Alpha"));
    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    let confirm_rename = |runner: &mut NativeRunner, draft: &str| {
        runner.patch_store.preset_draft_name = draft.into();
        runner.menu.rebuild(runner.menu_config());
        assert!(runner.menu.focus_item_key("preset.rename.apply"));
        let opened = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
                request_snapshot: None,
            })
            .unwrap();
        let display = snapshot_from(&opened)["display"].clone();
        assert_eq!(display["title"], "Confirm Rename");
        let effects = confirm_current_dialog(runner)
            .into_iter()
            .find_map(|message| match message {
                RunnerMessage::PlatformEffects { effects } => Some(effects),
                _ => None,
            })
            .expect("rename save effect");
        runner.patch_store.preset_rename_source = Some("Alpha".into());
        (display["lines"].clone(), effects)
    };

    let (lines, effects) = confirm_rename(&mut runner, " Beta ");
    assert_eq!(
        lines.as_array().unwrap()[..2],
        [json!("Overwrite preset Beta"), json!("with Alpha?")]
    );
    assert!(matches!(
        effects.as_slice(),
        [RuntimePlatformEffect::StoreSavePreset { name, mode, .. }]
            if name == "Beta" && *mode == Some(crate::PresetSaveMode::Overwrite)
    ));

    let (lines, effects) = confirm_rename(&mut runner, "Gamma");
    assert_eq!(lines.as_array().unwrap()[0], json!("Rename Alpha to"));
    assert!(matches!(
        effects.as_slice(),
        [RuntimePlatformEffect::StoreSavePreset { name, mode: None, .. }] if name == "Gamma"
    ));
}
