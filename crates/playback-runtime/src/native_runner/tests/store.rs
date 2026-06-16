use super::*;

#[test]
fn system_menu_save_default_emits_native_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 0, 1];
    runner.menu.state.cursor = 0;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(
        snapshot_from(&opened)["display"]["title"],
        "Confirm Default"
    );
    let messages = confirm_current_dialog(&mut runner);

    let payload = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => {
                effects.iter().find_map(|effect| match effect {
                    RuntimePlatformEffect::StoreSaveDefault { payload, .. } => Some(payload),
                    _ => None,
                })
            }
            _ => None,
        })
        .expect("save default payload");
    assert_eq!(payload["activeBehavior"], "life");
    assert!(
        payload["runtimeConfig"]["instruments"]
            .as_array()
            .unwrap()
            .len()
            >= 8
    );
    assert_ne!(payload, &Value::Null);
}

#[test]
fn load_default_result_applies_native_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let payload = json!({
        "activeBehavior": "sequencer",
        "runtimeConfig": {
            "activePartIndex": 1,
            "parts": [
                { "l1": { "behaviorId": "life" }, "name": "life" },
                { "l1": { "behaviorId": "sequencer" }, "name": "sequencer" }
            ],
            "instruments": [
                { "type": "sampler", "name": "sampler", "noteBehavior": "hold", "autoName": true, "mixer": { "volume": 70, "panPos": 10 } }
            ],
            "masterVolume": 88,
            "displayBrightness": 66,
            "buttonBrightness": 55,
            "danceMode": "pan",
            "midi": { "enabled": true, "syncMode": "external" }
        },
        "mappingConfig": platform_core::default_mapping_config()
    });

    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(payload),
            },
        })
        .unwrap();

    assert_eq!(runner.active_part_index, 1);
    assert_eq!(runner.behavior.id(), "sequencer");
    assert_eq!(runner.instruments[0].kind, "sampler");
    assert_eq!(runner.instruments[0].note_behavior, "hold");
    assert_eq!(runner.note_behaviors[0], NoteBehavior::Hold);
    assert_eq!(runner.ui.master_volume, 88);
    assert_eq!(runner.sync_source, SyncSource::External);
    assert_eq!(snapshot_from(&messages)["activeBehavior"], "sequencer");
}

#[test]
fn midi_store_results_update_native_snapshot_state() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiListOutputsResult {
                outputs: vec![MidiPort {
                    id: "out1".into(),
                    name: "Output 1".into(),
                }],
            },
        })
        .unwrap();
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiListInputsResult {
                inputs: vec![MidiPort {
                    id: "in1".into(),
                    name: "Input 1".into(),
                }],
            },
        })
        .unwrap();
    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiStatus {
                ok: false,
                message: Some("No device".into()),
                selected_out_id: Some("out1".into()),
                selected_in_id: Some("in1".into()),
            },
        })
        .unwrap();
    let midi = &snapshot_from(&messages)["settings"]["midi"];

    assert_eq!(midi["outputs"][0]["id"], "out1");
    assert_eq!(midi["inputs"][0]["id"], "in1");
    assert_eq!(midi["outId"], "out1");
    assert_eq!(midi["inId"], "in1");
    assert_eq!(midi["status"], "No device");
}

#[test]
fn midi_output_menu_selects_dynamic_port() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::MidiListOutputsResult {
                outputs: vec![MidiPort {
                    id: "out1".into(),
                    name: "Output 1".into(),
                }],
            },
        })
        .unwrap();
    runner.menu.state.stack = vec![5, 2, 2];
    runner.menu.state.cursor = 1;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::MidiSelectOutput { id: Some("out1".into()) }]
    )));
}

#[test]
fn entering_midi_port_groups_requests_port_lists() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 2];
    runner.menu.state.cursor = 2;

    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::MidiListOutputsRequest]
    )));

    runner.menu.state.stack = vec![5, 2];
    runner.menu.state.cursor = 3;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::MidiListInputsRequest]
    )));
}

#[test]
fn preset_load_menu_selects_dynamic_preset() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::ListPresetsResult {
                names: vec!["Alpha".into()],
            },
        })
        .unwrap();
    runner.menu.state.stack = vec![5, 0, 0, 2];
    runner.menu.state.cursor = 1;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Load");
    let messages = confirm_current_dialog(&mut runner);

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::StoreLoadPreset { name: "Alpha".into() }]
    )));
}

