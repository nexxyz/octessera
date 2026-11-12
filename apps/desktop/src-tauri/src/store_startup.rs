use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use tauri::Manager;

const BUNDLED_DEFAULT_CONFIG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../config/generated/desktop/default.json"
));

#[derive(Debug)]
pub(crate) enum DesktopStoreStartupError {
    ResolveAppData { source: String },
    CreateStoreDirectory { path: PathBuf, source: String },
    CreatePresetDirectory { path: PathBuf, source: String },
    ParseBundledDefault { source: String },
    SeedDocument { path: PathBuf, source: String },
    InvalidStore { detail: String },
    LockStore { path: PathBuf, source: String },
    AlreadyRunning { path: PathBuf },
}

impl fmt::Display for DesktopStoreStartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResolveAppData { source } => write!(
                formatter,
                "desktop startup store initialization failed: unable to resolve app-data directory: {source}"
            ),
            Self::CreateStoreDirectory { path, source } => write!(
                formatter,
                "desktop startup store initialization failed: unable to create store directory {}: {source}",
                path.display()
            ),
            Self::CreatePresetDirectory { path, source } => write!(
                formatter,
                "desktop startup store initialization failed: unable to create preset directory {}: {source}",
                path.display()
            ),
            Self::ParseBundledDefault { source } => write!(
                formatter,
                "desktop startup store initialization failed: unable to parse bundled default: {source}"
            ),
            Self::SeedDocument { path, source } => write!(
                formatter,
                "desktop startup store initialization failed: unable to atomically seed split document {}: {source}",
                path.display()
            ),
            Self::InvalidStore { detail } => write!(
                formatter,
                "desktop storage can't be opened: {detail}"
            ),
            Self::LockStore { path, source } => write!(
                formatter,
                "desktop startup store initialization failed: unable to lock store directory {}: {source}",
                path.display()
            ),
            Self::AlreadyRunning { path } => write!(
                formatter,
                "Octessera is already running with the store in {}. Close the other Octessera window, then start this one again.",
                path.display()
            ),
        }
    }
}

impl std::error::Error for DesktopStoreStartupError {}

/// Held while the app runs, so a second instance cannot repair this one's live take or race its
/// saves. The OS releases it when the process ends, however it ends.
#[derive(Debug)]
pub(crate) struct StoreLock {
    _file: File,
}

/// Takes the store lock before anything reads the store, then prepares the store.
pub(crate) fn ensure_store_dir(
    app: &tauri::App,
) -> Result<(PathBuf, StoreLock), DesktopStoreStartupError> {
    let dir = resolve_store_root(explicit_store_root(), || {
        app.path().app_data_dir().map_err(|error| error.to_string())
    })?;
    std::fs::create_dir_all(&dir).map_err(|error| {
        DesktopStoreStartupError::CreateStoreDirectory {
            path: dir.clone(),
            source: error.to_string(),
        }
    })?;
    let lock = lock_store_dir(&dir)?;
    Ok((ensure_store_dir_at(dir)?, lock))
}

fn lock_store_dir(dir: &Path) -> Result<StoreLock, DesktopStoreStartupError> {
    let lock_error = |source: String| DesktopStoreStartupError::LockStore {
        path: dir.to_path_buf(),
        source,
    };
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("octessera.lock"))
        .map_err(|error| lock_error(error.to_string()))?;
    match file.try_lock() {
        Ok(()) => Ok(StoreLock { _file: file }),
        Err(TryLockError::WouldBlock) => Err(DesktopStoreStartupError::AlreadyRunning {
            path: dir.to_path_buf(),
        }),
        Err(TryLockError::Error(error)) => Err(lock_error(error.to_string())),
    }
}

pub(crate) fn explicit_store_root() -> Option<PathBuf> {
    std::env::var_os("OCTESSERA_DESKTOP_STORE_DIR").map(PathBuf::from)
}

