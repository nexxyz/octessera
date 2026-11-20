use super::*;

#[test]
fn a_confirmed_rename_moves_the_source_preset_over_an_existing_target() {
    let (service, root) = service_and_root("rename-target-overwrite");
    let store = root.join("store");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let loaded = loaded_patch(&runner);
    let source_bytes = write_patch(&store, "Source", &json!({"stored": "Source"}));
    write_patch(&store, "Target", &json!({"stored": "Target"}));
    let request = enqueue_preset(
        &service,
        &mut playback,
        &mut runner,
        NativeManualSaveRequest::Preset {
            name: "Target".into(),
            mode: Some(playback_runtime::PresetSaveMode::Overwrite),
            rename_from: Some("Source".into()),
        },
    );
    let message = receive_preset_result(&service, &mut runner, Duration::from_secs(2));
    let HostMessage::RuntimeResult {
        result:
            RuntimeStoreResult::Identified {
                result,
                request_id,
                revision: Some(revision),
            },
    } = &message
    else {
        panic!("preset overwrite must return an identified result");
    };
    assert_eq!(request_id, request.request_id());
    assert_eq!(*revision, request.revision());
    assert!(matches!(
        result.as_ref(),
        RuntimeStoreResult::RenamePresetResult { from, name }
            if from == "Source" && name == "Target"
    ));
    assert!(!store.join("patches/Source.json").exists());
    assert_eq!(
        std::fs::read(store.join("patches/Target.json")).unwrap(),
        source_bytes
    );
    assert_eq!(
        crate::platform_service::list_presets(&store).unwrap(),
        vec!["Target"]
    );
    playback
        .dispatch_host_message_music_first(message, &mut runner, &mut TestHost)
        .unwrap();
    assert!(service.native_preset_write().is_none());
    assert_eq!(loaded_patch(&runner), loaded);
    cleanup(root);
}

#[test]
fn a_rename_onto_an_existing_preset_without_overwrite_keeps_both_presets() {
    let (service, root) = service_and_root("rename-existing-refused");
    let store = root.join("store");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let source_bytes = write_patch(&store, "Source", &json!({"stored": "Source"}));
    let target_bytes = write_patch(&store, "Target", &json!({"stored": "Target"}));
    enqueue_preset(
        &service,
        &mut playback,
        &mut runner,
        NativeManualSaveRequest::Preset {
            name: "Target".into(),
            mode: None,
            rename_from: Some("Source".into()),
        },
    );
    let message = receive_preset_result(&service, &mut runner, Duration::from_secs(2));
    assert!(matches!(
        &message,
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { result, .. }
        } if matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error }
            if error.message.as_deref() == Some("Rename Source failed: Target already exists"))
    ));
    assert_eq!(
        std::fs::read(store.join("patches/Source.json")).unwrap(),
        source_bytes
    );
    assert_eq!(
        std::fs::read(store.join("patches/Target.json")).unwrap(),
        target_bytes
    );
    cleanup(root);
}

#[test]
fn save_onto_an_existing_preset_without_overwrite_is_refused_and_shown() {
    let (service, root) = service_and_root("save-existing-refused");
    let store = root.join("store");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let patch = runner
        .capture_config_snapshot()
        .into_portable_patch_payload()
        .unwrap();
    let source_bytes = write_patch(&store, "Source", &patch);
    let target_bytes = write_patch(&store, "Target", &json!({"preserve": true}));
    enqueue_preset(
        &service,
        &mut playback,
        &mut runner,
        NativeManualSaveRequest::Preset {
            name: "Target".into(),
            mode: None,
            rename_from: None,
        },
    );
    let message = receive_preset_result(&service, &mut runner, Duration::from_secs(2));
    let HostMessage::RuntimeResult {
        result: RuntimeStoreResult::Identified { result, .. },
    } = &message
    else {
        panic!("refused overwrite must return an identified result");
    };
    assert!(matches!(
        result.as_ref(),
        RuntimeStoreResult::RuntimeFailure { error }
            if error.operation == RuntimeOperation::StoreSavePreset
                && error.message.as_deref() == Some("Save Target failed: preset already exists")
    ));
    assert_eq!(
        std::fs::read(store.join("patches/Source.json")).unwrap(),
        source_bytes
    );
    assert_eq!(
        std::fs::read(store.join("patches/Target.json")).unwrap(),
        target_bytes
    );
    playback
        .dispatch_host_message_music_first(message, &mut runner, &mut TestHost)
        .unwrap();
    assert!(service.native_preset_write().is_none());
    let display = &runner.capture_display_scene().unwrap().into_snapshot()["display"];
    assert_eq!(display["title"], "CAN'T SAVE");
    assert_eq!(display["lines"][0], "Target was not");
    assert_eq!(display["lines"][3], "exists.");
    assert_eq!(display["lines"][5], "store_save_preset");
    cleanup(root);
}
