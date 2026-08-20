use super::*;

#[test]
fn restore_barrier_cancels_store_writes_already_waiting_in_worker() {
    use std::time::Duration;

    let root = std::env::temp_dir().join(format!(
        "octessera-platform-restore-barrier-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = root.join("store");
    let service = PiPlatformService::new(store.clone(), root.join("samples"));
    let store_guard = service.store_lock.lock().unwrap();
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
    drop(store_guard);

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
    assert!(!store.join("default.json").exists());
    assert!(!store.join("patches").join("stale-preset.json").exists());

    service.acknowledge_restored_state();
    std::fs::create_dir_all(store.join("patches")).unwrap();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::StoreSaveDefault {
                    payload: serde_json::json!({"fresh": true}),
                    mode: None,
                },
                "fresh-default".into(),
                None,
            ),
            PlatformJobKind::SaveDefault {
                payload: serde_json::json!({"fresh": true}),
                is_auto: None,
            },
        ))
        .unwrap();
    let barrier = service.enqueue_test_barrier().unwrap();
    barrier.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(
        load_json(&store.join("default.json")).unwrap(),
        Some(serde_json::json!({"fresh": true}))
    );
    let _ = std::fs::remove_dir_all(root);
}
