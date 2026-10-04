use super::*;

#[test]
fn restore_barrier_cancels_store_writes_already_waiting_in_worker() {
    use std::time::Duration;

    let root = crate::test_temp_dir::unique_temp_path("octessera-platform-restore-barrier");
    let store = root.join("store");
    let service = PiPlatformService::new(store.clone(), root.join("samples"));
    std::fs::create_dir_all(store.join("patches")).unwrap();
    let mut restored_full = crate::user_data_archive::canonical_defaults();
    restored_full["runtimeConfig"]["bpm"] = serde_json::json!(88);
    let restored_documents =
        playback_runtime::split_system_patch_documents(&restored_full).unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&restored_documents.system).unwrap(),
    )
    .unwrap();
    std::fs::write(
        store.join("default.patch.json"),
        serde_json::to_vec(&restored_documents.patch).unwrap(),
    )
    .unwrap();
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
                "restore-gate".into(),
                None,
            ),
            PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::StoreSaveDefault {
                    payload: serde_json::json!({"stale": "default"}),
                    mode: None,
                },
                "stale-default".into(),
                Some(1),
            ),
            PlatformJobKind::SaveDefault {
                payload: serde_json::json!({"stale": "default"}),
                is_auto: None,
            },
        ))
        .unwrap();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::StoreSavePreset {
                    name: "stale-preset".into(),
                    payload: serde_json::json!({"stale": "preset"}),
                    mode: None,
                },
                "stale-preset".into(),
                Some(2),
            ),
            PlatformJobKind::SavePreset {
                name: "stale-preset".into(),
                payload: serde_json::json!({"stale": "preset"}),
            },
        ))
        .unwrap();
    service.store_write_barrier.invalidate();
    assert_eq!(
        service.load_default_now().unwrap(),
        Some(restored_documents.patch.clone())
    );
    release_tx.send(()).unwrap();
    let barrier = service.enqueue_test_barrier().unwrap();
    barrier.recv_timeout(Duration::from_secs(1)).unwrap();
    let results = service.drain_results(8);
    assert_eq!(
        results
            .iter()
            .filter_map(|message| match message {
                HostMessage::RuntimeResult {
                    result:
                        RuntimeStoreResult::Identified {
                            request_id, result, ..
                        },
                } => Some((request_id.as_str(), result.as_ref())),
                _ => None,
            })
            .filter(|(_, result)| matches!(result, RuntimeStoreResult::RuntimeFailure { .. }))
            .map(|(request_id, _)| request_id)
            .collect::<Vec<_>>(),
        vec!["stale-default", "stale-preset"]
    );
    assert_eq!(
        load_json(&store.join("default.patch.json")).unwrap(),
        Some(restored_documents.patch)
    );
    assert!(!store.join("patches").join("stale-preset.json").exists());
    assert!(service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::StoreSaveSystem {
                    payload: serde_json::json!({"stale": true}),
                },
                "blocked-system".into(),
                None,
            ),
            PlatformJobKind::SaveSystem {
                payload: serde_json::json!({"stale": true}),
            },
        ))
        .is_err());

    service.acknowledge_restored_state();
    std::fs::create_dir_all(store.join("patches")).unwrap();
    let fresh_payload = playback_runtime::split_system_patch_documents(
        &crate::user_data_archive::canonical_defaults(),
    )
    .unwrap()
    .patch;
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::StoreSaveDefault {
                    payload: fresh_payload.clone(),
                    mode: None,
                },
                "fresh-default".into(),
                None,
            ),
            PlatformJobKind::SaveDefault {
                payload: fresh_payload.clone(),
                is_auto: None,
            },
        ))
        .unwrap();
    let barrier = service.enqueue_test_barrier().unwrap();
    barrier.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(
        load_json(&store.join("default.patch.json")).unwrap(),
        Some(fresh_payload)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn restore_barrier_cancels_native_default_snapshot_waiting_in_worker() {
    use playback_runtime::{NativeRunner, NativeRunnerConfig, RuntimeOperation};
    use std::time::Duration;

    let root = crate::test_temp_dir::unique_temp_path("octessera-platform-native-restore");
    let store = root.join("store");
    let service = PiPlatformService::new(store.clone(), root.join("samples"));
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let snapshot = runner.capture_config_snapshot();
    let request =
        playback_runtime::PlaybackRuntime::new(playback_runtime::RuntimeConfig::default())
            .next_native_store_request(RuntimeOperation::StoreSaveDefault, snapshot.revision());
    let store_guard = service.store_lock.lock().unwrap();
    service
        .enqueue_native_default(request.clone(), snapshot)
        .unwrap();
    service.store_write_barrier.invalidate();
    drop(store_guard);

    let barrier = service.enqueue_test_barrier().unwrap();
    barrier.recv_timeout(Duration::from_secs(1)).unwrap();
    let result = service
        .drain_platform_results(4)
        .into_iter()
        .find_map(|result| match result {
            PlatformResult::NativeDefaultCompletion(completion) => Some(completion.result),
            PlatformResult::NativePresetCompletion(_) => None,
            PlatformResult::Legacy(_) => None,
        })
        .unwrap();
    assert!(matches!(
        result,
        RuntimeStoreResult::Identified { request_id, result, .. }
            if request_id == request.request_id()
                && matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { .. })
    ));
    assert!(!store.join("default.patch.json").exists());
    drop(service);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn native_default_registration_rejection_does_not_queue_or_replace_default() {
    use playback_runtime::{
        NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig, RuntimeOperation,
        RuntimeStoreResult,
    };
    use std::time::Duration;

    let (root, original_bytes, mut adapter) = native_default_test_adapter();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let (rejected, snapshot) = next_native_default_request(&mut playback, &runner);
    assert!(runner.register_native_default_write(
        rejected.request_id(),
        rejected.revision(),
        false,
    ));
    let failure = crate::platform_service::platform_native_persistence::submit_native_default(
        &adapter.core.platform_service,
        &mut runner,
        rejected.clone(),
        snapshot,
    )
    .expect("duplicate registration must fail before queueing");
    assert!(matches!(
        &failure,
        playback_runtime::HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                request_id,
                revision: Some(revision),
                result,
            }
        } if request_id == rejected.request_id()
            && *revision == rejected.revision()
            && matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error }
                if error.operation == RuntimeOperation::StoreSaveDefault)
    ));
    dispatch_result(&mut playback, &mut runner, &mut adapter, failure);
    adapter
        .core
        .platform_service
        .enqueue_test_barrier()
        .unwrap()
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        std::fs::read(root.join("store/default.patch.json")).unwrap(),
        original_bytes
    );
    assert!(adapter
        .core
        .platform_service
        .native_default_write()
        .is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn native_default_queue_rejection_clears_pending_and_allows_retry() {
    use playback_runtime::{
        NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig, RuntimeOperation,
        RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
    };
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let (root, original_bytes, mut adapter) = native_default_test_adapter();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    adapter
        .core
        .platform_service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                RuntimePlatformEffect::SystemInfoRequest,
                "gate".into(),
                None,
            ),
            PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    for index in 0..32 {
        adapter
            .core
            .platform_service
            .enqueue(PlatformJob::new(
                RuntimePlatformRequest::new(
                    RuntimePlatformEffect::SystemInfoRequest,
                    format!("queued-{index}"),
                    None,
                ),
                PlatformJobKind::TestBarrier {
                    completed: mpsc::sync_channel(1).0,
                },
            ))
            .unwrap();
    }

    let (queue_rejected, snapshot) = next_native_default_request(&mut playback, &runner);
    let failure = crate::platform_service::platform_native_persistence::submit_native_default(
        &adapter.core.platform_service,
        &mut runner,
        queue_rejected.clone(),
        snapshot,
    )
    .expect("full queue must reject the native write");
    assert!(matches!(
        &failure,
        playback_runtime::HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, result, .. }
        } if request_id == queue_rejected.request_id()
            && matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error }
                if error.operation == RuntimeOperation::StoreSaveDefault)
    ));
    dispatch_result(&mut playback, &mut runner, &mut adapter, failure);
    assert!(adapter
        .core
        .platform_service
        .native_default_write()
        .is_none());
    release_tx.send(()).unwrap();
    let barrier_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match adapter.core.platform_service.enqueue_test_barrier() {
            Ok(barrier) => {
                barrier.recv_timeout(Duration::from_secs(1)).unwrap();
                break;
            }
            Err(_) if Instant::now() < barrier_deadline => {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(error) => panic!("platform queue did not drain: {error}"),
        }
    }
    assert_eq!(
        std::fs::read(root.join("store/default.patch.json")).unwrap(),
        original_bytes
    );

    let (retry, snapshot) = next_native_default_request(&mut playback, &runner);
    assert!(
        crate::platform_service::platform_native_persistence::submit_native_default(
            &adapter.core.platform_service,
            &mut runner,
            retry.clone(),
            snapshot,
        )
        .is_none()
    );
    assert_eq!(
        adapter.core.platform_service.native_default_write(),
        Some(retry.clone())
    );

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut saved = false;
    while Instant::now() < deadline && !saved {
        for message in adapter.drain_platform_results_for_runner(&mut runner, 4) {
            saved |= matches!(
                &message,
                playback_runtime::HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified {
                        request_id,
                        result,
                        ..
                    }
                } if request_id == retry.request_id()
                    && matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, .. })
            );
            dispatch_result(&mut playback, &mut runner, &mut adapter, message);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(saved, "retry after queue rejection did not complete");
    assert!(adapter
        .core
        .platform_service
        .load_default_now()
        .unwrap()
        .is_some());
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn native_default_test_adapter() -> (
    std::path::PathBuf,
    Vec<u8>,
    crate::host_adapter::PiHostAdapter,
) {
    let root = crate::test_temp_dir::unique_temp_path("octessera-native-default-rejection");
    let documents = playback_runtime::split_system_patch_documents(
        &crate::user_data_archive::canonical_defaults(),
    )
    .unwrap();
    let original_bytes = serde_json::to_vec(&documents.patch).unwrap();
    std::fs::create_dir_all(root.join("store")).unwrap();
    std::fs::write(
        root.join("store/system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    std::fs::write(root.join("store/default.patch.json"), &original_bytes).unwrap();
    let adapter = crate::host_adapter::PiHostAdapter::new(
        None,
        root.join("store"),
        root.join("samples"),
        std::sync::Arc::new(|_| {}),
        false,
        crate::usb_config::UsbAudioOut::Jack,
    );
    (root, original_bytes, adapter)
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn next_native_default_request(
    playback: &mut playback_runtime::PlaybackRuntime,
    runner: &playback_runtime::NativeRunner,
) -> (
    playback_runtime::NativeStoreRequest,
    playback_runtime::NativeConfigSnapshot,
) {
    let snapshot = runner.capture_config_snapshot();
    let request = playback.next_native_store_request(
        playback_runtime::RuntimeOperation::StoreSaveDefault,
        snapshot.revision(),
    );
    (request, snapshot)
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn dispatch_result(
    playback: &mut playback_runtime::PlaybackRuntime,
    runner: &mut playback_runtime::NativeRunner,
    adapter: &mut crate::host_adapter::PiHostAdapter,
    result: playback_runtime::HostMessage,
) {
    playback
        .dispatch_host_message_music_first(result, runner, adapter)
        .unwrap();
}
