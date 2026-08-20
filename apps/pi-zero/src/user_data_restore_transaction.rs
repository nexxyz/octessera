use std::fs;
use std::io;
use std::path::Path;

type Tree<'a> = (&'a Path, &'a Path, &'a Path);

#[derive(Clone, Copy)]
struct TreeState {
    original_exists: bool,
}

#[derive(Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct FaultInjection {
    pub(crate) replacement_failure_at: Option<usize>,
    pub(crate) rollback_failure_at: Option<usize>,
    pub(crate) post_swap_rollback_failure_at: Option<usize>,
}

#[derive(Debug)]
struct SwapFailure {
    message: String,
    current_moved: bool,
}

#[derive(Clone, Copy)]
enum RenameKind {
    CurrentToOld,
    ReplacementToCurrent,
    CurrentToReplacement,
    OldToCurrent,
}

pub(crate) fn replace_trees(trees: &[Tree<'_>]) -> Result<(), String> {
    replace_trees_internal(trees, &mut None)
}

#[cfg(test)]
pub(crate) fn replace_trees_with_faults(
    trees: &[Tree<'_>],
    faults: &mut FaultInjection,
) -> Result<(), String> {
    replace_trees_internal(trees, &mut Some(faults))
}

fn replace_trees_internal(
    trees: &[Tree<'_>],
    faults: &mut Option<&mut FaultInjection>,
) -> Result<(), String> {
    let states = trees
        .iter()
        .map(|(current, replacement, old)| {
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
            Ok(TreeState {
                original_exists: path_exists(current),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    for (index, tree) in trees.iter().enumerate() {
        if let Err(failure) = swap_tree_internal(*tree, index, faults) {
            let mut rollback_errors = Vec::new();
            if failure.current_moved {
                if let Err(error) = rollback_tree_internal(*tree, states[index], index, faults) {
                    rollback_errors.push(error);
                }
            }
            for rollback_index in (0..index).rev() {
                if let Err(error) = rollback_tree_internal(
                    trees[rollback_index],
                    states[rollback_index],
                    rollback_index,
                    faults,
                ) {
                    rollback_errors.push(error);
                }
            }
            return Err(transaction_error(failure.message, rollback_errors));
        }
    }

    if let Err(error) = verify_committed_trees(trees) {
        let mut rollback_errors = Vec::new();
        for rollback_index in (0..trees.len()).rev() {
            if let Err(rollback_error) = rollback_tree_internal(
                trees[rollback_index],
                states[rollback_index],
                rollback_index,
                faults,
            ) {
                rollback_errors.push(rollback_error);
            }
        }
        return Err(transaction_error(error, rollback_errors));
    }

    for (current, _, old) in trees {
        if path_exists(current) {
            remove_recovery_tree(old);
        }
    }
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn swap_tree(current: &Path, replacement: &Path, old: &Path) -> Result<(), String> {
    match swap_tree_internal((current, replacement, old), 0, &mut None) {
        Ok(()) => Ok(()),
        Err(failure) if failure.current_moved => match rollback_tree(current, old, replacement) {
            Ok(()) => Err(failure.message),
            Err(error) => Err(transaction_error(failure.message, vec![error])),
        },
        Err(failure) => Err(failure.message),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn rollback_tree(current: &Path, old: &Path, replacement: &Path) -> Result<(), String> {
    rollback_tree_internal(
        (current, replacement, old),
        TreeState {
            original_exists: path_exists(old),
        },
        0,
        &mut None,
    )
}

fn swap_tree_internal(
    (current, replacement, old): Tree<'_>,
    index: usize,
    faults: &mut Option<&mut FaultInjection>,
) -> Result<(), SwapFailure> {
    let current_moved = if path_exists(current) {
        rename_path(current, old, index, RenameKind::CurrentToOld, faults).map_err(|error| {
            SwapFailure {
                message: format!("restore could not move live tree to recovery path: {error}"),
                current_moved: false,
            }
        })?;
        true
    } else {
        false
    };
    rename_path(
        replacement,
        current,
        index,
        RenameKind::ReplacementToCurrent,
        faults,
    )
    .map_err(|error| SwapFailure {
        message: format!("restore replacement swap failed: {error}"),
        current_moved,
    })
}

fn rollback_tree_internal(
    (current, replacement, old): Tree<'_>,
    state: TreeState,
    index: usize,
    faults: &mut Option<&mut FaultInjection>,
) -> Result<(), String> {
    if state.original_exists {
        if !path_exists(old) {
            return Err(format!(
                "restore rollback recovery tree is missing: {}",
                old.display()
            ));
        }
        if path_exists(current) {
            rename_path(
                current,
                replacement,
                index,
                RenameKind::CurrentToReplacement,
                faults,
            )
            .map_err(io_error)?;
        }
        rename_path(old, current, index, RenameKind::OldToCurrent, faults).map_err(io_error)?;
    } else if path_exists(current) {
        rename_path(
            current,
            replacement,
            index,
            RenameKind::CurrentToReplacement,
            faults,
        )
        .map_err(io_error)?;
    }
    if !verify_restored_tree(current, old, state.original_exists) {
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

fn verify_restored_tree(current: &Path, old: &Path, original_exists: bool) -> bool {
    path_exists(current) == original_exists && !path_exists(old)
}

fn rename_path(
    from: &Path,
    to: &Path,
    index: usize,
    kind: RenameKind,
    faults: &mut Option<&mut FaultInjection>,
) -> io::Result<()> {
    #[cfg(not(test))]
    let _ = (index, kind, faults);
    #[cfg(test)]
    if let Some(faults) = faults.as_deref_mut() {
        let failure = match kind {
            RenameKind::ReplacementToCurrent => faults.replacement_failure_at == Some(index),
            RenameKind::OldToCurrent | RenameKind::CurrentToReplacement => {
                if faults.rollback_failure_at == Some(index) {
                    true
                } else {
                    matches!(kind, RenameKind::OldToCurrent)
                        && faults.post_swap_rollback_failure_at == Some(index)
                }
            }
            RenameKind::CurrentToOld => false,
        };
        if failure {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected restore filesystem failure",
            ));
        }
    }
    fs::rename(from, to)
}

fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn remove_recovery_tree(path: &Path) {
    if path_exists(path) {
        let _ = fs::remove_dir_all(path);
    }
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
