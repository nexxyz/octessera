use super::*;

#[test]
pub(crate) fn system_menu_save_default_emits_native_config_payload() {
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
    assert!(payload["activeBehavior"].is_null());
    assert_eq!(payload["runtimeConfig"]["activeBehavior"], "life");
    assert!(payload["runtimeConfig"]["xyTouch"].is_null());
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
pub(crate) fn load_default_result_applies_native_config_payload() {
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
pub(crate) fn midi_store_results_update_native_snapshot_state() {
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
pub(crate) fn midi_output_menu_selects_dynamic_port() {
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
    runner.menu.state.stack = vec![5, 4, 2];
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
pub(crate) fn entering_midi_port_groups_requests_port_lists() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 4];
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

    runner.menu.state.stack = vec![5, 4];
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
pub(crate) fn preset_load_menu_selects_dynamic_preset() {
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
