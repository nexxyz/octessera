use super::{platform_request, temp_store_dir, test_adapter};
use crate::persistence::{atomic_write_json, valid_preset_name};
use playback_runtime::{HostAdapter, HostMessage, RuntimePlatformEffect, RuntimeStoreResult};
use std::time::Instant;

#[test]
fn preset_name_validation_rejects_unsafe_names() {
    assert!(valid_preset_name("default"));
    for name in [
        "../default",
        "presets/evil",
        r"C:\x",
        "",
        "   ",
        "bad/name",
        "bad\\name",
        "bad:name",
        "bad<name>",
        "bad>name",
        "bad\"name",
        "bad|name",
        "bad?name",
        "bad*name",
        "bad\nname",
        "bad.",
        "CON",
        "NUL.json",
        "COM1",
        "LPT9.txt",
    ] {
        assert!(!valid_preset_name(name), "{name:?}");
    }
}

#[test]
fn preset_host_paths_reject_unsafe_names_and_filter_list() {
    let (mut adapter, _) = test_adapter();
    adapter.store_dir = temp_store_dir("preset-safety");
    let presets = adapter.store_dir.join("presets");
    std::fs::create_dir_all(&presets).unwrap();
    std::fs::write(presets.join("safe.json"), "{}").unwrap();
    std::fs::write(presets.join(" bad .json"), "{}").unwrap();

    assert_eq!(
        adapter.list_preset_names().unwrap(),
        vec!["safe".to_string()]
    );
    for name in [
        "../default",
        "presets/evil",
        r"C:\x",
        "",
        "   ",
        "bad/name",
        "bad\\name",
        "bad:name",
        "bad<name>",
        "bad?name",
        "CON",
        "NUL.json",
        "bad\nname",
    ] {
        assert!(adapter.load_preset_payload(name).is_err(), "load {name:?}");
        assert!(
            adapter
                .save_preset_payload(name, &serde_json::json!({}))
                .is_err(),
            "save {name:?}"
        );
        assert!(
            adapter.delete_preset_payload(name).is_err(),
            "delete {name:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&adapter.store_dir);
}

#[test]
fn preset_patch_files_are_preferred_and_delete_removes_legacy_copy() {
    let (mut adapter, _) = test_adapter();
    adapter.store_dir = temp_store_dir("preset-patch-precedence");
    let presets = adapter.store_dir.join("presets");
    std::fs::create_dir_all(&presets).unwrap();
    std::fs::write(presets.join("Jam.json"), r#"{"legacy":true}"#).unwrap();
    std::fs::create_dir_all(presets.join("patches")).unwrap();
    std::fs::write(
        presets.join("patches").join("Jam.json"),
        r#"{"patch":true}"#,
    )
    .unwrap();
    std::fs::write(
        presets.join("Jam.patch.json"),
        r#"{"legacy_patch_name":true}"#,
    )
    .unwrap();

    assert_eq!(
        adapter.list_preset_names().unwrap(),
        vec!["Jam".to_string(), "Jam.patch".to_string()]
    );
    assert_eq!(
        adapter.load_preset_payload("Jam").unwrap(),
        Some(serde_json::json!({ "patch": true }))
    );

    adapter
        .save_preset_payload("New", &serde_json::json!({ "kind": "octessera.patch" }))
        .unwrap();
    assert!(presets.join("patches").join("New.json").is_file());
    assert!(!presets.join("New.json").is_file());

    assert!(adapter.delete_preset_payload("Jam").unwrap());
    assert!(!presets.join("Jam.json").exists());
    assert!(!presets.join("patches").join("Jam.json").exists());
    assert!(presets.join("Jam.patch.json").exists());
    let _ = std::fs::remove_dir_all(&adapter.store_dir);
}

#[test]
fn atomic_json_write_overwrites_existing_file() {
    let dir = temp_store_dir("atomic-overwrite");
    let path = dir.join("default.json");
    std::fs::write(&path, "{\"old\":true}").unwrap();

    atomic_write_json(&path, &serde_json::json!({ "new": true })).unwrap();

    let saved = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
        serde_json::json!({ "new": true })
    );
    assert!(std::fs::read_dir(&dir).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains(".tmp-")));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn backup_save_rotates_to_latest_twenty_files() {
    let (mut adapter, _) = test_adapter();
    adapter.store_dir = temp_store_dir("backup-rotation");
    let backups = adapter.store_dir.join("backups");
    std::fs::create_dir_all(&backups).unwrap();
    for index in 0..20 {
        std::fs::write(backups.join(format!("bak-{index:03}.json")), "{}").unwrap();
    }

    adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::StoreSaveBackup {
            payload: serde_json::json!({ "latest": true }),
        }))
        .unwrap();

    let mut names = std::fs::read_dir(&backups)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names.len(), 20);
    assert!(!names.contains(&"bak-000.json".to_string()));
    assert!(names.iter().any(|name| name.starts_with("bak-1")));
    let _ = std::fs::remove_dir_all(&adapter.store_dir);
}

