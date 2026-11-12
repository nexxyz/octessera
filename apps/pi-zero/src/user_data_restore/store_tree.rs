//! The new store a restore swaps in: the restored documents and presets, plus what stays local.

use super::{copy_regular_if_present, copy_tree_if_present, io_error, Filesystems};
use std::fs;
use std::path::Path;
use store_files::write_json_atomically;

/// Copies the old patches as they are into the new store's `backups`, which every restore keeps,
/// so the ones the pre-restore archive could not take are not lost with the old store.
pub(super) fn keep_unarchived_patches(
    filesystems: &dyn Filesystems,
    old_store: &Path,
    kept: &Path,
) -> Result<(), String> {
    copy_tree_if_present(
        filesystems,
        old_store,
        &old_store.join("patches"),
        &kept.join("patches"),
        false,
    )?;
    for path in [
        crate::platform_service::current_patch_path(old_store),
        crate::platform_service::recovery_patch_path(old_store),
    ] {
        if let Some(name) = path.file_name() {
            copy_regular_if_present(&path, &kept.join(name))?;
        }
    }
    Ok(())
}

pub(super) fn build_store_tree(
    filesystems: &dyn Filesystems,
    old_store: &Path,
    new_store: &Path,
    bundle: &playback_runtime::UserDataBundle,
    restored: &playback_runtime::user_data_bundle::UserDataArchiveReconstruction,
) -> Result<(), String> {
    fs::create_dir_all(new_store.join("patches")).map_err(io_error)?;
    write_json_atomically(&new_store.join("system.json"), &restored.system)?;
    write_json_atomically(
        &crate::platform_service::default_patch_path(new_store),
        &restored.default_patch,
    )?;
    write_json_atomically(
        &crate::platform_service::current_patch_path(new_store),
        &restored.current_patch,
    )?;
    write_json_atomically(
        &crate::platform_service::recovery_patch_path(new_store),
        &restored.current_patch,
    )?;
    let system = crate::platform_service::load_json(&new_store.join("system.json"))?
        .ok_or_else(|| "staged System settings are missing".to_string())?;
    for patch_path in [
        crate::platform_service::default_patch_path(new_store),
        crate::platform_service::current_patch_path(new_store),
        crate::platform_service::recovery_patch_path(new_store),
    ] {
        let patch = crate::platform_service::load_json(&patch_path)?
            .ok_or_else(|| format!("staged patch is missing: {}", patch_path.display()))?;
        playback_runtime::compose_system_patch_documents(&system, &patch)?;
    }
    for preset in &bundle.presets {
        let path = crate::platform_service::preset_patch_path(new_store, &preset.display_name)?;
        write_json_atomically(&path, &preset.patch)?;
    }
    copy_regular_if_present(
        &old_store.join("device.json"),
        &new_store.join("device.json"),
    )?;
    copy_tree_if_present(
        filesystems,
        old_store,
        &old_store.join("backups"),
        &new_store.join("backups"),
        false,
    )
}