fn resolve_store_root<F>(
    explicit_root: Option<PathBuf>,
    app_data_dir: F,
) -> Result<PathBuf, DesktopStoreStartupError>
where
    F: FnOnce() -> Result<PathBuf, String>,
{
    if let Some(dir) = explicit_root {
        return Ok(dir);
    }
    app_data_dir().map_err(|source| DesktopStoreStartupError::ResolveAppData { source })
}

pub(crate) fn ensure_store_dir_at(dir: PathBuf) -> Result<PathBuf, DesktopStoreStartupError> {
    std::fs::create_dir_all(&dir).map_err(|error| {
        DesktopStoreStartupError::CreateStoreDirectory {
            path: dir.clone(),
            source: error.to_string(),
        }
    })?;
    let presets_dir = dir.join("presets");
    if presets_dir.is_file() {
        std::fs::create_dir_all(&presets_dir).map_err(|error| {
            DesktopStoreStartupError::CreatePresetDirectory {
                path: presets_dir.clone(),
                source: error.to_string(),
            }
        })?;
    }
    ensure_split_documents(&dir)?;
    std::fs::create_dir_all(&presets_dir).map_err(|error| {
        DesktopStoreStartupError::CreatePresetDirectory {
            path: presets_dir.clone(),
            source: error.to_string(),
        }
    })?;
    Ok(dir)
}

fn ensure_split_documents(dir: &std::path::Path) -> Result<(), DesktopStoreStartupError> {
    use playback_runtime::{compose_system_patch_documents, split_system_patch_documents};

    let system_path = dir.join("system.json");
    let patch_path = dir.join("default.patch.json");
    match (system_path.is_file(), patch_path.is_file()) {
        (true, true) => {
            let system = read_document(&system_path)?;
            let patch = read_document(&patch_path)?;
            compose_system_patch_documents(&system, &patch).map_err(|error| {
                DesktopStoreStartupError::InvalidStore {
                    detail: format!("split documents are invalid; preserve both files: {error}"),
                }
            })?;
            Ok(())
        }
        (false, false) => {
            let bundled: serde_json::Value = serde_json::from_str(BUNDLED_DEFAULT_CONFIG)
                .map_err(|error| DesktopStoreStartupError::ParseBundledDefault {
                    source: error.to_string(),
                })?;
            let documents = split_system_patch_documents(&bundled).map_err(|error| {
                DesktopStoreStartupError::ParseBundledDefault { source: error }
            })?;
            store_files::write_json_atomically(&system_path, &documents.system).map_err(|source| {
                DesktopStoreStartupError::SeedDocument {
                    path: system_path.clone(),
                    source,
                }
            })?;
            store_files::write_json_atomically(&patch_path, &documents.patch).map_err(|source| {
                DesktopStoreStartupError::SeedDocument {
                    path: patch_path.clone(),
                    source,
                }
            })?;
            let system = read_document(&system_path)?;
            let patch = read_document(&patch_path)?;
            compose_system_patch_documents(&system, &patch).map_err(|error| {
                DesktopStoreStartupError::InvalidStore {
                    detail: format!("seeded split documents failed readback validation: {error}"),
                }
            })?;
            if system != documents.system || patch != documents.patch {
                return Err(DesktopStoreStartupError::InvalidStore {
                    detail: "seeded split documents differ from the native projection".into(),
                });
            }
            Ok(())
        }
        (system_exists, patch_exists) => Err(DesktopStoreStartupError::InvalidStore {
            detail: format!(
                "the store is incomplete (system.json: {system_exists}, default.patch.json: {patch_exists}); move the remaining file out of the store folder and start again for a fresh default"
            ),
        }),
    }
}

fn read_document(path: &std::path::Path) -> Result<serde_json::Value, DesktopStoreStartupError> {
    let content =
        std::fs::read_to_string(path).map_err(|error| DesktopStoreStartupError::InvalidStore {
            detail: format!("unable to read {}: {error}", path.display()),
        })?;
    serde_json::from_str(&content).map_err(|error| DesktopStoreStartupError::InvalidStore {
        detail: format!(
            "invalid JSON in {}; preserve existing bytes: {error}",
            path.display()
        ),
    })
}

#[cfg(test)]
mod tests;
