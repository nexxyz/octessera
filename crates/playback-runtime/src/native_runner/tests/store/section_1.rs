use super::*;

#[test]
pub(crate) fn system_menu_save_default_emits_native_config_payload() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.state.stack = vec![5, 0, 1];
    runner.menu.state.cursor = 0;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
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
    assert!(payload["runtimeConfig"]["sparksXyTouch"].is_null());
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
    runner.transport.transport = RuntimeTransportState::Playing;
    runner.transport.current_ppqn_pulse = 96;
    let payload = json!({
        "activeBehavior": "sequencer",
        "runtimeConfig": {
            "activeLayerIndex": 1,
            "layers": [
                { "worlds": { "behaviorId": "life" }, "name": "life" },
                { "worlds": { "behaviorId": "sequencer" }, "name": "sequencer" }
            ],
            "instruments": [
                { "type": "sampler", "name": "sampler", "noteBehavior": "hold", "autoName": true, "mixer": { "volume": 70, "panPos": 10 } }
            ],
            "masterVolume": 88,
            "displayBrightness": 66,
            "buttonBrightness": 55,
            "sparksMode": "pan",
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

    assert_eq!(runner.active_layer_index, 1);
    assert_eq!(runner.behavior.id(), "sequencer");
    assert_eq!(runner.instruments[0].kind, "sampler");
    assert_eq!(runner.instruments[0].note_behavior, "hold");
    assert_eq!(runner.note_behaviors[0], NoteBehavior::Hold);
    assert_eq!(runner.display.ui.master_volume, 88);
    assert_eq!(runner.transport.sync_source, SyncSource::External);
    assert_eq!(runner.transport.transport, RuntimeTransportState::Stopped);
    assert_eq!(runner.transport.current_ppqn_pulse, 0);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.contains(&RuntimePlatformEffect::MidiPanic)
    )));
    assert_eq!(snapshot_from(&messages)["activeBehavior"], "sequencer");
}

#[test]
pub(crate) fn patch_and_device_payloads_split_local_device_fields() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.display.ui.display_brightness = 44;
    runner.usb_audio_out = "uac2".into();
    runner.midi_enabled = true;
    runner.audio_output_buffer_frames = 512;

    let patch = runner.patch_payload();
    assert_eq!(patch["kind"], "octessera.patch");
    assert_eq!(patch["schemaVersion"], 1);
    assert!(patch["runtimeConfig"]["usb"].is_null());
    assert!(patch["runtimeConfig"]["midi"].is_null());
    assert!(patch["runtimeConfig"]["displayBrightness"].is_null());
    assert!(patch["runtimeConfig"]["sound"]["audioOutputBufferFrames"].is_null());
    assert!(patch["runtimeConfig"]["auxBindings"].is_object());

    let device = runner.device_config_payload();
    assert_eq!(device["runtimeConfig"]["usb"]["audioOut"], "uac2");
    assert_eq!(device["runtimeConfig"]["midi"]["enabled"], true);
    assert_eq!(device["runtimeConfig"]["displayBrightness"], 44);
    assert_eq!(
        device["runtimeConfig"]["sound"]["audioOutputBufferFrames"],
        512
    );
}

#[test]
pub(crate) fn legacy_full_preset_load_preserves_device_fields() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.display.ui.display_brightness = 22;
    runner.usb_audio_out = "both".into();
    runner.midi_enabled = false;
    runner.audio_output_buffer_frames = 256;

    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: "Legacy".into(),
                payload: Some(json!({
                    "runtimeConfig": {
                        "activeBehavior": "sequencer",
                        "layers": [{ "worlds": { "behaviorId": "sequencer" } }],
                        "displayBrightness": 88,
                        "usb": { "audioOut": "uac2", "midiOutEnabled": true },
                        "midi": { "enabled": true },
                        "sound": { "audioOutputBufferFrames": 1024 }
                    }
                })),
            },
        })
        .unwrap();

    assert_eq!(runner.behavior.id(), "sequencer");
    assert_eq!(runner.display.ui.display_brightness, 22);
    assert_eq!(runner.usb_audio_out, "both");
    assert!(!runner.midi_enabled);
    assert_eq!(runner.audio_output_buffer_frames, 256);
}

#[test]
pub(crate) fn full_default_load_and_device_apply_update_device_fields() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(json!({
            "runtimeConfig": {
                "displayBrightness": 77,
                "midi": { "enabled": true },
                "sound": { "audioOutputBufferFrames": 1024 }
            }
        }))
        .unwrap();
    assert_eq!(runner.display.ui.display_brightness, 77);
    assert!(runner.midi_enabled);
    assert_eq!(runner.audio_output_buffer_frames, 1024);

    runner
        .apply_device_config_payload_preserving_patch(json!({
            "runtimeConfig": {
                "displayBrightness": 33,
                "activeBehavior": "sequencer"
            }
        }))
        .unwrap();
    assert_eq!(runner.display.ui.display_brightness, 33);
    assert_ne!(runner.behavior.id(), "sequencer");
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
            request_snapshot: None,
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
            request_snapshot: None,
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
            request_snapshot: None,
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
            request_snapshot: None,
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