#[test]
fn save_current_uses_loaded_preset_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: "Alpha".into(),
                payload: Some(json!({ "runtimeConfig": { "activeBehavior": "sequencer" } })),
            },
        })
        .unwrap();
    runner.menu.state.stack = vec![5, 0, 0];
    runner.menu.state.cursor = 1;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Save");
    let messages = confirm_current_dialog(&mut runner);

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSavePreset { name, mode, .. }]
                    if name == "Alpha" && mode.as_deref() == Some("overwrite")
            )
    )));
}

#[test]
fn native_store_and_action_toasts_cover_common_confirmation_results() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner.menu.state.stack = vec![5, 0, 0];
    runner.menu.state.cursor = 1;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert!(!messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::PlatformEffects { .. })));
    assert_eq!(runner.toast.as_ref().unwrap().message, "No preset loaded");

    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: "Alpha".into(),
                payload: Some(json!({ "runtimeConfig": { "activeBehavior": "sequencer" } })),
            },
        })
        .unwrap();
    assert_eq!(runner.toast.as_ref().unwrap().message, "Loaded Alpha");

    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SavePresetResult {
                name: "Alpha".into(),
                outcome: "overwritten".into(),
            },
        })
        .unwrap();
    assert_eq!(runner.toast.as_ref().unwrap().message, "Saved Alpha");

    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::DeletePresetResult {
                name: "Alpha".into(),
                ok: true,
            },
        })
        .unwrap();
    assert_eq!(runner.toast.as_ref().unwrap().message, "Deleted Alpha");
}

#[test]
fn midi_panic_and_synth_preset_actions_show_toasts() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let effect = runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect("midi.panic".into()))
        .unwrap();
    assert_eq!(effect, Some(RuntimePlatformEffect::MidiPanic));
    assert_eq!(runner.toast.as_ref().unwrap().message, "MIDI panic sent");

    runner.load_synth_preset(0, "lead");
    assert_eq!(runner.toast.as_ref().unwrap().message, "Loaded synth lead");
}

#[test]
fn preset_save_as_uses_text_draft_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.preset_draft_name = "Jam A".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![5, 0, 0, 0];
    runner.menu.state.cursor = 1;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Save");
    let messages = confirm_current_dialog(&mut runner);

    assert_eq!(runner.preset_draft_name, "Jam A");
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSavePreset { name, mode, .. }]
                    if name == "Jam A" && mode.is_none()
            )
    )));
}

#[test]
fn fresh_save_as_preset_name_uses_timestamp_format() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    assert!(is_timestamp_preset_name(&runner.preset_draft_name));
    assert!(is_timestamp_preset_name(&clean_preset_name("   ")));
}

#[test]
fn preset_save_as_uses_fresh_timestamp_draft_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let draft_name = runner.preset_draft_name.clone();
    assert!(is_timestamp_preset_name(&draft_name));
    runner.menu.state.stack = vec![5, 0, 0, 0];
    runner.menu.state.cursor = 1;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Save");
    let messages = confirm_current_dialog(&mut runner);

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSavePreset { name, mode, .. }]
                    if name == &draft_name && mode.is_none()
            )
    )));
}

fn is_timestamp_preset_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 17
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7 | 10) || byte.is_ascii_digit())
}

#[test]
fn preset_rename_pick_sets_new_name_and_apply_saves() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.preset_names = vec!["Alpha".into()];
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![5, 0, 0, 3];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();

    assert_eq!(runner.preset_rename_source.as_deref(), Some("Alpha"));
    assert_eq!(runner.preset_draft_name, "Alpha");
    runner.preset_draft_name = "Alpha A".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![5, 0, 0, 3];
    runner.menu.state.cursor = 2;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Rename");
    let messages = confirm_current_dialog(&mut runner);

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if matches!(
                effects.as_slice(),
                [RuntimePlatformEffect::StoreSavePreset { name, .. }]
                    if name == "Alpha A"
            )
    )));

    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SavePresetResult {
                name: "Alpha A".into(),
                outcome: "created".into(),
            },
        })
        .unwrap();

    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects == &vec![RuntimePlatformEffect::StoreDeletePreset { name: "Alpha".into() }]
    )));
}
