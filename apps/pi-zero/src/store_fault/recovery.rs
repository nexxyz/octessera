//! Finds which saved settings file keeps the device from starting and puts the board's factory
//! version, or the newest rolling patch backup that still loads, in its place, keeping the broken
//! file beside it.

use playback_runtime::{
    compose_local_system_patch_documents, split_system_patch_documents, SystemPatchDocuments,
};
use serde_json::Value;
use std::path::Path;

pub(crate) const SYSTEM_FILE: &str = "system.json";
pub(crate) const PATCH_FILE: &str = "default.patch.json";

pub(crate) fn factory_documents() -> Result<SystemPatchDocuments, String> {
    let full: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json"))
            .map_err(|error| format!("factory settings are unreadable: {error}"))?;
    split_system_patch_documents(&full)
}

pub(crate) fn broken_files(store_dir: &Path, factory: &SystemPatchDocuments) -> Vec<&'static str> {
    let loads =
        |system: &Value, patch: &Value| compose_local_system_patch_documents(system, patch).is_ok();
    let system = read_document(&store_dir.join(SYSTEM_FILE));
    let patch = read_document(&store_dir.join(PATCH_FILE));
    let with_factory_patch = system
        .as_ref()
        .is_some_and(|system| loads(system, &factory.patch));
    let with_factory_system = patch
        .as_ref()
        .is_some_and(|patch| loads(&factory.system, patch));
    match (&system, &patch) {
        (Some(system), Some(patch)) if loads(system, patch) => Vec::new(),
        _ if with_factory_patch => vec![PATCH_FILE],
        _ if with_factory_system => vec![SYSTEM_FILE],
        _ => vec![SYSTEM_FILE, PATCH_FILE],
    }
}

/// The newest rolling patch backup that loads with the saved System file, offered only when the
/// patch is the one broken file (a patch backup can't help a broken System file).
pub(crate) fn loadable_patch_backup(
    store_dir: &Path,
    factory: &SystemPatchDocuments,
) -> Option<Value> {
    if broken_files(store_dir, factory) != [PATCH_FILE] {
        return None;
    }
    let system = read_document(&store_dir.join(SYSTEM_FILE))?;
    store_files::rotated_backups_newest_first(&store_dir.join("backups"))
        .iter()
        .filter_map(|path| read_document(path))
        .find(|patch| compose_local_system_patch_documents(&system, patch).is_ok())
}

/// Keeps the broken patch as `<name>.broken-<stamp>` and writes `backup` in its place.
pub(crate) fn restore_patch_backup(
    store_dir: &Path,
    backup: &Value,
    stamp: &str,
) -> Result<(), String> {
    keep_broken(store_dir, PATCH_FILE, stamp)?;
    store_files::write_json_atomically(&store_dir.join(PATCH_FILE), backup)
}

/// Moves each broken file aside as `<name>.broken-<stamp>` and writes the factory version.
pub(crate) fn restore_factory_files(
    store_dir: &Path,
    factory: &SystemPatchDocuments,
    stamp: &str,
) -> Result<Vec<&'static str>, String> {
    let broken = broken_files(store_dir, factory);
    for name in &broken {
        let path = store_dir.join(name);
        keep_broken(store_dir, name, stamp)?;
        let document = if *name == SYSTEM_FILE {
            &factory.system
        } else {
            &factory.patch
        };
        store_files::write_json_atomically(&path, document)?;
    }
    Ok(broken)
}

/// Keeps `system.json` as `system.json.pre-reset-<stamp>` and writes the factory System in its place.
pub(crate) fn reset_system_file(
    store_dir: &Path,
    factory: &SystemPatchDocuments,
    stamp: &str,
) -> Result<(), String> {
    store_files::keep_file_aside(store_dir, SYSTEM_FILE, "pre-reset", stamp)?;
    store_files::write_json_atomically(&store_dir.join(SYSTEM_FILE), &factory.system)
}

fn keep_broken(store_dir: &Path, name: &str, stamp: &str) -> Result<(), String> {
    store_files::keep_file_aside(store_dir, name, "broken", stamp)
}

fn read_document(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}
