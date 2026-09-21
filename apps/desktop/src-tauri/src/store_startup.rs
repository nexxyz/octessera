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
    SeedDefault { path: PathBuf, source: String },
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
            Self::SeedDefault { path, source } => write!(
                formatter,
                "desktop startup store initialization failed: unable to atomically seed bundled default {}: {source}",
                path.display()
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

fn ensure_store_dir_at(dir: PathBuf) -> Result<PathBuf, DesktopStoreStartupError> {
    std::fs::create_dir_all(&dir).map_err(|error| {
        DesktopStoreStartupError::CreateStoreDirectory {
            path: dir.clone(),
            source: error.to_string(),
        }
    })?;
    let presets_dir = dir.join("presets");
    std::fs::create_dir_all(&presets_dir).map_err(|error| {
        DesktopStoreStartupError::CreatePresetDirectory {
            path: presets_dir.clone(),
            source: error.to_string(),
        }
    })?;
    let default_path = dir.join("default.json");
    if !default_path.is_file() {
        let bundled: serde_json::Value =
            serde_json::from_str(BUNDLED_DEFAULT_CONFIG).map_err(|error| {
                DesktopStoreStartupError::ParseBundledDefault {
                    source: error.to_string(),
                }
            })?;
        persistence::atomic_write_json(&default_path, &bundled).map_err(|source| {
            DesktopStoreStartupError::SeedDefault {
                path: default_path.clone(),
                source,
            }
        })?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("create temp directory");
        dir
    }

    fn remove_temp_dir(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn store_root_file_is_reported_at_startup() {
        let parent = unique_temp_dir("octessera-store-root-file");
        let root = parent.join("store");
        fs::write(&root, b"not a directory").expect("store root file");

        let error = ensure_store_dir_at(root).expect_err("file root must fail");

        assert!(error.to_string().starts_with(
            "desktop startup store initialization failed: unable to create store directory"
        ));
        remove_temp_dir(&parent);
    }

    #[test]
    fn preset_directory_creation_failure_is_reported() {
        let root = unique_temp_dir("octessera-preset-directory-file");
        fs::write(root.join("presets"), b"not a directory").expect("preset directory file");

        let error = ensure_store_dir_at(root.clone()).expect_err("preset file must fail");

        assert!(error.to_string().starts_with(
            "desktop startup store initialization failed: unable to create preset directory"
        ));
        remove_temp_dir(&root);
    }

    #[test]
    fn malformed_existing_default_is_preserved() {
        let root = unique_temp_dir("octessera-malformed-default");
        let original = b"{ malformed";
        fs::write(root.join("default.json"), original).expect("malformed default");

        ensure_store_dir_at(root.clone()).expect("existing malformed default");

        assert_eq!(
            fs::read(root.join("default.json")).expect("existing default"),
            original
        );
        remove_temp_dir(&root);
    }

    #[test]
    fn seed_write_failure_is_reported() {
        let root = unique_temp_dir("octessera-seed-write-failure");
        fs::create_dir(root.join("default.json")).expect("default path directory");

        let error = ensure_store_dir_at(root.clone()).expect_err("seed write must fail");

        assert!(error.to_string().starts_with(
            "desktop startup store initialization failed: unable to atomically seed bundled default"
        ));
        remove_temp_dir(&root);
    }

    #[test]
    fn valid_first_seed_creates_default_and_presets() {
        let root = unique_temp_dir("octessera-valid-first-seed");

        ensure_store_dir_at(root.clone()).expect("first seed");

        let payload: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(root.join("default.json")).expect("seeded default"),
        )
        .expect("parse seeded default");
        assert_eq!(payload["runtimeConfig"]["layers"][3]["autoName"], true);
        assert_eq!(payload["runtimeConfig"]["displayBrightness"], 100);
        assert_eq!(payload["runtimeConfig"]["gridBrightness"], 100);
        assert_eq!(payload["runtimeConfig"]["buttonBrightness"], 100);
        assert!(root.join("presets").is_dir());
        remove_temp_dir(&root);
    }

    #[test]
    fn valid_existing_custom_default_is_preserved() {
        let root = unique_temp_dir("octessera-existing-custom-default");
        let custom = serde_json::json!({ "kept": true });
        fs::write(
            root.join("default.json"),
            serde_json::to_vec(&custom).expect("serialize custom default"),
        )
        .expect("write custom default");

        ensure_store_dir_at(root.clone()).expect("existing custom default");

        let actual: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(root.join("default.json")).expect("custom default"),
        )
        .expect("parse custom default");
        assert_eq!(actual, custom);
        remove_temp_dir(&root);
    }

    #[test]
    fn explicit_env_store_root_is_used_without_app_data_resolution() {
        let root = unique_temp_dir("octessera-explicit-store-root");
        let selected = resolve_store_root(Some(root.clone()), || {
            Err("app-data resolution should not be called".to_string())
        })
        .expect("explicit store root");

        ensure_store_dir_at(selected.clone()).expect("explicit store root initialization");
        assert!(selected.join("default.json").is_file());
        remove_temp_dir(&root);
    }

    #[test]
    fn app_data_resolution_failure_is_not_replaced_with_executable_relative_path() {
        let error = resolve_store_root(None, || Err("app-data unavailable".to_string()))
            .expect_err("app-data failure must stop startup");

        assert_eq!(
            error.to_string(),
            "desktop startup store initialization failed: unable to resolve app-data directory: app-data unavailable"
        );
    }
}
