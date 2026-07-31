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
fn system_info_job_returns_identified_typed_result() {
    let job = PlatformJob::new(
        RuntimePlatformRequest::new(
            playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
            "system-info-test".into(),
            Some(7),
        ),
        PlatformJobKind::SystemInfo,
    );
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    let result = {
        let update_executor = device_update::production_executor();
        handle_job(
            Path::new("."),
            Path::new("."),
            job,
            update_executor.as_ref(),
        )
    };
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    let result = handle_job(Path::new("."), Path::new("."), job);
    assert!(matches!(
        result,
        RuntimeStoreResult::Identified { result, request_id, revision }
            if request_id == "system-info-test"
                && revision == Some(7)
                && matches!(*result, RuntimeStoreResult::SystemInfoResult { .. })
    ));
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn usb_storage_start_reports_waiting_until_host_configures_gadget() {
    assert_eq!(
        usb_storage_message("storage-start", "HOST_STATE=not attached\n"),
        "USB SD2 transfer waiting for host"
    );
    assert_eq!(
        usb_storage_message("storage-start", "HOST_STATE=configured\n"),
        "USB SD2 transfer active"
    );
}
