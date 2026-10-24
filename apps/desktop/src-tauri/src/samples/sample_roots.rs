use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::path::BaseDirectory;
use tauri::Manager;

static SAMPLES_ROOT: OnceLock<PathBuf> = OnceLock::new();

pub(crate) fn initialize_samples_root(app: &tauri::App) -> Result<(), String> {
    let resource = app
        .path()
        .resolve("samples", BaseDirectory::Resource)
        .map_err(|e| format!("samples resource resolve failed: {e}"))?;
    let root = match resource_state(&resource) {
        Ok(root) => root,
        Err(error) => {
            if cfg!(debug_assertions) || cfg!(test) {
                resolve_dev_samples_root().map_err(|fallback| format!("{error}; {fallback}"))?
            } else {
                return Err(error);
            }
        }
    };
    SAMPLES_ROOT
        .set(root)
        .map_err(|_| "samples root initialized more than once".to_string())
}

pub(super) fn resolve_samples_root() -> Result<PathBuf, String> {
    if let Some(root) = SAMPLES_ROOT.get() {
        return Ok(root.clone());
    }
    if cfg!(debug_assertions) || cfg!(test) {
        resolve_dev_samples_root()
    } else {
        Err("samples resource was not initialized".to_string())
    }
}

fn resource_state(resource: &Path) -> Result<PathBuf, String> {
    if resource.is_dir() && !resource.is_symlink() {
        return Ok(resource.to_path_buf());
    }
    Err(format!(
        "samples resource is not a real directory: {}",
        resource.display()
    ))
}

fn resolve_dev_samples_root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cwd failed: {e}"))?;
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root_samples = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(|p| p.join("samples"));
    if let Some(path) = repo_root_samples {
        if path.exists() && path.is_dir() && !path.is_symlink() {
            return Ok(path);
        }
    }
    for candidate in sample_root_candidates(&cwd, &manifest_dir) {
        if candidate.is_dir() && !candidate.is_symlink() {
            return Ok(candidate);
        }
    }
    Err("samples resource and development sample library are unavailable".to_string())
}

pub(super) fn resolve_user_samples_root() -> Result<PathBuf, String> {
    let dir = user_data_dir()?.join("samples");
    fs::create_dir_all(&dir).map_err(|e| format!("create user samples dir failed: {e}"))?;
    Ok(dir)
}

fn user_data_dir() -> Result<PathBuf, String> {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Ok(PathBuf::from(appdata).join("octessera"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("octessera"));
    }
    let cwd = std::env::current_dir().map_err(|e| format!("cwd failed: {e}"))?;
    Ok(cwd.join("userdata"))
}

fn sample_root_candidates(cwd: &Path, manifest_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = parent_sample_dirs(cwd);
    candidates.extend(parent_sample_dirs(manifest_dir));
    candidates
}

fn parent_sample_dirs(base: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![base.join("samples")];
    let mut current = base.parent();
    for _ in 0..3 {
        let Some(parent) = current else { break };
        candidates.push(parent.join("samples"));
        current = parent.parent();
    }
    candidates
}
