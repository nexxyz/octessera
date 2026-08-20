use crate::persistence::atomic_write_json;
use crate::user_data_archive::{self, StagedRestore};
use playback_runtime::apply_user_data_patch_and_preferences;
use serde_json::Value;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

#[path = "user_data_restore_transaction.rs"]
mod transaction;
#[cfg(test)]
pub(super) use transaction::{replace_trees_with_faults, FaultInjection};
#[cfg(test)]
pub(super) use transaction::{rollback_tree, swap_tree};

pub(crate) fn restore(
    store_dir: &Path,
    samples_dir: &Path,
    recordings_dir: &Path,
    screen_recordings_dir: &Path,
    session: &str,
    staged: StagedRestore,
) -> Result<(), String> {
    let parent = store_dir
        .parent()
        .ok_or_else(|| "store path has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(io_error)?;
    let backup_path = parent.join(format!("octessera-pre-restore-{session}.oct"));
    if fs::symlink_metadata(&backup_path).is_ok() {
        return Err(format!(
            "pre-restore backup already exists: {}",
            backup_path.display()
        ));
    }
    write_pre_restore_backup(
        store_dir,
        samples_dir,
        recordings_dir,
        screen_recordings_dir,
        &backup_path,
    )?;

    let canonical = user_data_archive::canonical_defaults();
    let current = apply_user_data_patch_and_preferences(
        &canonical,
        &staged.bundle.current_state.patch,
        &staged.bundle.preferences,
    )?;
    let default = apply_user_data_patch_and_preferences(
        &canonical,
        &staged.bundle.default_state.patch,
        &staged.bundle.preferences,
    )?;
    let new_store = parent.join(format!(".octessera-store-new-{session}"));
    let new_samples = parent.join(format!(".octessera-samples-new-{session}"));
    let old_store = parent.join(format!(".octessera-store-old-{session}"));
    let old_samples = parent.join(format!(".octessera-samples-old-{session}"));
    let new_recordings = sibling_path(recordings_dir, "new", session);
    let new_screen_recordings = sibling_path(screen_recordings_dir, "new", session);
    let old_recordings = sibling_path(recordings_dir, "old", session);
    let old_screen_recordings = sibling_path(screen_recordings_dir, "old", session);
    for path in [
        &new_store,
        &new_samples,
        &old_store,
        &old_samples,
        &new_recordings,
        &new_screen_recordings,
        &old_recordings,
        &old_screen_recordings,
    ] {
        if fs::symlink_metadata(path).is_ok() {
            return Err(format!(
                "restore recovery path already exists: {}",
                path.display()
            ));
        }
    }
    let result = (|| {
        build_store_tree(store_dir, &new_store, &staged.bundle, &current, &default)?;
        build_samples_tree(
            samples_dir,
            &new_samples,
            &staged,
            staged.bundle.media_included,
        )?;
        build_media_tree(
            recordings_dir,
            &new_recordings,
            &staged,
            playback_runtime::UserDataMediaKind::Audio,
            staged.bundle.media_included,
        )?;
        build_media_tree(
            screen_recordings_dir,
            &new_screen_recordings,
            &staged,
            playback_runtime::UserDataMediaKind::Screen,
            staged.bundle.media_included,
        )?;
        let trees = [
            (store_dir, new_store.as_path(), old_store.as_path()),
            (samples_dir, new_samples.as_path(), old_samples.as_path()),
            (
                recordings_dir,
                new_recordings.as_path(),
                old_recordings.as_path(),
            ),
            (
                screen_recordings_dir,
                new_screen_recordings.as_path(),
                old_screen_recordings.as_path(),
            ),
        ];
        transaction::replace_trees(&trees)
    })();
    result
}

fn write_pre_restore_backup(
    store_dir: &Path,
    samples_dir: &Path,
    recordings_dir: &Path,
    screen_recordings_dir: &Path,
    backup_path: &Path,
) -> Result<(), String> {
    let plan = user_data_archive::build_export_plan(
        store_dir,
        samples_dir,
        recordings_dir,
        screen_recordings_dir,
        true,
    )?;
    let temp = backup_path.with_file_name(format!(
        ".{}.tmp-{}",
        backup_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("backup"),
        std::process::id()
    ));
    let result = (|| {
        let mut file = File::create(&temp).map_err(io_error)?;
        user_data_archive::write_archive(&plan, &mut file)?;
        file.sync_all().map_err(io_error)?;
        fs::rename(&temp, backup_path).map_err(io_error)?;
        Ok::<_, String>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn build_store_tree(
    old_store: &Path,
    new_store: &Path,
    bundle: &playback_runtime::UserDataBundle,
    current: &Value,
    default: &Value,
) -> Result<(), String> {
    fs::create_dir_all(new_store.join("patches")).map_err(io_error)?;
    atomic_write_json(&new_store.join("default.json"), default)?;
    atomic_write_json(&new_store.join("current.json"), current)?;
    for preset in &bundle.presets {
        let path = crate::platform_service::preset_patch_path(new_store, &preset.display_name)?;
        atomic_write_json(&path, &preset.patch)?;
    }
    for name in ["device.json", "recovery-save.json"] {
        copy_regular_if_present(&old_store.join(name), &new_store.join(name))?;
    }
    copy_tree_if_present(&old_store.join("backups"), &new_store.join("backups"))
}

fn build_samples_tree(
    old_samples: &Path,
    new_samples: &Path,
    staged: &StagedRestore,
    media_included: bool,
) -> Result<(), String> {
    if let Ok(metadata) = fs::symlink_metadata(old_samples) {
        if metadata.file_type().is_symlink() {
            return Err("sample directory is a symlink".into());
        }
    }
    fs::create_dir_all(new_samples).map_err(io_error)?;
    if !media_included {
        copy_tree_if_present(old_samples, new_samples)?;
    } else if old_samples.is_dir() {
        copy_packaged_samples(old_samples, old_samples, new_samples)?;
    }
    copy_staged_media(
        new_samples,
        staged,
        playback_runtime::UserDataMediaKind::Sample,
    )?;
    Ok(())
}

fn build_media_tree(
    old_media: &Path,
    new_media: &Path,
    staged: &StagedRestore,
    kind: playback_runtime::UserDataMediaKind,
    media_included: bool,
) -> Result<(), String> {
    if let Ok(metadata) = fs::symlink_metadata(old_media) {
        if metadata.file_type().is_symlink() {
            return Err("media directory is a symlink".into());
        }
    }
    fs::create_dir_all(new_media).map_err(io_error)?;
    if !media_included {
        copy_tree_if_present(old_media, new_media)?;
    }
    copy_staged_media(new_media, staged, kind)
}

fn copy_staged_media(
    target: &Path,
    staged: &StagedRestore,
    kind: playback_runtime::UserDataMediaKind,
) -> Result<(), String> {
    for media in &staged.media {
        if media.reference.kind == kind {
            copy_file(&media.path, &target.join(&media.reference.id))?;
        }
    }
    Ok(())
}

fn sibling_path(path: &Path, role: &str, session: &str) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            ".octessera-{role}-{}-{session}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("media")
        ))
}

fn copy_packaged_samples(root: &Path, current: &Path, target: &Path) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
        if metadata.file_type().is_symlink() {
            return Err("sample directory contains a symlink".into());
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "sample path escaped its root".to_string())?;
        let relative = relative
            .to_str()
            .ok_or_else(|| "sample file name is not valid UTF-8".to_string())?
            .replace('\\', "/");
        if metadata.is_dir() {
            copy_packaged_samples(root, &path, &target.join(relative))?;
        } else if metadata.is_file() && user_data_archive::is_packaged_sample(&relative) {
            copy_file(&path, &target.join(relative))?;
        }
    }
    Ok(())
}

fn copy_tree_if_present(source: &Path, target: &Path) -> Result<(), String> {
    if !source.exists() {
        return Ok(());
    }
    fs::create_dir_all(target).map_err(io_error)?;
    for entry in fs::read_dir(source).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path).map_err(io_error)?;
        if metadata.file_type().is_symlink() {
            return Err("protected store contains a symlink".into());
        }
        if metadata.is_dir() {
            copy_tree_if_present(&source_path, &target_path)?;
        } else if metadata.is_file() {
            copy_file(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn copy_regular_if_present(source: &Path, target: &Path) -> Result<(), String> {
    if !source.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(source).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("protected store entry is unsafe".into());
    }
    copy_file(source, target)
}

fn copy_file(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    fs::copy(source, target).map_err(io_error)?;
    Ok(())
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
#[path = "user_data_restore_failure_tests.rs"]
mod failure_tests;