#[test]
fn deferred_default_save_flushes_runtime_result() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = std::env::temp_dir().join(format!(
        "octessera-host-adapter-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    adapter.store_dir = temp_dir.clone();
    let payload = serde_json::json!({ "runtimeConfig": { "masterVolume": 73 } });
    let follow_ups = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::StoreSaveDefault {
            payload: payload.clone(),
            mode: Some("deferred".into()),
        }))
        .unwrap();
    assert!(follow_ups.is_empty());
    adapter.pending_default_save = adapter
        .pending_default_save
        .take()
        .map(|(payload, _, request)| (payload, Instant::now(), request));
    let follow_ups = adapter.flush_due_default_save().unwrap();
    assert!(matches!(
        &follow_ups[..],
        [HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified { .. }
        }]
    ));
    assert!(temp_dir.join("default.json").is_file());
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn pending_default_save_flushes_immediately_on_shutdown() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = std::env::temp_dir().join(format!(
        "octessera-host-adapter-shutdown-default-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    adapter.store_dir = temp_dir.clone();
    let payload = serde_json::json!({ "runtimeConfig": { "layers": [{ "name": "life" }] } });
    adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::StoreSaveDefault {
            payload: payload.clone(),
            mode: Some("deferred".into()),
        }))
        .unwrap();

    adapter.flush_pending_default_save_now().unwrap();

    let saved = std::fs::read_to_string(temp_dir.join("default.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
        payload
    );
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn failed_deferred_default_save_retains_pending_payload_for_retry() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = temp_store_dir("retry-default-save");
    let blocker = temp_dir.join("not-a-directory");
    std::fs::write(&blocker, "blocker").unwrap();
    adapter.store_dir = blocker;
    adapter.pending_default_save = Some((
        serde_json::json!({ "masterVolume": 72 }),
        Instant::now(),
        platform_request(RuntimePlatformEffect::StoreSaveDefault {
            payload: serde_json::json!({ "masterVolume": 72 }),
            mode: Some("deferred".into()),
        }),
    ));

    assert!(adapter.flush_due_default_save().is_err());
    assert!(adapter.pending_default_save.is_some());
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn malformed_default_load_returns_store_error() {
    let (mut adapter, _) = test_adapter();
    let temp_dir = std::env::temp_dir().join(format!(
        "octessera-host-adapter-bad-default-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::fs::write(temp_dir.join("default.json"), "not json").unwrap();
    adapter.store_dir = temp_dir.clone();
    let follow_ups = adapter
        .handle_platform_effect(&platform_request(RuntimePlatformEffect::StoreLoadDefault))
        .unwrap();
    assert!(
        matches!(&follow_ups[..], [HostMessage::RuntimeResult { result: RuntimeStoreResult::StoreError { message } }] if message.starts_with("Default load failed:"))
    );
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn recovery_save_effect_writes_recovery_save_file() {
    let (mut adapter, _) = test_adapter();
    adapter.store_dir = temp_store_dir("recovery-save");
    let payload = serde_json::json!({ "runtimeConfig": { "masterVolume": 64 } });

    let follow_ups = adapter
        .handle_platform_effect(&platform_request(
            RuntimePlatformEffect::StoreSaveRecovery {
                payload: payload.clone(),
            },
        ))
        .unwrap();

    assert!(follow_ups.is_empty());
    let saved = std::fs::read_to_string(adapter.store_dir.join("recovery-save.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
        payload
    );
    let _ = std::fs::remove_dir_all(&adapter.store_dir);
}
