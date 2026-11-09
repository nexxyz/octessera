use super::filesystem::{remove_tree_within, Filesystems};
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;

type Tree<'a> = (&'a Path, &'a Path, &'a Path);

/// What one tree's swap moved, so a rollback puts back exactly that. The live folder itself never
/// moves, and entries that stay in place (the SD card mount, a read-only library folder) neither.
#[derive(Default)]
struct TreeSwap {
    original_exists: bool,
    moved_out: Vec<OsString>,
    moved_in: Vec<OsString>,
}

/// Injected rename failures: `(tree, n)` fails that tree's n-th rename (from 0) of a move phase.
#[derive(Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct FaultInjection {
    pub(crate) move_out_failure_at: Option<(usize, usize)>,
    pub(crate) move_in_failure_at: Option<(usize, usize)>,
    pub(crate) rollback_failure_at: Option<usize>,
    pub(crate) post_swap_rollback_failure_at: Option<usize>,
}

#[derive(Clone, Copy)]
enum RenameKind {
    CurrentToOld,
    ReplacementToCurrent,
    CurrentToReplacement,
    OldToCurrent,
}

/// Swaps each live folder's contents for its replacement; `samples` is the live folder that
/// holds the bundled library.
pub(crate) fn replace_trees(
    filesystems: &dyn Filesystems,
    trees: &[Tree<'_>],
    samples: &Path,
) -> Result<(), String> {
    replace_trees_internal(filesystems, trees, samples, &mut None)
}

#[cfg(test)]
pub(crate) fn replace_trees_with_faults(
    filesystems: &dyn Filesystems,
    trees: &[Tree<'_>],
    samples: &Path,
    faults: &mut FaultInjection,
) -> Result<(), String> {
    replace_trees_internal(filesystems, trees, samples, &mut Some(faults))
}

fn replace_trees_internal(
    filesystems: &dyn Filesystems,
    trees: &[Tree<'_>],
    samples: &Path,
    faults: &mut Option<&mut FaultInjection>,
) -> Result<(), String> {
    for (_, replacement, old) in trees {
        if path_exists(old) {
            return Err(format!(
                "restore recovery path already exists: {}",
                old.display()
            ));
        }
        if !path_exists(replacement) {
            return Err(format!(
                "restore replacement path is missing: {}",
                replacement.display()
            ));
        }
    }

    let mut swaps: Vec<TreeSwap> = Vec::with_capacity(trees.len());
    for (index, tree) in trees.iter().enumerate() {
        let mut swap = TreeSwap::default();
        let result = swap_tree(filesystems, *tree, samples, index, &mut swap, faults);
        swaps.push(swap);
        if let Err(message) = result {
            let rollback_errors = rollback_trees(trees, &swaps, faults);
            return Err(transaction_error(message, rollback_errors));
        }
    }

    if let Err(error) = verify_committed_trees(trees) {
        let rollback_errors = rollback_trees(trees, &swaps, faults);
        return Err(transaction_error(error, rollback_errors));
    }

    for (current, _, old) in trees {
        if path_exists(current) && path_exists(old) {
            if let Err(error) = remove_tree_within(filesystems, old, old) {
                eprintln!(
                    "restore succeeded but could not remove its recovery folder {}: {error}",
                    old.display()
                );
            }
        }
    }
    Ok(())
}

fn swap_tree(
    filesystems: &dyn Filesystems,
    (current, replacement, old): Tree<'_>,
    samples: &Path,
    index: usize,
    swap: &mut TreeSwap,
    faults: &mut Option<&mut FaultInjection>,
) -> Result<(), String> {
    swap.original_exists = path_exists(current);
    if !swap.original_exists {
        fs::create_dir(current)
            .map_err(|error| format!("restore could not create {}: {error}", current.display()))?;
    }
    fs::create_dir(old).map_err(|error| {
        format!(
            "restore could not create recovery path {}: {error}",
            old.display()
        )
    })?;
    for name in entry_names(current).map_err(io_error)? {
        let path = current.join(&name);
        if filesystems.stays_in_place(current, &path, current == samples)? {
            continue;
        }
        rename_path(
            &path,
            &old.join(&name),
            (index, swap.moved_out.len()),
            RenameKind::CurrentToOld,
            faults,
        )
        .map_err(|error| format!("restore could not move live tree to recovery path: {error}"))?;
        swap.moved_out.push(name);
    }
    injected_failure((index, 0), RenameKind::ReplacementToCurrent, faults)
        .map_err(|error| format!("restore replacement swap failed: {error}"))?;
    for name in entry_names(replacement).map_err(io_error)? {
        rename_path(
            &replacement.join(&name),
            &current.join(&name),
            (index, swap.moved_in.len()),
            RenameKind::ReplacementToCurrent,
            faults,
        )
        .map_err(|error| format!("restore replacement swap failed: {error}"))?;
        swap.moved_in.push(name);
    }
    fs::remove_dir(replacement).map_err(|error| format!("restore replacement swap failed: {error}"))
}

fn rollback_trees(
    trees: &[Tree<'_>],
    swaps: &[TreeSwap],
    faults: &mut Option<&mut FaultInjection>,
) -> Vec<String> {
    let mut errors = Vec::new();
    for (index, swap) in swaps.iter().enumerate().rev() {
        if let Err(error) = rollback_tree(trees[index], swap, index, faults) {
            errors.push(error);
        }
    }
    errors
}

fn rollback_tree(
    (current, replacement, old): Tree<'_>,
    swap: &TreeSwap,
    index: usize,
    faults: &mut Option<&mut FaultInjection>,
) -> Result<(), String> {
    if !path_exists(replacement) {
        fs::create_dir(replacement).map_err(io_error)?;
    }
    for name in swap.moved_in.iter().rev() {
        rename_path(
            &current.join(name),
            &replacement.join(name),
            (index, 0),
            RenameKind::CurrentToReplacement,
            faults,
        )
        .map_err(io_error)?;
    }
    for name in swap.moved_out.iter().rev() {
        rename_path(
            &old.join(name),
            &current.join(name),
            (index, 0),
            RenameKind::OldToCurrent,
            faults,
        )
        .map_err(io_error)?;
    }
    if path_exists(old) {
        fs::remove_dir(old).map_err(io_error)?;
    }
    if !swap.original_exists && path_exists(current) {
        fs::remove_dir(current).map_err(io_error)?;
    }
    if path_exists(current) != swap.original_exists || path_exists(old) {
        return Err(format!(
            "restore rollback could not verify live tree: {}",
            current.display()
        ));
    }
    Ok(())
}

fn verify_committed_trees(trees: &[Tree<'_>]) -> Result<(), String> {
    for (current, replacement, _) in trees {
        let current_is_tree = fs::symlink_metadata(current)
            .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
            .unwrap_or(false);
        if !current_is_tree || path_exists(replacement) {
            return Err(format!(
                "restore could not verify committed live tree: {} (current={}, replacement={})",
                current.display(),
                current_is_tree,
                path_exists(replacement),
            ));
        }
    }
    Ok(())
}

fn entry_names(path: &Path) -> io::Result<Vec<OsString>> {
    fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect()
}

/// `at` is the tree index and the rename's number within its move phase.
fn rename_path(
    from: &Path,
    to: &Path,
    at: (usize, usize),
    kind: RenameKind,
    faults: &mut Option<&mut FaultInjection>,
) -> io::Result<()> {
    injected_failure(at, kind, faults)?;
    fs::rename(from, to)
}

fn injected_failure(
    at: (usize, usize),
    kind: RenameKind,
    faults: &mut Option<&mut FaultInjection>,
) -> io::Result<()> {
    #[cfg(not(test))]
    let _ = (at, kind, faults);
    #[cfg(test)]
    if let Some(faults) = faults.as_deref_mut() {
        let index = at.0;
        let failure = match kind {
            RenameKind::CurrentToOld => faults.move_out_failure_at == Some(at),
            RenameKind::ReplacementToCurrent => faults.move_in_failure_at == Some(at),
            RenameKind::OldToCurrent | RenameKind::CurrentToReplacement => {
                if faults.rollback_failure_at == Some(index) {
                    true
                } else {
                    matches!(kind, RenameKind::OldToCurrent)
                        && faults.post_swap_rollback_failure_at == Some(index)
                }
            }
        };
        if failure {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected restore filesystem failure",
            ));
        }
    }
    Ok(())
}

fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn transaction_error(message: String, rollback_errors: Vec<String>) -> String {
    if rollback_errors.is_empty() {
        return message;
    }
    format!(
        "{message}; restore rollback failed: {}",
        rollback_errors.join("; ")
    )
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}
