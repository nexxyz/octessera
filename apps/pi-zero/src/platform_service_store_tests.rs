use super::*;

#[test]
fn list_presets_filters_unsafe_legacy_files() {
    let dir = std::env::temp_dir().join(format!(
        "octessera-pi-preset-list-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("safe.json"), "{}").unwrap();
    std::fs::write(dir.join("default.json"), "{}").unwrap();
    std::fs::write(dir.join("recovery-save.json"), "{}").unwrap();
    std::fs::write(dir.join("bak-123.json"), "{}").unwrap();
    std::fs::write(dir.join("bad:name.json"), "{}").unwrap();
    std::fs::write(dir.join("CON.json"), "{}").unwrap();

    assert_eq!(list_presets(&dir).unwrap(), vec!["safe".to_string()]);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn preset_patch_files_are_preferred_and_delete_removes_legacy_copy() {
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
    std::fs::write(dir.join("Jam.json"), r#"{"legacy":true}"#).unwrap();
    std::fs::create_dir_all(dir.join("patches")).unwrap();
    std::fs::write(dir.join("patches").join("Jam.json"), r#"{"patch":true}"#).unwrap();
    std::fs::write(dir.join("Jam.patch.json"), r#"{"legacy_patch_name":true}"#).unwrap();

    assert_eq!(
        list_presets(&dir).unwrap(),
        vec!["Jam".to_string(), "Jam.patch".to_string()]
    );
    assert_eq!(
        load_json(&preset_load_path(&dir, "Jam").unwrap()).unwrap(),
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
    assert!(!dir.join("Jam.json").exists());
    assert!(!dir.join("patches").join("Jam.json").exists());
    assert!(dir.join("Jam.patch.json").exists());

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
