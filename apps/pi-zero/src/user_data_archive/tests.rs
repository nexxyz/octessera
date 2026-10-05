use super::*;
use std::fs;
use std::path::PathBuf;

fn root(name: &str) -> PathBuf {
    let root = crate::test_temp_dir::unique_temp_path(&format!("octessera-user-data-{name}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn write_split_store(store: &Path, full: &Value) {
    let documents = playback_runtime::split_system_patch_documents(full).unwrap();
    fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    fs::write(
        crate::platform_service::default_patch_path(store),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();
    fs::write(
        crate::platform_service::current_patch_path(store),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();
}

#[test]
fn export_preserves_exact_names_and_optional_custom_samples() {
    let root = root("export");
    let store = root.join("store");
    let samples = root.join("samples");
    let recordings = root.join("recordings");
    let screen_recordings = root.join("screen-recordings");
    fs::create_dir_all(store.join("patches")).unwrap();
    fs::create_dir_all(&samples).unwrap();
    fs::create_dir_all(&recordings).unwrap();
    fs::create_dir_all(&screen_recordings).unwrap();
    let defaults = canonical_defaults();
    write_split_store(&store, &defaults);
    let patch = playback_runtime::split_system_patch_documents(&defaults)
        .unwrap()
        .patch;
    fs::write(
        store.join("patches").join("Soft Kit 01.json"),
        serde_json::to_vec(&patch).unwrap(),
    )
    .unwrap();
    fs::write(samples.join("My Kick.wav"), b"custom sample").unwrap();
    fs::write(recordings.join("take.wav"), b"audio recording").unwrap();
    fs::write(screen_recordings.join("screen.webm"), b"screen recording").unwrap();

    let without_media =
        build_export_plan(&store, &samples, &recordings, &screen_recordings, false).unwrap();
    assert_eq!(without_media.bundle.presets[0].display_name, "Soft Kit 01");
    assert!(!without_media.bundle.media_included);
    assert!(without_media.media.is_empty());

    let with_media =
        build_export_plan(&store, &samples, &recordings, &screen_recordings, true).unwrap();
    let sample = with_media
        .bundle
        .media
        .iter()
        .find(|media| media.kind == UserDataMediaKind::Sample)
        .unwrap();
    assert_eq!(sample.display_name, "My Kick.wav");
    assert_eq!(sample.size, b"custom sample".len() as u64);
    assert_eq!(with_media.media.len(), 3);
    assert!(with_media
        .bundle
        .media
        .iter()
        .any(|media| media.kind == UserDataMediaKind::Audio && media.id == "take.wav"));
    assert!(with_media
        .bundle
        .media
        .iter()
        .any(|media| media.kind == UserDataMediaKind::Screen && media.id == "screen.webm"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn export_composes_split_documents_before_portable_patch_and_preferences() {
    let root = root("split-export");
    let store = root.join("store");
    fs::create_dir_all(&store).unwrap();
    let mut full = canonical_defaults();
    full["runtimeConfig"]["displayBrightness"] = serde_json::json!(47);
    full["runtimeConfig"]["midi"]["inId"] = serde_json::json!("local-midi-in");
    full["runtimeConfig"]["midi"]["outId"] = serde_json::json!("local-midi-out");
    full["runtimeConfig"]["usb"]["dataRole"] = serde_json::json!("host");
    full["runtimeConfig"]["audioOutputs"]["usb"] = serde_json::json!(false);
    full["runtimeConfig"]["instruments"][0]["sample"]["assignments"] =
        serde_json::json!([{ "level": null, "sampleSlot": 5, "x": 6, "y": 1 }]);
    let mut current = full.clone();
    current["runtimeConfig"]["instruments"][0]["sample"]["assignments"] =
        serde_json::json!([{ "level": null, "sampleSlot": 3, "x": 2, "y": 4 }]);
    let default_documents = playback_runtime::split_system_patch_documents(&full).unwrap();
    let current_documents = playback_runtime::split_system_patch_documents(&current).unwrap();
    let system = default_documents.system;
    let default_patch = default_documents.patch;
    let current_patch = current_documents.patch;
    fs::write(
        store.join("system.json"),
        serde_json::to_vec(&system).unwrap(),
    )
    .unwrap();
    fs::write(
        crate::platform_service::default_patch_path(&store),
        serde_json::to_vec(&default_patch).unwrap(),
    )
    .unwrap();
    fs::write(
        crate::platform_service::current_patch_path(&store),
        serde_json::to_vec(&current_patch).unwrap(),
    )
    .unwrap();

    let plan = build_export_plan(
        &store,
        &root.join("samples"),
        &root.join("recordings"),
        &root.join("screen-recordings"),
        false,
    )
    .unwrap();
    let archived = &plan.bundle.current_state.patch;
    assert_eq!(archived["kind"], "octessera.patch");
    assert_eq!(
        archived["runtimeConfig"]["instruments"][0]["sample"]["assignments"][0]["sampleSlot"],
        3
    );
    assert!(archived["runtimeConfig"]["midi"].get("inId").is_none());
    assert!(archived["runtimeConfig"]["midi"].get("outId").is_none());
    assert_eq!(plan.bundle.preferences.values["displayBrightness"], 47);
    assert!(!plan.bundle.preferences.values.contains_key("midi"));
    assert!(!plan.bundle.preferences.values.contains_key("usb"));
    assert!(!plan.bundle.preferences.values.contains_key("audioOutputs"));
    assert_eq!(
        plan.bundle.default_state.patch["runtimeConfig"]["instruments"][0]["sample"]["assignments"]
            [0]["sampleSlot"],
        5
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn export_refuses_partial_malformed_and_legacy_store_documents() {
    let root = root("invalid-split-export");
    let store = root.join("store");
    fs::create_dir_all(&store).unwrap();
    let samples = root.join("samples");
    let recordings = root.join("recordings");
    let screen = root.join("screen");
    assert!(build_export_plan(&store, &samples, &recordings, &screen, false).is_err());

    let documents = playback_runtime::split_system_patch_documents(&canonical_defaults()).unwrap();
    fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    fs::write(crate::platform_service::default_patch_path(&store), b"{").unwrap();
    assert!(build_export_plan(&store, &samples, &recordings, &screen, false).is_err());

    fs::write(store.join("default.json"), b"legacy").unwrap();
    fs::write(
        crate::platform_service::default_patch_path(&store),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();
    assert!(build_export_plan(&store, &samples, &recordings, &screen, false).is_err());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn archive_round_trip_stages_media_and_rejects_mutation_on_invalid_archive() {
    let root = root("stage");
    let store = root.join("store");
    let samples = root.join("samples");
    let recordings = root.join("recordings");
    let screen_recordings = root.join("screen-recordings");
    fs::create_dir_all(store.join("patches")).unwrap();
    fs::create_dir_all(&samples).unwrap();
    write_split_store(&store, &canonical_defaults());
    fs::write(samples.join("User.wav"), b"sample bytes").unwrap();
    let plan = build_export_plan(&store, &samples, &recordings, &screen_recordings, true).unwrap();
    let archive_path = root.join("bundle.oct");
    let mut archive = File::create(&archive_path).unwrap();
    write_archive(&plan, &mut archive).unwrap();
    archive.sync_all().unwrap();
    let staged = stage_archive(&archive_path, &root.join("stage")).unwrap();
    assert_eq!(staged.bundle.media[0].id, "User.wav");
    assert_eq!(fs::read(&staged.media[0].path).unwrap(), b"sample bytes");

    fs::write(&archive_path, b"not an archive").unwrap();
    let original = fs::read(staged.media[0].path.clone()).unwrap();
    assert!(stage_archive(&archive_path, &root.join("bad-stage")).is_err());
    assert_eq!(fs::read(staged.media[0].path.clone()).unwrap(), original);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn audio_media_stages_separately_from_samples() {
    let root = root("future-media");
    let empty_sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let reference = UserDataMediaReference {
        id: "take.wav".into(),
        kind: UserDataMediaKind::Audio,
        display_name: "take.wav".into(),
        size: 0,
        sha256: empty_sha256.into(),
    };
    let bundle = new_user_data_bundle(
        UserDataBundleMetadata {
            board_profile: "raspberry-pi-zero-2w".into(),
            runtime_version: "test".into(),
        },
        Vec::new(),
        UserDataMusicalState {
            patch: canonical_defaults(),
        },
        UserDataMusicalState {
            patch: canonical_defaults(),
        },
        playback_runtime::UserPreferenceDelta::empty(),
        true,
        vec![reference],
        &canonical_defaults(),
    )
    .unwrap();
    let media_path = root.join("take.wav");
    fs::write(&media_path, b"").unwrap();
    let archive_path = root.join("future.oct");
    let mut archive = File::create(&archive_path).unwrap();
    write_archive(
        &ExportPlan {
            bundle,
            media: vec![MediaFile {
                reference: UserDataMediaReference {
                    id: "take.wav".into(),
                    kind: UserDataMediaKind::Audio,
                    display_name: "take.wav".into(),
                    size: 0,
                    sha256: empty_sha256.into(),
                },
                path: media_path,
            }],
        },
        &mut archive,
    )
    .unwrap();
    archive.sync_all().unwrap();
    let staged = stage_archive(&archive_path, &root.join("stage")).unwrap();
    assert_eq!(staged.media[0].reference.kind, UserDataMediaKind::Audio);
    assert_eq!(
        staged.media[0].path.parent().unwrap().file_name().unwrap(),
        "audio"
    );
    let _ = fs::remove_dir_all(root);
}
