use super::super::super::{PlatformJob, PlatformJobKind};
use super::*;
use playback_runtime::{RuntimePlatformEffect, RuntimePlatformRequest};

#[test]
fn queue_failure_returns_an_identified_error_without_retrying() {
    let (service, root) = service_and_root("queue-retry");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = runner_with_aux_mapping(true, false);
    let mut pending = PendingPiPersistence::default();
    aux_turn(&mut runner, -1);
    std::thread::sleep(Duration::from_millis(160));
    let eligible_at = Instant::now();
    let due = eligible_at + Duration::from_secs(2);
    apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        eligible_at,
    );

    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                RuntimePlatformEffect::SystemInfoRequest,
                "autosave-gate".into(),
                None,
            ),
            PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    let mut completed = Vec::new();
    for index in 0..32 {
        let (completed_tx, completed_rx) = std::sync::mpsc::sync_channel(1);
        completed.push(completed_rx);
        service
            .enqueue(PlatformJob::new(
                RuntimePlatformRequest::new(
                    RuntimePlatformEffect::SystemInfoRequest,
                    format!("autosave-fill-{index}"),
                    None,
                ),
                PlatformJobKind::TestBarrier {
                    completed: completed_tx,
                },
            ))
            .unwrap();
    }
    let failures = apply_autosave_at(&mut pending, &service, &mut playback, &mut runner, due);
    assert_eq!(failures.len(), 1);
    assert!(matches!(
        &failures[0],
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, revision: Some(_), result }
        } if request_id.starts_with("platform-")
            && matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error }
                if error.operation == RuntimeOperation::StoreSaveDefault)
    ));
    runner
        .send_music_first(failures.into_iter().next().unwrap())
        .unwrap();
    assert!(service.native_default_write().is_none());
    assert!(!pending.is_pending());

    release_tx.send(()).unwrap();
    for receiver in completed {
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    }
    assert!(apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        due + Duration::from_secs(1),
    )
    .is_empty());
    assert!(service.native_default_write().is_none());
    assert!(!root.join("store/default.patch.json").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn worker_failure_is_reported_without_an_automatic_retry() {
    let (service, root) = service_and_root("worker-retry");
    let store = root.join("store");
    std::fs::create_dir_all(store.join("default.patch.json")).unwrap();
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = runner_with_aux_mapping(true, false);
    let mut pending = PendingPiPersistence::default();
    aux_turn(&mut runner, -1);
    std::thread::sleep(Duration::from_millis(160));
    let eligible_at = Instant::now();
    let due = eligible_at + Duration::from_secs(2);
    apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        eligible_at,
    );
    apply_autosave_at(&mut pending, &service, &mut playback, &mut runner, due);
    let failed = collect_results(&service, &mut runner, 1);
    assert!(matches!(
        &failed[0],
        RuntimeStoreResult::Identified { result, .. }
            if matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error }
                if error.operation == RuntimeOperation::StoreSaveDefault)
    ));
    assert!(service.native_default_write().is_none());
    assert!(!pending.is_pending());
    std::fs::remove_dir(store.join("default.patch.json")).unwrap();

    assert!(apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        Instant::now() + Duration::from_secs(10),
    )
    .is_empty());
    assert!(service.native_default_write().is_none());
    assert!(!store.join("default.patch.json").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn restore_generation_invalidates_pending_native_metadata() {
    let (service, root) = service_and_root("restore-invalidate");
    let mut runner = runner_with_aux_mapping(true, false);
    aux_turn(&mut runner, -1);
    std::thread::sleep(Duration::from_millis(160));
    let now = Instant::now();
    let intent = runner.persistence_intent_at(now).unwrap();
    let mut pending = PendingPiPersistence::default();
    pending.observe_native(Some(intent), now, service.store_write_generation(), false);
    assert!(pending.is_pending());
    service.invalidate_store_writes_for_test();
    pending.cancel_if_invalid(
        service.store_write_generation(),
        service.store_writes_blocked(),
    );
    assert!(!pending.is_pending());
    assert!(service.native_default_write().is_none());
    let _ = std::fs::remove_dir_all(root);
}
