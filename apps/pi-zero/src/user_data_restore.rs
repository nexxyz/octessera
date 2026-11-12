use crate::user_data_archive::{self, StagedRestore};
use serde_json::Value;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

mod filesystem;
mod store_tree;
mod transaction;
use filesystem::remove_tree_within;
pub(crate) use filesystem::{Filesystems, LiveFilesystems};
use store_tree::{build_store_tree, keep_unarchived_patches};
#[cfg(test)]
pub(super) use transaction::{replace_trees_with_faults, FaultInjection};

/// The runtime-owned folder next to the store, on the live folders' filesystem, that holds the
/// pre-restore archive and the new and old trees while they swap.
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) const STAGING_DIR_NAME: &str = "restore";
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) const STAGING_DIR_NAME: &str = ".octessera-restore";

pub(crate) fn restore(
    filesystems: &dyn Filesystems,
    store_dir: &Path,
    samples_dir: &Path,
    recordings_dir: &Path,
    screen_recordings_dir: &Path,
    session: &str,
    staged: StagedRestore,
) -> Result<(), String> {
    let canonical = user_data_archive::canonical_defaults();
    let (target_system, target_patch) = read_target_pair(store_dir)?;
    let restored = playback_runtime::user_data_bundle::reconstruct_user_data_archive(
        &canonical,
        &target_system,
        &target_patch,
        &staged.bundle.current_state.patch,
        &staged.bundle.default_state.patch,
        &staged.bundle.preferences,
    )?;
    let staging = store_dir
        .parent()
        .ok_or_else(|| "store path has no parent".to_string())?
        .join(STAGING_DIR_NAME);
    fs::create_dir_all(&staging).map_err(io_error)?;
    let backup_path = staging.join(format!("octessera-pre-restore-{session}.oct"));
    if fs::symlink_metadata(&backup_path).is_ok() {
        return Err(format!(
            "pre-restore backup already exists: {}",
            backup_path.display()
        ));
    }
    let backup_skipped_patches = write_pre_restore_backup(
        store_dir,
        samples_dir,
        recordings_dir,
        screen_recordings_dir,
        &backup_path,
    )?;

    let new_store = staging.join(format!(".octessera-store-new-{session}"));
    let new_samples = staging.join(format!(".octessera-samples-new-{session}"));
    let old_store = staging.join(format!(".octessera-store-old-{session}"));
    let old_samples = staging.join(format!(".octessera-samples-old-{session}"));
    let new_recordings = staging_path(&staging, recordings_dir, "new", session);
    let new_screen_recordings = staging_path(&staging, screen_recordings_dir, "new", session);
    let old_recordings = staging_path(&staging, recordings_dir, "old", session);
    let old_screen_recordings = staging_path(&staging, screen_recordings_dir, "old", session);
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
        let _sd_card = hold_sd_card_lock(filesystems.sd_card_lock_path())?;
        build_store_tree(
            filesystems,
            store_dir,
            &new_store,
            &staged.bundle,
            &restored,
        )?;
        if backup_skipped_patches {
            keep_unarchived_patches(
                filesystems,
                store_dir,
                &new_store
                    .join("backups")
                    .join(format!("pre-restore-{session}")),
            )?;
        }
        build_samples_tree(
            filesystems,
            samples_dir,
            &new_samples,
            &staged,
            staged.bundle.media_included,
        )?;
        build_media_tree(
            filesystems,
            recordings_dir,
            &new_recordings,
            &staged,
            playback_runtime::UserDataMediaKind::Audio,
            staged.bundle.media_included,
        )?;
        build_media_tree(
            filesystems,
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
        transaction::replace_trees(filesystems, &trees, samples_dir)
    })();
    let rolled_back_cleanly = [
        &old_store,
        &old_samples,
        &old_recordings,
        &old_screen_recordings,
    ]
    .iter()
    .all(|path| fs::symlink_metadata(path).is_err());
    if result.is_ok() {
        prune_pre_restore_archives(&staging, &backup_path);
    } else if rolled_back_cleanly {
        for path in [
            &new_store,
            &new_samples,
            &new_recordings,
            &new_screen_recordings,
        ] {
            if fs::symlink_metadata(path).is_ok() {
                if let Err(error) = remove_tree_within(filesystems, path, path) {
                    eprintln!(
                        "restore failed and could not remove its staging folder {}: {error}",
                        path.display()
                    );
                }
            }
        }
        let _ = fs::remove_file(&backup_path);
    }
    result
}

