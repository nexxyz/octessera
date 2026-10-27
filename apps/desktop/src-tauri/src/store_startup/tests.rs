use super::*;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).expect("create temp directory");
    dir
}

fn remove_temp_dir(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

#[test]
fn store_root_file_is_reported_at_startup() {
    let parent = unique_temp_dir("octessera-store-root-file");
    let root = parent.join("store");
    fs::write(&root, b"not a directory").expect("store root file");

    let error = ensure_store_dir_at(root).expect_err("file root must fail");

    assert!(error.to_string().starts_with(
        "desktop startup store initialization failed: unable to create store directory"
    ));
    remove_temp_dir(&parent);
}

#[test]
fn preset_directory_creation_failure_is_reported() {
    let root = unique_temp_dir("octessera-preset-directory-file");
    fs::write(root.join("presets"), b"not a directory").expect("preset directory file");

    let error = ensure_store_dir_at(root.clone()).expect_err("preset file must fail");

    assert!(error.to_string().starts_with(
        "desktop startup store initialization failed: unable to create preset directory"
    ));
    remove_temp_dir(&root);
}

#[test]
fn unknown_store_file_is_refused_and_preserved() {
    let root = unique_temp_dir("octessera-malformed-default");
    let original = b"{ malformed";
    fs::write(root.join("default.json"), original).expect("malformed default");

    let error = ensure_store_dir_at(root.clone()).expect_err("unknown files block seeding");
    assert!(error.to_string().contains("start again"));

    assert_eq!(
        fs::read(root.join("default.json")).expect("existing default"),
        original
    );
    remove_temp_dir(&root);
}

#[test]
fn valid_first_seed_creates_native_split_pair_and_presets() {
    let root = unique_temp_dir("octessera-valid-first-seed");

    ensure_store_dir_at(root.clone()).expect("first seed");

    let expected = playback_runtime::split_system_patch_documents(
        &serde_json::from_str(BUNDLED_DEFAULT_CONFIG).expect("bundled default"),
    )
    .expect("native projection");
    let system = read_document(&root.join("system.json")).expect("seeded system");
    let patch = read_document(&root.join("default.patch.json")).expect("seeded patch");
    assert_eq!(system, expected.system);
    assert_eq!(patch, expected.patch);
    assert!(playback_runtime::compose_system_patch_documents(&system, &patch).is_ok());
    assert!(!root.join("default.json").exists());
    assert!(root.join("presets").is_dir());
    remove_temp_dir(&root);
}

#[test]
fn complete_valid_pair_is_accepted_without_rewriting_documents() {
    let root = unique_temp_dir("octessera-complete-pair");
    let bundled: serde_json::Value =
        serde_json::from_str(BUNDLED_DEFAULT_CONFIG).expect("bundled default");
    let documents =
        playback_runtime::split_system_patch_documents(&bundled).expect("native projection");
    persistence::atomic_write_json(&root.join("system.json"), &documents.system).unwrap();
    persistence::atomic_write_json(&root.join("default.patch.json"), &documents.patch).unwrap();
    let system_bytes = fs::read(root.join("system.json")).unwrap();
    let patch_bytes = fs::read(root.join("default.patch.json")).unwrap();

    ensure_store_dir_at(root.clone()).expect("valid complete pair");

    assert_eq!(fs::read(root.join("system.json")).unwrap(), system_bytes);
    assert_eq!(
        fs::read(root.join("default.patch.json")).unwrap(),
        patch_bytes
    );
    remove_temp_dir(&root);
}

#[test]
fn partial_or_corrupt_pair_is_refused_without_changing_bytes() {
    let root = unique_temp_dir("octessera-partial-pair");
    let original = b"not json";
    fs::write(root.join("system.json"), original).expect("system bytes");

    let error = ensure_store_dir_at(root.clone()).expect_err("partial pair must fail");

    assert!(error.to_string().contains("incomplete"));
    assert_eq!(fs::read(root.join("system.json")).unwrap(), original);
    assert!(!root.join("default.patch.json").exists());
    remove_temp_dir(&root);
}

#[test]
fn complete_invalid_pair_is_refused_without_changing_bytes() {
    let root = unique_temp_dir("octessera-invalid-pair");
    let system_bytes = b"{}";
    let patch_bytes = b"{}";
    fs::write(root.join("system.json"), system_bytes).expect("system bytes");
    fs::write(root.join("default.patch.json"), patch_bytes).expect("patch bytes");

    let error = ensure_store_dir_at(root.clone()).expect_err("corrupt pair must fail");

    assert!(error.to_string().contains("split documents are invalid"));
    assert_eq!(fs::read(root.join("system.json")).unwrap(), system_bytes);
    assert_eq!(
        fs::read(root.join("default.patch.json")).unwrap(),
        patch_bytes
    );
    remove_temp_dir(&root);
}

#[test]
fn unknown_store_document_is_refused_without_overwriting() {
    let root = unique_temp_dir("octessera-existing-custom-default");
    let custom = serde_json::json!({ "kept": true });
    fs::write(
        root.join("default.json"),
        serde_json::to_vec(&custom).expect("serialize custom default"),
    )
    .expect("write custom default");

    let error = ensure_store_dir_at(root.clone()).expect_err("unknown files block seeding");
    assert!(error.to_string().contains("start again"));
    assert_eq!(
        fs::read(root.join("default.json")).unwrap(),
        serde_json::to_vec(&custom).unwrap()
    );
    assert!(!root.join("system.json").exists());
    assert!(!root.join("default.patch.json").exists());
    remove_temp_dir(&root);
}

#[test]
fn explicit_env_store_root_is_used_without_app_data_resolution() {
    let root = unique_temp_dir("octessera-explicit-store-root");
    let selected = resolve_store_root(Some(root.clone()), || {
        Err("app-data resolution should not be called".to_string())
    })
    .expect("explicit store root");

    ensure_store_dir_at(selected.clone()).expect("explicit store root initialization");
    assert!(selected.join("system.json").is_file());
    assert!(selected.join("default.patch.json").is_file());
    remove_temp_dir(&root);
}

#[test]
fn app_data_resolution_failure_is_not_replaced_with_executable_relative_path() {
    let error = resolve_store_root(None, || Err("app-data unavailable".to_string()))
        .expect_err("app-data failure must stop startup");

    assert_eq!(
        error.to_string(),
        "desktop startup store initialization failed: unable to resolve app-data directory: app-data unavailable"
    );
}
