use super::*;
use playback_runtime::RuntimePlatformEffect;
use std::sync::mpsc;

#[test]
fn stale_preset_result_does_not_attach_catalog_or_clear_newer_rename() {
    let (service, root) = service_and_root("stale-rename-result");
    let store = root.join("store");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let patch = runner
        .capture_config_snapshot()
        .into_portable_patch_payload()
        .unwrap();
    let current_source = write_patch(&store, "CurrentSource", &patch);
    write_patch(&store, "OldSource", &patch);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    service
        .enqueue(crate::platform_service::PlatformJob::new(
            RuntimePlatformRequest::new(
                RuntimePlatformEffect::SystemInfoRequest,
                "stale-preset-gate".into(),
                None,
            ),
            crate::platform_service::PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();

    let old = enqueue_preset(
        &service,
        &mut playback,
        &mut runner,
        NativeManualSaveRequest::Preset {
            name: "OldTarget".into(),
            mode: None,
            rename_from: Some("OldSource".into()),
        },
    );
    let old_failure = finish_platform_result(
        &service,
        &mut runner,
        PlatformResult::Legacy(failure(&old, "test retry before late completion".into())),
    )
    .unwrap();
    playback
        .dispatch_host_message_music_first(old_failure, &mut runner, &mut TestHost)
        .unwrap();

    let current = enqueue_preset(
        &service,
        &mut playback,
        &mut runner,
        NativeManualSaveRequest::Preset {
            name: "NewTarget".into(),
            mode: None,
            rename_from: Some("CurrentSource".into()),
        },
    );
    let stale = PlatformResult::NativePresetCompletion(NativePresetCompletion {
        result: RuntimeStoreResult::SavePresetResult {
            name: "OldTarget".into(),
            outcome: "created".into(),
        }
        .with_identity(old.request_id().into(), Some(old.revision())),
        names: vec!["OldTarget".into()],
        cleanup_error: None,
    });
    assert!(finish_platform_result(&service, &mut runner, stale).is_none());
    assert_eq!(service.native_preset_write().unwrap().request, current);
    assert!(!runner.register_native_preset_write(
        "native-preset-stale-probe",
        current.revision(),
        NativeManualSaveRequest::Preset {
            name: "probe".into(),
            mode: None,
            rename_from: None,
        },
    ));
    assert_eq!(
        std::fs::read(store.join("patches/CurrentSource.json")).unwrap(),
        current_source
    );
    assert!(!store.join("patches/NewTarget.json").exists());

    release_tx.send(()).unwrap();
    let completion = receive_preset_result(&service, &mut runner, Duration::from_secs(2));
    assert!(matches!(
        &completion,
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, result, .. }
        } if request_id == current.request_id()
            && matches!(result.as_ref(), RuntimeStoreResult::SavePresetResult { name, outcome }
                if name == "NewTarget" && outcome == "created")
    ));
    playback
        .dispatch_host_message_music_first(completion, &mut runner, &mut TestHost)
        .unwrap();
    assert!(store.join("patches/OldTarget.json").is_file());
    assert!(!store.join("patches/OldSource.json").exists());
    assert!(store.join("patches/NewTarget.json").is_file());
    assert!(!store.join("patches/CurrentSource.json").exists());
    cleanup(root);
}
