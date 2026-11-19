//! Finds which saved settings file keeps the device from starting and puts the board's factory
//! version in its place, keeping the broken file beside it.

use playback_runtime::{
    compose_local_system_patch_documents, split_system_patch_documents, SystemPatchDocuments,
};
use serde_json::Value;
use std::path::{Path, PathBuf};

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

/// Moves each broken file aside as `<name>.broken-<stamp>` and writes the factory version.
pub(crate) fn restore_factory_files(
    store_dir: &Path,
    factory: &SystemPatchDocuments,
    stamp: &str,
) -> Result<Vec<&'static str>, String> {
    let broken = broken_files(store_dir, factory);
    for name in &broken {
        let path = store_dir.join(name);
        if path.exists() {
            let kept = free_path(store_dir, &format!("{name}.broken-{stamp}"));
            std::fs::rename(&path, &kept)
                .map_err(|error| format!("could not keep {name}: {error}"))?;
        }
        let document = if *name == SYSTEM_FILE {
            &factory.system
        } else {
            &factory.patch
        };
        store_files::write_json_atomically(&path, document)?;
    }
    Ok(broken)
}

fn read_document(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn free_path(dir: &Path, name: &str) -> PathBuf {
    let mut path = dir.join(name);
    let mut suffix = 1;
    while path.exists() {
        path = dir.join(format!("{name}-{suffix}"));
        suffix += 1;
    }
    path
}
