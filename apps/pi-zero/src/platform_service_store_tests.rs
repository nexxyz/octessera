use super::*;

#[test]
fn list_presets_ignores_noncanonical_files() {
    let dir = std::env::temp_dir().join(format!(
        "octessera-pi-preset-list-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("default.json"), "{}").unwrap();
    std::fs::write(dir.join("recovery-save.json"), "{}").unwrap();
    std::fs::write(dir.join("bak-123.json"), "{}").unwrap();
    std::fs::write(dir.join("bad:name.json"), "{}").unwrap();
    std::fs::write(dir.join("CON.json"), "{}").unwrap();
    std::fs::create_dir_all(dir.join("patches")).unwrap();
    std::fs::write(dir.join("patches").join("safe.json"), "{}").unwrap();

    assert_eq!(list_presets(&dir).unwrap(), vec!["safe".to_string()]);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn preset_store_uses_only_canonical_patch_files() {
    let dir = std::env::temp_dir().join(format!(
        "octessera-pi-preset-patch-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(dir.join("patches")).unwrap();
    std::fs::write(dir.join("patches").join("Jam.json"), r#"{"patch":true}"#).unwrap();

    assert_eq!(list_presets(&dir).unwrap(), vec!["Jam".to_string()]);
    assert_eq!(
        load_json(&preset_patch_path(&dir, "Jam").unwrap()).unwrap(),
        Some(serde_json::json!({ "patch": true }))
    );
    save_json(
        &preset_patch_path(&dir, "New").unwrap(),
        &serde_json::json!({ "kind": "octessera.patch" }),
    )
    .unwrap();
    assert!(dir.join("patches").join("New.json").is_file());
    assert!(!dir.join("New.json").is_file());
    assert!(delete_preset_payload(&dir, "Jam"));
    assert!(!dir.join("patches").join("Jam.json").exists());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn patch_catalog_accepts_valid_names_that_are_special_only_at_store_root() {
    let dir = std::env::temp_dir().join(format!(
        "octessera-pi-preset-special-stems-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let patches = dir.join("patches");
    std::fs::create_dir_all(&patches).unwrap();
    for name in ["default", "bak-jam", "default.patch", "current", "device"] {
        std::fs::write(patches.join(format!("{name}.json")), "{}").unwrap();
    }
    std::fs::write(dir.join("default.json"), "{}").unwrap();
    std::fs::write(dir.join("bak-root.json"), "{}").unwrap();

    assert_eq!(
        list_presets(&dir).unwrap(),
        vec![
            "bak-jam".to_string(),
            "current".to_string(),
            "default".to_string(),
            "default.patch".to_string(),
            "device".to_string(),
        ]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn store_job_waits_for_store_lock() {
    let root = std::env::temp_dir().join(format!(
        "octessera-platform-store-lock-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let service = PiPlatformService::new(root.join("store"), root.join("samples"));
    let store_guard = service.store_lock.lock().unwrap();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::StoreListPresets,
                "store-lock".into(),
                None,
            ),
            PlatformJobKind::ListPresets,
        ))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert!(service.drain_results(4).is_empty());
    drop(store_guard);
    let mut found = false;
    for _ in 0..100 {
        found |= service.drain_results(4).into_iter().any(|message| {
            matches!(
                message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified { request_id, .. }
                } if request_id == "store-lock"
            )
        });
        if found {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(found);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn default_load_rejects_queued_legacy_save_without_cancelling_it() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-default-load-pending-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = root.join("store");
    let service = PiPlatformService::new(store.clone(), root.join("samples"));
    std::fs::create_dir_all(&store).unwrap();
    let documents = playback_runtime::split_system_patch_documents(
        &crate::user_data_archive::canonical_defaults(),
    )
    .unwrap();
    let mut prior = documents.patch.clone();
    prior["runtimeConfig"]["bpm"] = serde_json::json!(90);
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    std::fs::write(
        store.join("default.patch.json"),
        serde_json::to_vec(&prior).unwrap(),
    )
    .unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
                "gate".into(),
                None,
            ),
            PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    let mut payload = documents.patch;
    payload["runtimeConfig"]["bpm"] = serde_json::json!(111);
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::StoreSaveDefault {
                    payload: payload.clone(),
                    mode: None,
                },
                "default-save".into(),
                None,
            ),
            PlatformJobKind::SaveDefault {
                payload: payload.clone(),
                is_auto: None,
            },
        ))
        .unwrap();
    assert_eq!(
        service.load_default_now().unwrap_err(),
        "Save pending, try again"
    );
    assert_eq!(
        load_json(&store.join("default.patch.json")).unwrap(),
        Some(prior)
    );
    assert_eq!(
        service.load_system_now().unwrap(),
        Some(documents.system),
        "System Load must not be blocked by a patch save"
    );
    release_tx.send(()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut completed = false;
    while std::time::Instant::now() < deadline && !completed {
        completed = service.drain_results(4).iter().any(|message| {
            matches!(
                message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified { request_id, result, .. }
                } if request_id == "default-save"
                    && matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, .. })
            )
        });
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(completed);
    assert_eq!(
        load_json(&store.join("default.patch.json")).unwrap(),
        Some(payload)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn named_preset_load_rejects_queued_named_save_without_cancelling_it() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-preset-load-pending-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = root.join("store");
    let service = PiPlatformService::new(store.clone(), root.join("samples"));
    std::fs::create_dir_all(store.join("patches")).unwrap();
    let documents = playback_runtime::split_system_patch_documents(
        &crate::user_data_archive::canonical_defaults(),
    )
    .unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    let mut prior = documents.patch.clone();
    prior["runtimeConfig"]["bpm"] = serde_json::json!(90);
    std::fs::write(
        store.join("patches").join("preset.json"),
        serde_json::to_vec(&prior).unwrap(),
    )
    .unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
                "gate".into(),
                None,
            ),
            PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    let mut payload = documents.patch;
    payload["runtimeConfig"]["bpm"] = serde_json::json!(111);
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::StoreSavePreset {
                    name: "preset".into(),
                    payload: payload.clone(),
                    mode: None,
                },
                "preset-save".into(),
                None,
            ),
            PlatformJobKind::SavePreset {
                name: "preset".into(),
                payload: payload.clone(),
            },
        ))
        .unwrap();
    assert_eq!(
        service.load_preset_now("preset").unwrap_err(),
        "Save pending, try again"
    );
    release_tx.send(()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut completed = false;
    while std::time::Instant::now() < deadline && !completed {
        completed = service.drain_results(4).iter().any(|message| {
            matches!(
                message,
                HostMessage::RuntimeResult {
                    result: RuntimeStoreResult::Identified { request_id, result, .. }
                } if request_id == "preset-save"
                    && matches!(result.as_ref(), RuntimeStoreResult::SavePresetResult { .. })
            )
        });
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(completed);
    assert_eq!(
        load_json(&preset_patch_path(&store, "preset").unwrap()).unwrap(),
        Some(payload)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn system_save_runs_off_thread_and_returns_one_identified_completion() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-system-save-worker-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = root.join("store");
    let service = PiPlatformService::new(store.clone(), root.join("samples"));
    std::fs::create_dir_all(&store).unwrap();
    let documents = playback_runtime::split_system_patch_documents(
        &crate::user_data_archive::canonical_defaults(),
    )
    .unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    std::fs::write(
        store.join("default.patch.json"),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();
    let mut next_system = documents.system.clone();
    next_system["runtimeConfig"]["screenSleepSeconds"] = serde_json::json!(45);
    let prior_bytes = std::fs::read(store.join("system.json")).unwrap();
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
                "gate".into(),
                None,
            ),
            PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::StoreSaveSystem {
                    payload: next_system.clone(),
                },
                "system-save".into(),
                Some(7),
            ),
            PlatformJobKind::SaveSystem {
                payload: next_system.clone(),
            },
        ))
        .unwrap();
    assert_eq!(
        std::fs::read(store.join("system.json")).unwrap(),
        prior_bytes
    );
    release_tx.send(()).unwrap();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut completions = Vec::new();
    while std::time::Instant::now() < deadline && completions.is_empty() {
        completions.extend(service.drain_results(4));
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(matches!(
        completions.as_slice(),
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, revision: Some(7), result }
        }] if request_id == "system-save"
            && matches!(result.as_ref(), RuntimeStoreResult::SaveSystemResult { ok: true })
    ));
    assert_eq!(
        load_json(&store.join("system.json")).unwrap(),
        Some(next_system)
    );
    assert!(service.drain_results(4).is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn queued_system_save_is_cancelled_by_restore_generation() {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-system-save-cancel-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = root.join("store");
    let service = PiPlatformService::new(store.clone(), root.join("samples"));
    std::fs::create_dir_all(&store).unwrap();
    let prior = br#"{"before":true}"#;
    std::fs::write(store.join("system.json"), prior).unwrap();
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
                "gate".into(),
                None,
            ),
            PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
    service
        .enqueue(PlatformJob::new(
            RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::StoreSaveSystem {
                    payload: serde_json::json!({"after": true}),
                },
                "cancelled-system-save".into(),
                Some(9),
            ),
            PlatformJobKind::SaveSystem {
                payload: serde_json::json!({"after": true}),
            },
        ))
        .unwrap();
    service.store_write_barrier.invalidate();
    release_tx.send(()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut completion = None;
    while std::time::Instant::now() < deadline && completion.is_none() {
        completion = service.drain_results(4).into_iter().find(|message| {
            matches!(message, HostMessage::RuntimeResult {
                result: RuntimeStoreResult::Identified { request_id, .. }
            } if request_id == "cancelled-system-save")
        });
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(matches!(
        completion,
        Some(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { request_id, revision: Some(9), result }
        }) if request_id == "cancelled-system-save"
            && matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { error }
                if error.operation == playback_runtime::RuntimeOperation::StoreSaveSystem
                    && error.request_id.as_deref() == Some("cancelled-system-save"))
    ));
    assert_eq!(std::fs::read(store.join("system.json")).unwrap(), prior);
    let _ = std::fs::remove_dir_all(root);
}
