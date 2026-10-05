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
            mode: None,
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
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn source_delete_failure_keeps_both_presets_and_reports_partial_cleanup() {
    let (service, root) = service_and_root("rename-cleanup-failure");
    let store = root.join("store");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let expected = runner
        .capture_config_snapshot()
        .into_portable_patch_payload()
        .unwrap();
    let source_path = crate::platform_service::preset_patch_path(&store, "Source").unwrap();
    let source_bytes = write_patch(&store, "Source", &expected);
    let mut source_lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&source_path)
        .unwrap();
    let _request = enqueue_preset(
        &service,
        &mut playback,
        &mut runner,
        NativeManualSaveRequest::Preset {
            name: "Target".into(),
            mode: None,
            rename_from: Some("Source".into()),
        },
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let raw = loop {
        if let Some(result) = service.drain_platform_results(8).into_iter().next() {
            break result;
        }
        assert!(Instant::now() < deadline, "preset cleanup result timed out");
        std::thread::sleep(Duration::from_millis(2));
    };
    let PlatformResult::NativePresetCompletion(completion) = raw else {
        panic!("rename target should be saved before source cleanup result");
    };
    assert!(matches!(
        &completion.result,
        RuntimeStoreResult::Identified { result, .. }
            if matches!(result.as_ref(), RuntimeStoreResult::SavePresetResult { name, .. } if name == "Target")
    ));
    let cleanup_error = completion.cleanup_error.clone().unwrap();
    assert!(cleanup_error.contains("Source"));
    assert!(cleanup_error.contains("could not be removed"));
    assert_eq!(
        completion.names,
        vec!["Source".to_string(), "Target".to_string()]
    );
    let message = finish_platform_result(
        &service,
        &mut runner,
        PlatformResult::NativePresetCompletion(completion),
    )
    .unwrap();
    playback
        .dispatch_host_message_music_first(message, &mut runner, &mut TestHost)
        .unwrap();
    let toast = runner.capture_display_scene().unwrap().into_snapshot()["display"]["toast"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(toast.contains("Target") && toast.contains("clean"));
    assert_eq!(
        crate::platform_service::load_json(
            &crate::platform_service::preset_patch_path(&store, "Target").unwrap()
        )
        .unwrap(),
        Some(expected)
    );
    source_lock.seek(SeekFrom::Start(0)).unwrap();
    let mut current_source = Vec::new();
    source_lock.read_to_end(&mut current_source).unwrap();
    assert_eq!(current_source, source_bytes);
    drop(source_lock);
    assert_eq!(
        crate::platform_service::list_presets(&store).unwrap(),
        vec!["Source".to_string(), "Target".to_string()]
    );
    cleanup(root);
}
