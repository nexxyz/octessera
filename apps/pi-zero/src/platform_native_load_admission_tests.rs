use super::*;
use std::sync::mpsc;

#[test]
fn queued_native_autosave_blocks_default_load_until_its_write_finishes() {
    let (service, root) = service_and_root("load-admission");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    service
        .enqueue(crate::platform_service::PlatformJob::new(
            playback_runtime::RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
                "autosave-gate".into(),
                None,
            ),
            crate::platform_service::PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();

    let payload = runner
        .capture_config_snapshot()
        .into_local_patch_payload()
        .unwrap();
    let snapshot = runner.capture_config_snapshot();
    let request =
        playback.next_native_store_request(RuntimeOperation::StoreSaveDefault, snapshot.revision());
    assert!(runner.register_native_default_write(request.request_id(), request.revision(), true));
    service
        .enqueue_native_autosave(NativeAutosaveWrite {
            default: Some(request),
            backup: None,
            snapshot: Box::new(snapshot),
            generation: service.store_write_generation(),
        })
        .unwrap();
    assert_eq!(
        service.load_default_now().unwrap_err(),
        "Save pending, try again"
    );
    release_tx.send(()).unwrap();
    let results = collect_results(&service, &mut runner, 1);
    assert!(matches!(
        results.as_slice(),
        [RuntimeStoreResult::Identified { result, .. }]
            if matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, .. })
    ));
    assert_eq!(
        crate::platform_service::platform_service_store::load_json(
            &root.join("store/default.patch.json")
        )
        .unwrap(),
        Some(payload)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn queued_native_manual_default_save_blocks_default_load_until_its_write_finishes() {
    let (service, root) = service_and_root("manual-load-admission");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    service
        .enqueue(crate::platform_service::PlatformJob::new(
            playback_runtime::RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
                "manual-gate".into(),
                None,
            ),
            crate::platform_service::PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();

    let payload = runner
        .capture_config_snapshot()
        .into_local_patch_payload()
        .unwrap();
    let snapshot = runner.capture_config_snapshot();
    let request =
        playback.next_native_store_request(RuntimeOperation::StoreSaveDefault, snapshot.revision());
    assert!(runner.register_native_default_write(request.request_id(), request.revision(), false));
    service.enqueue_native_default(request, snapshot).unwrap();
    assert_eq!(
        service.load_default_now().unwrap_err(),
        "Save pending, try again"
    );
    release_tx.send(()).unwrap();
    let results = collect_results(&service, &mut runner, 1);
    assert!(matches!(
        results.as_slice(),
        [RuntimeStoreResult::Identified { result, .. }]
            if matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, .. })
    ));
    assert_eq!(
        crate::platform_service::platform_service_store::load_json(
            &root.join("store/default.patch.json")
        )
        .unwrap(),
        Some(payload)
    );
    let _ = std::fs::remove_dir_all(root);
}