fn read_target_pair(store_dir: &Path) -> Result<(Value, Value), String> {
    if store_dir.join("default.json").exists() {
        return Err("legacy mixed store cannot be restored over".into());
    }
    let system = crate::platform_service::load_json(&store_dir.join("system.json"))?
        .ok_or_else(|| "System settings are missing".to_string())?;
    let patch = crate::platform_service::load_json(&crate::platform_service::default_patch_path(
        store_dir,
    ))?
    .ok_or_else(|| "Default patch is missing".to_string())?;
    crate::platform_service::validate_patch_document(store_dir, &patch)?;
    Ok((system, patch))
}

/// Keeps a card insert or transfer from mounting into the samples folder while restore reads and
/// swaps it. Both boards create the lock file at boot, so restore never runs without it.
fn hold_sd_card_lock(path: &Path) -> Result<File, String> {
    let file = File::open(path).map_err(|error| {
        format!(
            "restore could not open the SD card lock {}: {error}",
            path.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
            return Err(format!(
                "restore could not take the SD card lock: {}",
                io::Error::last_os_error()
            ));
        }
    }
    Ok(file)
}

/// Each restore leaves its pre-restore archive behind; the one just written and the newest other
/// one stay, so a clock running behind older archives cannot prune the latest.
const KEPT_OLDER_PRE_RESTORE_ARCHIVES: usize = 1;

fn prune_pre_restore_archives(staging: &Path, kept: &Path) {
    let Ok(entries) = fs::read_dir(staging) else {
        return;
    };
    let mut archives: Vec<_> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_name().to_str().is_some_and(|name| {
                name.starts_with("octessera-pre-restore-") && name.ends_with(".oct")
            })
        })
        .filter_map(|entry| {
            let metadata = entry.metadata().ok().filter(fs::Metadata::is_file)?;
            Some((metadata.modified().ok()?, entry.path()))
        })
        .filter(|(_, path)| path != kept)
        .collect();
    archives.sort_by(|left, right| right.cmp(left));
    for (_, path) in archives.into_iter().skip(KEPT_OLDER_PRE_RESTORE_ARCHIVES) {
        let _ = fs::remove_file(path);
    }
}

/// `true` when the archive had to leave out a damaged patch or preset.
fn write_pre_restore_backup(
    store_dir: &Path,
    samples_dir: &Path,
    recordings_dir: &Path,
    screen_recordings_dir: &Path,
    backup_path: &Path,
) -> Result<bool, String> {
    let plan = user_data_archive::build_export_plan(
        store_dir,
        samples_dir,
        recordings_dir,
        screen_recordings_dir,
        true,
    )?;
    for warning in &plan.warnings {
        eprintln!("Pre-restore backup: {warning}");
    }
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
    result.map(|()| !plan.warnings.is_empty())
}

fn build_samples_tree(
    filesystems: &dyn Filesystems,
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
        copy_tree_if_present(filesystems, old_samples, old_samples, new_samples, true)?;
    } else if old_samples.is_dir() {
        copy_packaged_samples(filesystems, old_samples, old_samples, new_samples)?;
    }
    copy_staged_media(
        new_samples,
        staged,
        playback_runtime::UserDataMediaKind::Sample,
    )?;
    Ok(())
}

fn build_media_tree(
    filesystems: &dyn Filesystems,
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
        copy_tree_if_present(filesystems, old_media, old_media, new_media, false)?;
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

fn staging_path(staging: &Path, path: &Path, role: &str, session: &str) -> PathBuf {
    staging.join(format!(
        ".octessera-{role}-{}-{session}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("media")
    ))
}

fn copy_packaged_samples(
    filesystems: &dyn Filesystems,
    root: &Path,
    current: &Path,
    target: &Path,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let path = entry.path();
        if filesystems.stays_in_place(root, &path, true)? {
            continue;
        }
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
            copy_packaged_samples(filesystems, root, &path, target)?;
        } else if metadata.is_file() && crate::sample_browser::is_packaged_library_file(&relative) {
            copy_file(&path, &target.join(relative))?;
        }
    }
    Ok(())
}

/// Copies `source` into `target`, leaving out what stays in place below `root` (the SD card).
fn copy_tree_if_present(
    filesystems: &dyn Filesystems,
    root: &Path,
    source: &Path,
    target: &Path,
    holds_library: bool,
) -> Result<(), String> {
    if !source.exists() {
        return Ok(());
    }
    fs::create_dir_all(target).map_err(io_error)?;
    for entry in fs::read_dir(source).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let source_path = entry.path();
        if filesystems.stays_in_place(root, &source_path, holds_library)? {
            continue;
        }
        let target_path = target.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path).map_err(io_error)?;
        if metadata.file_type().is_symlink() {
            return Err("protected store contains a symlink".into());
        }
        if metadata.is_dir() {
            copy_tree_if_present(filesystems, root, &source_path, &target_path, holds_library)?;
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
mod failure_tests;
