use super::*;

#[test]
pub(crate) fn save_current_uses_loaded_preset_name() {
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
            request_snapshot: None,
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
pub(crate) fn native_store_and_action_toasts_cover_common_confirmation_results() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    runner.menu.state.stack = vec![5, 0, 0];
    runner.menu.state.cursor = 1;
    let messages = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert!(!messages
        .iter()
        .any(|message| matches!(message, RunnerMessage::PlatformEffects { .. })));
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "No preset loaded"
    );

    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadPresetResult {
                name: "Alpha".into(),
                payload: Some(json!({ "runtimeConfig": { "activeBehavior": "sequencer" } })),
            },
        })
        .unwrap();
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Loaded Alpha"
    );

    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SavePresetResult {
                name: "Alpha".into(),
                outcome: "overwritten".into(),
            },
        })
        .unwrap();
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Saved Alpha"
    );

    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::DeletePresetResult {
                name: "Alpha".into(),
                ok: true,
            },
        })
        .unwrap();
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Deleted Alpha"
    );
}

#[test]
pub(crate) fn midi_panic_and_synth_preset_actions_show_toasts() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let effect = runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect("midi.panic".into()))
        .unwrap();
    assert_eq!(effect, Some(RuntimePlatformEffect::MidiPanic));
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "MIDI panic sent"
    );

    runner.load_synth_preset(0, "lead");
    assert_eq!(
        runner.display.toast.as_ref().unwrap().message,
        "Loaded synth lead"
    );
}

#[test]
pub(crate) fn preset_save_as_uses_text_draft_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.preset_draft_name = "Jam A".into();
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![5, 0, 0, 0];
    runner.menu.state.cursor = 1;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    assert_eq!(snapshot_from(&opened)["display"]["title"], "Confirm Save");
    let messages = confirm_current_dialog(&mut runner);

    assert_eq!(runner.preset_draft_name, "Jam A");
    let payload = messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => {
                effects.iter().find_map(|effect| match effect {
                    RuntimePlatformEffect::StoreSavePreset {
                        name,
                        mode,
                        payload,
                    } if name == "Jam A" && mode.is_none() => Some(payload),
                    _ => None,
                })
            }
            _ => None,
        })
        .expect("preset save payload");
    assert_eq!(payload["kind"], "octessera.patch");
    assert_eq!(payload["schemaVersion"], 1);
}

#[test]
pub(crate) fn fresh_save_as_preset_name_uses_timestamp_format() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    assert!(is_timestamp_preset_name(&runner.preset_draft_name));
    assert!(is_timestamp_preset_name(&clean_preset_name("   ")));
}

#[test]
pub(crate) fn preset_save_as_uses_fresh_timestamp_draft_name() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let draft_name = runner.preset_draft_name.clone();
    assert!(is_timestamp_preset_name(&draft_name));
    runner.menu.state.stack = vec![5, 0, 0, 0];
    runner.menu.state.cursor = 1;

    let opened = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
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

pub(crate) fn is_timestamp_preset_name(name: &str) -> bool {
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
pub(crate) fn preset_rename_pick_sets_new_name_and_apply_saves() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.preset_names = vec!["Alpha".into()];
    runner.menu.rebuild(runner.menu_config());
    runner.menu.state.stack = vec![5, 0, 0, 3];
    runner.menu.state.cursor = 0;

    let _ = runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
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
            request_snapshot: None,
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
