use crate::persistence::atomic_write_json;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(crate) fn load_json(path: &Path) -> Result<Option<serde_json::Value>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|error| error.to_string())
}

pub(crate) fn save_json(path: &Path, payload: &serde_json::Value) -> Result<(), String> {
    atomic_write_json(path, payload)
}

pub(super) fn delete_preset_payload(store_dir: &Path, name: &str) -> bool {
    let Ok(legacy) = preset_path(store_dir, name) else {
        return false;
    };
    let Ok(patch) = preset_patch_path(store_dir, name) else {
        return false;
    };
    let mut removed = false;
    for path in [legacy, patch] {
        if path.is_file() && std::fs::remove_file(path).is_ok() {
            removed = true;
        }
    }
    removed
}

pub(crate) fn list_presets(store_dir: &Path) -> Result<Vec<String>, String> {
    let mut names = BTreeSet::new();
    if !store_dir.is_dir() {
        return Ok(Vec::new());
    }
    for entry in std::fs::read_dir(store_dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.path().is_file() {
            continue;
        }
        if let Some(name) = entry
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(preset_name_from_file_name)
        {
            names.insert(name);
        }
    }
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
                .and_then(preset_name_from_file_name)
            {
                names.insert(name);
            }
        }
    }
    Ok(names.into_iter().collect())
}

pub(crate) fn preset_path(store_dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !playback_runtime::is_valid_preset_name(name) {
        return Err(format!("Unsafe preset name: {name:?}"));
    }
    Ok(store_dir.join(format!("{name}.json")))
}

pub(crate) fn preset_patch_path(store_dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !playback_runtime::is_valid_preset_name(name) {
        return Err(format!("Unsafe preset name: {name:?}"));
    }
    Ok(store_dir.join("patches").join(format!("{name}.json")))
}

pub(crate) fn preset_load_path(store_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let patch = preset_patch_path(store_dir, name)?;
    if patch.is_file() {
        return Ok(patch);
    }
    preset_path(store_dir, name)
}

pub(crate) fn save_backup(store_dir: &Path, payload: &serde_json::Value) -> Result<(), String> {
    let dir = store_dir.join("backups");
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    save_json(&dir.join(format!("bak-{millis}.json")), payload)?;
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with("bak-") && name.ends_with(".json") {
            paths.push(path);
        }
    }
    paths.sort();
    for path in paths.iter().take(paths.len().saturating_sub(20)) {
        std::fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn preset_name_from_file_name(file_name: &str) -> Option<String> {
    if matches!(
        file_name,
        "default.json"
            | "default.patch.json"
            | "current.json"
            | "device.json"
            | "recovery-save.json"
    ) || file_name.starts_with("bak-")
    {
        return None;
    }
    let name = file_name.strip_suffix(".json")?;
    playback_runtime::is_valid_preset_name(name).then(|| name.to_string())
}
