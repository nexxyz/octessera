use super::filesystem::remove_tree_within;
use super::{Filesystems, LiveFilesystems, STAGING_DIR_NAME};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

struct SessionTree {
    live: PathBuf,
    new: PathBuf,
    old: PathBuf,
}

pub(crate) fn finish_interrupted_restore_at_boot() {
    use crate::main_paths::{
        default_recordings_dir, default_samples_dir, default_screen_recordings_dir,
        default_store_dir,
    };
    finish_interrupted_restore(
        &LiveFilesystems,
        &default_store_dir(),
        &default_samples_dir(),
        &default_recordings_dir(),
        &default_screen_recordings_dir(),
    );
}

/// Completes a restore swap a power cut interrupted. Every tree's new folder is fully built before
/// the first old folder exists, so a session with no old folder only needs its new folders deleted;
/// a session with one is rolled forward, tree by tree, and its old folders go once the pre-restore
/// archive is confirmed. A failed session is left as it is and picks up again at the next boot.
fn finish_interrupted_restore(
    filesystems: &dyn Filesystems,
    store: &Path,
    samples: &Path,
    recordings: &Path,
    screen_recordings: &Path,
) {
    let Some(staging) = store.parent().map(|parent| parent.join(STAGING_DIR_NAME)) else {
        return;
    };
    let live_trees = live_trees(store, samples, recordings, screen_recordings);
    for session in leftover_sessions(&staging, &live_trees) {
        let trees = live_trees.map(|(label, live)| SessionTree {
            live: live.to_path_buf(),
            new: staging.join(format!("{}{session}", staged_prefix(label, "new"))),
            old: staging.join(format!("{}{session}", staged_prefix(label, "old"))),
        });
        let archive = staging.join(format!("octessera-pre-restore-{session}.oct"));
        if let Err(error) = recover_session(filesystems, &trees, samples, &archive) {
            eprintln!("could not finish the interrupted restore {session}: {error}");
        }
    }
}

/// Whether `staging` still holds a new or old folder of some session, which only the boot sweep
/// may finish: a restore on top of it could be rolled forward over at the next boot.
pub(super) fn has_leftover_session(
    staging: &Path,
    store: &Path,
    samples: &Path,
    recordings: &Path,
    screen_recordings: &Path,
) -> bool {
    !leftover_sessions(
        staging,
        &live_trees(store, samples, recordings, screen_recordings),
    )
    .is_empty()
}

fn live_trees<'a>(
    store: &'a Path,
    samples: &'a Path,
    recordings: &'a Path,
    screen_recordings: &'a Path,
) -> [(&'a str, &'a Path); 4] {
    [
        ("store", store),
        ("samples", samples),
        (file_label(recordings), recordings),
        (file_label(screen_recordings), screen_recordings),
    ]
}

/// The store and samples folders are named label first, the media folders role first, as `restore` names them.
fn staged_prefix(label: &str, role: &str) -> String {
    if matches!(label, "store" | "samples") {
        format!(".octessera-{label}-{role}-")
    } else {
        format!(".octessera-{role}-{label}-")
    }
}

fn file_label(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("media")
}

fn leftover_sessions(staging: &Path, live_trees: &[(&str, &Path)]) -> BTreeSet<String> {
    let Ok(entries) = fs::read_dir(staging) else {
        return BTreeSet::new();
    };
    let mut sessions = BTreeSet::new();
    for entry in entries.filter_map(Result::ok) {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        for (label, _) in live_trees {
            for role in ["new", "old"] {
                if let Some(session) = name.strip_prefix(&staged_prefix(label, role)) {
                    sessions.insert(session.to_string());
                }
            }
        }
    }
    sessions
}

fn recover_session(
    filesystems: &dyn Filesystems,
    trees: &[SessionTree],
    samples: &Path,
    archive: &Path,
) -> Result<(), String> {
    if !trees.iter().any(|tree| exists(&tree.old)) {
        for tree in trees.iter().filter(|tree| exists(&tree.new)) {
            remove_tree_within(filesystems, &tree.new, &tree.new).map_err(io_text)?;
        }
        return Ok(());
    }
    for tree in trees.iter().filter(|tree| exists(&tree.new)) {
        roll_forward_tree(filesystems, tree, tree.live == samples)?;
    }
    if !exists(archive) {
        return Err(format!(
            "pre-restore archive {} is missing, so the old folders stay",
            archive.display()
        ));
    }
    for tree in trees.iter().filter(|tree| exists(&tree.old)) {
        remove_tree_within(filesystems, &tree.old, &tree.old).map_err(io_text)?;
    }
    Ok(())
}

/// Live entries move out to the old folder while an entry of the new folder still has its
/// namesake in live, which proves no new entry has moved in yet; after that, live holds moved-in
/// entries that must stay.
fn roll_forward_tree(
    filesystems: &dyn Filesystems,
    tree: &SessionTree,
    holds_library: bool,
) -> Result<(), String> {
    fs::create_dir_all(&tree.live).map_err(io_text)?;
    fs::create_dir_all(&tree.old).map_err(io_text)?;
    let new_names = entry_names(&tree.new)?;
    let mut movable = Vec::new();
    for name in entry_names(&tree.live)? {
        if !filesystems.stays_in_place(&tree.live, &tree.live.join(&name), holds_library)? {
            movable.push(name);
        }
    }
    if movable.iter().any(|name| new_names.contains(name)) {
        for name in &movable {
            fs::rename(tree.live.join(name), tree.old.join(name)).map_err(io_text)?;
        }
    }
    for name in &new_names {
        fs::rename(tree.new.join(name), tree.live.join(name)).map_err(io_text)?;
    }
    fs::remove_dir(&tree.new).map_err(io_text)
}

fn entry_names(path: &Path) -> Result<Vec<OsString>, String> {
    fs::read_dir(path)
        .map_err(io_text)?
        .map(|entry| entry.map(|entry| entry.file_name()).map_err(io_text))
        .collect()
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn io_text(error: std::io::Error) -> String {
    error.to_string()
}
