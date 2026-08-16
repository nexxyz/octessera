use std::fmt;
use std::path::{Path, PathBuf};

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
    InvalidBundledDefault { detail: String },
    SeedDefault { path: PathBuf, source: String },
    ReadExistingDefault { path: PathBuf, source: String },
    ParseExistingDefault { path: PathBuf, source: String },
    RepairDefaultBrightness { path: PathBuf, source: String },
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
            Self::InvalidBundledDefault { detail } => write!(
                formatter,
                "desktop startup store initialization failed: bundled default is invalid: {detail}"
            ),
            Self::SeedDefault { path, source } => write!(
                formatter,
                "desktop startup store initialization failed: unable to atomically seed bundled default {}: {source}",
                path.display()
            ),
            Self::ReadExistingDefault { path, source } => write!(
                formatter,
                "desktop startup store initialization failed: unable to read existing default {}: {source}",
                path.display()
            ),
            Self::ParseExistingDefault { path, source } => write!(
                formatter,
                "desktop startup store initialization failed: unable to parse existing default {}: {source}",
                path.display()
            ),
            Self::RepairDefaultBrightness { path, source } => write!(
                formatter,
                "desktop startup store initialization failed: unable to atomically repair default brightness {}: {source}",
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
    ensure_store_dir_at_with_writer(dir, persistence::atomic_write_json)
}

fn ensure_store_dir_at_with_writer<F>(
    dir: PathBuf,
    write_json: F,
) -> Result<PathBuf, DesktopStoreStartupError>
where
    F: Fn(&Path, &serde_json::Value) -> Result<(), String>,
{
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
    let bundled: serde_json::Value =
        serde_json::from_str(BUNDLED_DEFAULT_CONFIG).map_err(|error| {
            DesktopStoreStartupError::ParseBundledDefault {
                source: error.to_string(),
            }
        })?;
    let default_path = dir.join("default.json");
    if !default_path.is_file() {
        write_json(&default_path, &bundled).map_err(|source| {
            DesktopStoreStartupError::SeedDefault {
                path: default_path.clone(),
                source,
            }
        })?;
    } else {
        repair_existing_desktop_default_brightness(&default_path, &bundled, &write_json)?;
    }
    Ok(dir)
}

fn repair_existing_desktop_default_brightness<F>(
    path: &Path,
    bundled: &serde_json::Value,
    write_json: &F,
) -> Result<(), DesktopStoreStartupError>
where
    F: Fn(&Path, &serde_json::Value) -> Result<(), String>,
{
    let mut payload: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).map_err(|error| {
            DesktopStoreStartupError::ReadExistingDefault {
                path: path.to_path_buf(),
                source: error.to_string(),
            }
        })?)
        .map_err(|error| DesktopStoreStartupError::ParseExistingDefault {
            path: path.to_path_buf(),
            source: error.to_string(),
        })?;
    let Some(runtime) = payload
        .get_mut("runtimeConfig")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Ok(());
    };
    let bundled_runtime = bundled
        .get("runtimeConfig")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| DesktopStoreStartupError::InvalidBundledDefault {
            detail: "missing runtimeConfig".to_string(),
        })?;
    let old_pi_defaults = [
        ("buttonBrightness", 35_u64),
        ("displayBrightness", 75_u64),
        ("gridBrightness", 25_u64),
    ];
    let mut changed = false;
    for (key, old_value) in old_pi_defaults {
        let should_repair = runtime
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .is_none_or(|current| current == old_value);
        if !should_repair {
            continue;
        }
        let Some(next) = bundled_runtime.get(key) else {
            continue;
        };
        if runtime.get(key) != Some(next) {
            runtime.insert(key.to_string(), next.clone());
            changed = true;
        }
    }
    if changed {
        write_json(path, &payload).map_err(|source| {
            DesktopStoreStartupError::RepairDefaultBrightness {
                path: path.to_path_buf(),
                source,
            }
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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

    fn failing_json_write(_: &Path, _: &serde_json::Value) -> Result<(), String> {
        Err("injected atomic write failure".to_string())
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
    fn malformed_existing_default_is_reported() {
        let root = unique_temp_dir("octessera-malformed-default");
        fs::write(root.join("default.json"), b"{ malformed").expect("malformed default");

        let error = ensure_store_dir_at(root.clone()).expect_err("malformed default must fail");

        assert!(error.to_string().starts_with(
            "desktop startup store initialization failed: unable to parse existing default"
        ));
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
    fn brightness_repair_write_failure_is_reported() {
        let root = unique_temp_dir("octessera-brightness-write-failure");
        fs::write(
            root.join("default.json"),
            serde_json::to_vec(&serde_json::json!({
                "runtimeConfig": {
                    "buttonBrightness": 35,
                    "displayBrightness": 75,
                    "gridBrightness": 25,
                    "masterVolume": 82
                }
            }))
            .expect("serialize old default"),
        )
        .expect("write old default");

        let error = ensure_store_dir_at_with_writer(root.clone(), failing_json_write)
            .expect_err("brightness repair write must fail");

        assert!(error.to_string().starts_with(
            "desktop startup store initialization failed: unable to atomically repair default brightness"
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
    fn valid_existing_default_repairs_only_legacy_brightness_values() {
        let root = unique_temp_dir("octessera-existing-default");
        fs::write(
            root.join("default.json"),
            serde_json::to_vec(&serde_json::json!({
                "runtimeConfig": {
                    "buttonBrightness": 35,
                    "displayBrightness": 88,
                    "gridBrightness": 25,
                    "masterVolume": 82
                }
            }))
            .expect("serialize existing default"),
        )
        .expect("write existing default");

        ensure_store_dir_at(root.clone()).expect("existing default");

        let actual: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("default.json")).expect("default"))
                .expect("parse default");
        assert_eq!(actual["runtimeConfig"]["buttonBrightness"], 100);
        assert_eq!(actual["runtimeConfig"]["displayBrightness"], 88);
        assert_eq!(actual["runtimeConfig"]["gridBrightness"], 100);
        assert_eq!(actual["runtimeConfig"]["masterVolume"], 82);
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
