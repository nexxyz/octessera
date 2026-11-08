use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
pub(crate) use store_files::{read_json_file as load_json, write_json_atomically as save_json};

pub(crate) fn default_patch_path(store_dir: &Path) -> PathBuf {
    store_dir.join("default.patch.json")
}

pub(crate) fn current_patch_path(store_dir: &Path) -> PathBuf {
    store_dir.join("current.json")
}

pub(crate) fn recovery_patch_path(store_dir: &Path) -> PathBuf {
    store_dir.join("recovery-save.json")
}

pub(crate) fn load_current_patch(store_dir: &Path) -> Result<Option<serde_json::Value>, String> {
    load_json(&current_patch_path(store_dir))
}

pub(crate) fn load_recovery_patch(store_dir: &Path) -> Result<Option<serde_json::Value>, String> {
    load_json(&recovery_patch_path(store_dir))
}

pub(crate) fn validate_patch_document(
    store_dir: &Path,
    patch: &serde_json::Value,
) -> Result<(), String> {
    let system = load_json(&store_dir.join("system.json"))?
        .ok_or_else(|| "System settings are missing".to_string())?;
    playback_runtime::compose_local_system_patch_documents(&system, patch).map(|_| ())
}

pub(crate) fn validate_named_preset_document(
    store_dir: &Path,
    patch: &serde_json::Value,
) -> Result<(), String> {
    let system = load_json(&store_dir.join("system.json"))?
        .ok_or_else(|| "System settings are missing".to_string())?;
    playback_runtime::compose_system_patch_documents(&system, patch).map(|_| ())
}

pub(super) fn delete_preset_payload(store_dir: &Path, name: &str) -> bool {
    delete_preset_payload_result(store_dir, name).is_ok()
}

pub(crate) fn delete_preset_payload_result(store_dir: &Path, name: &str) -> Result<(), String> {
    let Ok(patch) = preset_patch_path(store_dir, name) else {
        return Err(format!("Unsafe preset name: {name:?}"));
    };
    if !patch.is_file() {
        return Err(format!("Preset {name} does not exist"));
    }
    std::fs::remove_file(patch)
        .map_err(|error| format!("Preset {name} could not be removed: {error}"))
}

pub(crate) fn list_presets(store_dir: &Path) -> Result<Vec<String>, String> {
    let mut names = BTreeSet::new();
    let patch_dir = store_dir.join("patches");
    if patch_dir.is_dir() {
        for entry in std::fs::read_dir(&patch_dir).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            if !entry.path().is_file() {
                continue;
            }
            if let Some(name) = entry
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_suffix(".json"))
                .filter(|name| playback_runtime::is_valid_preset_name(name))
                .map(str::to_string)
            {
                names.insert(name);
            }
        }
    }
    Ok(names.into_iter().collect())
}

pub(crate) fn preset_patch_path(store_dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !playback_runtime::is_valid_preset_name(name) {
        return Err(format!("Unsafe preset name: {name:?}"));
    }
    Ok(store_dir.join("patches").join(format!("{name}.json")))
}

pub(crate) fn save_backup(store_dir: &Path, payload: &serde_json::Value) -> Result<(), String> {
    store_files::write_rotated_backup(&store_dir.join("backups"), payload)
}
