use super::*;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;

#[test]
fn locked_existing_target_returns_identified_write_failure_without_deleting_source() {
    let (service, root) = service_and_root("rename-write-failure");
    let store = root.join("store");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let source_payload = runner
        .capture_config_snapshot()
        .into_portable_patch_payload()
        .unwrap();
    let source_bytes = write_patch(&store, "Source", &source_payload);
    let target_bytes = write_patch(&store, "Target", &json!({"original": true}));
    let target_path = store.join("patches/Target.json");
    let mut target_lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(target_path)
        .unwrap();
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
        panic!("target I/O failure must be identified");
    };
    assert_eq!(request_id, request.request_id());
    assert_eq!(*revision, request.revision());
    assert!(matches!(
        result.as_ref(),
        RuntimeStoreResult::RuntimeFailure { error }
            if error.operation == RuntimeOperation::StoreSavePreset
                && error.message.as_deref().is_some_and(|message| !message.contains("already exists"))
    ));
    assert_eq!(
        std::fs::read(store.join("patches/Source.json")).unwrap(),
        source_bytes
    );
    target_lock.seek(SeekFrom::Start(0)).unwrap();
    let mut actual_target = Vec::new();
    target_lock.read_to_end(&mut actual_target).unwrap();
    assert_eq!(actual_target, target_bytes);
    playback
        .dispatch_host_message_music_first(message, &mut runner, &mut TestHost)
        .unwrap();
    assert!(service.native_preset_write().is_none());
    drop(target_lock);
    cleanup(root);
}

#[test]
fn a_locked_source_fails_the_rename_and_keeps_the_source_preset() {
    let (service, root) = service_and_root("rename-locked-source");
    let store = root.join("store");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let source_path = crate::platform_service::preset_patch_path(&store, "Source").unwrap();
    let source_bytes = write_patch(&store, "Source", &json!({"stored": "Source"}));
    let mut source_lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&source_path)
        .unwrap();
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
            if error.operation == RuntimeOperation::StoreSavePreset
                && error.message.as_deref().is_some_and(|message| message.starts_with("Rename Source failed")))
    ));
    source_lock.seek(SeekFrom::Start(0)).unwrap();
    let mut current_source = Vec::new();
    source_lock.read_to_end(&mut current_source).unwrap();
    assert_eq!(current_source, source_bytes);
    drop(source_lock);
    assert_eq!(
        crate::platform_service::list_presets(&store).unwrap(),
        vec!["Source".to_string()]
    );
    cleanup(root);
}
