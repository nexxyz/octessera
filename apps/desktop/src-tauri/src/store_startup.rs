use std::fmt;
use std::path::PathBuf;

use tauri::Manager;

use crate::persistence;

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
        }
    }
}

impl std::error::Error for DesktopStoreStartupError {}

pub(crate) fn ensure_store_dir(app: &tauri::App) -> Result<PathBuf, DesktopStoreStartupError> {
    let dir = resolve_store_root(
        std::env::var_os("OCTESSERA_DESKTOP_STORE_DIR").map(PathBuf::from),
        || app.path().app_data_dir().map_err(|error| error.to_string()),
    )?;
    ensure_store_dir_at(dir)
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
    let legacy_path = dir.join("default.json");
    if legacy_path.exists() {
        return Err(DesktopStoreStartupError::InvalidStore {
            detail: format!(
                "an old single-file default.json is in the store folder ({}); move it out of that folder and start again, and the simulator creates a fresh default",
                legacy_path.display()
            ),
        });
    }
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
        (false, false) if fresh_store_root(dir)? => {
            let bundled: serde_json::Value = serde_json::from_str(BUNDLED_DEFAULT_CONFIG)
                .map_err(|error| DesktopStoreStartupError::ParseBundledDefault {
                    source: error.to_string(),
                })?;
            let documents = split_system_patch_documents(&bundled).map_err(|error| {
                DesktopStoreStartupError::ParseBundledDefault { source: error }
            })?;
            persistence::atomic_write_json(&system_path, &documents.system).map_err(|source| {
                DesktopStoreStartupError::SeedDocument {
                    path: system_path.clone(),
                    source,
                }
            })?;
            persistence::atomic_write_json(&patch_path, &documents.patch).map_err(|source| {
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
                "the store is incomplete (system.json: {system_exists}, default.patch.json: {patch_exists}); move both files out of the store folder and start again for a fresh default"
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

fn fresh_store_root(dir: &std::path::Path) -> Result<bool, DesktopStoreStartupError> {
    for entry in std::fs::read_dir(dir).map_err(|error| DesktopStoreStartupError::InvalidStore {
        detail: format!("unable to inspect store root: {error}"),
    })? {
        let entry = entry.map_err(|error| DesktopStoreStartupError::InvalidStore {
            detail: format!("unable to inspect store root: {error}"),
        })?;
        if entry.file_name() != "presets" || !entry.path().is_dir() {
            return Ok(false);
        }
        if std::fs::read_dir(entry.path())
            .map_err(|error| DesktopStoreStartupError::InvalidStore {
                detail: format!("unable to inspect preset directory: {error}"),
            })?
            .next()
            .is_some()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests;
