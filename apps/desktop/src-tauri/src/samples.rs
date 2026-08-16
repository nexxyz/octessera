use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::path::BaseDirectory;
use tauri::Manager;

static SAMPLES_ROOT: OnceLock<PathBuf> = OnceLock::new();

#[derive(Serialize)]
pub struct SampleEntry {
    pub(crate) name: String,
    pub(crate) path: String,
    #[serde(rename = "isDir")]
    pub(crate) is_dir: bool,
}

#[tauri::command]
pub fn sample_list(dir: String) -> Result<Vec<SampleEntry>, String> {
    let root = resolve_samples_root()?;
    sample_list_from_root(&root, &dir)
}

pub(crate) fn resolve_sample_file(path: &str) -> Option<String> {
    let root = resolve_samples_root().ok()?;
    let normalized = canonical_sample_relative_path(path).ok()?;
    if normalized == "userdata" || normalized.starts_with("userdata/") {
        let user_root = resolve_user_samples_root().ok()?;
        return resolve_sample_file_from_roots(&root, &user_root, path);
    }
    resolve_sample_file_in_root(&root, &normalized)
}

fn sample_list_from_root(root: &PathBuf, dir: &str) -> Result<Vec<SampleEntry>, String> {
    let rel = canonical_sample_relative_path(dir)?;
    if rel == "userdata" || rel.starts_with("userdata/") {
        let user_root = resolve_user_samples_root()?;
        return sample_list_from_roots(root, &user_root, dir);
    }
    sample_list_from_roots(root, root, dir)
}

fn sample_list_from_roots(
    root: &Path,
    user_root: &Path,
    dir: &str,
) -> Result<Vec<SampleEntry>, String> {
    let rel = canonical_sample_relative_path(dir)?;
    if rel == "userdata" || rel.starts_with("userdata/") {
        let user_rel = canonical_sample_relative_path(rel.strip_prefix("userdata/").unwrap_or(""))?;
        let canon_user_root = fs::canonicalize(user_root)
            .map_err(|e| format!("user samples root resolve failed: {e}"))?;
        let canon_target = canonical_sample_dir(user_root, &user_rel, &canon_user_root)?;
        let mut out = read_sample_entries(&canon_target, &user_rel, "userdata")?;
        sort_sample_entries(&mut out);
        return Ok(out);
    }
    let canon_root =
        fs::canonicalize(root).map_err(|e| format!("samples root resolve failed: {e}"))?;
    let canon_target = canonical_sample_dir(root, &rel, &canon_root)?;
    let mut out = read_sample_entries(&canon_target, &rel, "samples")?;
    sort_sample_entries(&mut out);
    Ok(out)
}

fn canonical_sample_dir(root: &Path, rel: &str, canon_root: &Path) -> Result<PathBuf, String> {
    if fs::symlink_metadata(root)
        .map_err(|e| format!("samples root metadata failed: {e}"))?
        .file_type()
        .is_symlink()
    {
        return Err("samples root is symlinked".to_string());
    }
    reject_symlink_components(root, rel)?;
    let target = root.join(rel);
    let canon_target =
        fs::canonicalize(&target).map_err(|e| format!("directory not found: {e}"))?;
    if !canon_target.starts_with(canon_root) {
        return Err("path outside samples root".to_string());
    }
    Ok(canon_target)
}

fn read_sample_entries(
    canon_target: &Path,
    rel: &str,
    virtual_prefix: &str,
) -> Result<Vec<SampleEntry>, String> {
    let mut out = Vec::new();
    for entry in fs::read_dir(canon_target).map_err(|e| format!("read dir failed: {e}"))? {
        let entry = entry.map_err(|err| format!("read dir entry failed: {err}"))?;
        if let Some(sample_entry) = sample_entry_from_dir_entry(&entry, rel)? {
            out.push(if virtual_prefix.is_empty() {
                sample_entry
            } else {
                SampleEntry {
                    path: rel_join(virtual_prefix, &sample_entry.path),
                    ..sample_entry
                }
            });
        }
    }
    Ok(out)
}

fn sample_entry_from_dir_entry(
    entry: &fs::DirEntry,
    rel: &str,
) -> Result<Option<SampleEntry>, String> {
    if entry
        .file_type()
        .map_err(|err| format!("read entry type failed: {err}"))?
        .is_symlink()
    {
        return Err(format!(
            "sample tree contains a symlink: {}",
            entry.path().display()
        ));
    }
    let meta = entry
        .metadata()
        .map_err(|err| format!("read metadata failed: {err}"))?;
    let is_dir = meta.is_dir();
    if !is_dir && !meta.is_file() {
        return Err(format!(
            "sample tree contains a special entry: {}",
            entry.path().display()
        ));
    }
    let file_name = entry.file_name().to_string_lossy().to_string();
    if file_name.contains('\\') {
        return Err(format!(
            "sample tree contains an invalid filename: {}",
            entry.path().display()
        ));
    }
    if !is_dir && !supported_sample_file(&entry.path()) {
        return Ok(None);
    }
    Ok(Some(SampleEntry {
        name: file_name.clone(),
        path: rel_join(rel, &file_name),
        is_dir,
    }))
}

fn supported_sample_file(path: &Path) -> bool {
    path.extension()
        .map(|x| x.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
        == "wav"
}

fn sort_sample_entries(entries: &mut [SampleEntry]) {
    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });
}

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

fn resolve_samples_root() -> Result<PathBuf, String> {
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
        if path.exists() {
            if path.is_dir() && !path.is_symlink() {
                return Ok(path);
            }
        }
    }
    for candidate in sample_root_candidates(&cwd, &manifest_dir) {
        if candidate.is_dir() && !candidate.is_symlink() {
            return Ok(candidate);
        }
    }
    Err("samples resource and development sample library are unavailable".to_string())
}

fn resolve_user_samples_root() -> Result<PathBuf, String> {
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

fn sanitize_relative_dir(input: &str) -> Result<String, String> {
    let trimmed = input.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.starts_with('/') {
        return Err("absolute path is not allowed".to_string());
    }
    if trimmed
        .split('/')
        .next()
        .is_some_and(|part| part.ends_with(':'))
    {
        return Err("absolute path is not allowed".to_string());
    }
    let mut parts: Vec<String> = Vec::new();
    for p in trimmed.split('/') {
        if p.is_empty() || p == "." {
            continue;
        }
        if p == ".." {
            return Err("parent traversal is not allowed".to_string());
        }
        parts.push(p.to_string());
    }
    Ok(parts.join("/"))
}

fn canonical_sample_relative_path(input: &str) -> Result<String, String> {
    let rel = sanitize_relative_dir(input)?;
    if rel == "sd-card" || rel.starts_with("sd-card/") {
        return Err("SD card sample paths are not supported on desktop".to_string());
    }
    if rel == "samples" {
        return Ok(String::new());
    }
    if let Some(relative) = rel.strip_prefix("samples/") {
        return Ok(relative.to_string());
    }
    Ok(rel)
}

fn rel_join(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_string()
    } else {
        format!("{base}/{name}")
    }
}

fn resolve_sample_file_from_roots(
    root: &PathBuf,
    user_root: &PathBuf,
    path: &str,
) -> Option<String> {
    let rel = canonical_sample_relative_path(path).ok()?;
    if rel.is_empty() {
        return None;
    }
    if let Some(user_rel) = rel.strip_prefix("userdata/") {
        let user_rel = canonical_sample_relative_path(user_rel).ok()?;
        return resolve_sample_file_in_root(user_root, &user_rel);
    }
    if rel == "userdata" {
        return None;
    }
    resolve_sample_file_in_root(root, &rel)
}

fn resolve_sample_file_in_root(root: &PathBuf, rel: &str) -> Option<String> {
    if fs::symlink_metadata(root).ok()?.file_type().is_symlink() {
        return None;
    }
    reject_symlink_components(root, rel).ok()?;
    let target = root.join(rel);
    let canon_root = fs::canonicalize(root).ok()?;
    let canon_target = fs::canonicalize(&target).ok()?;
    if !canon_target.starts_with(&canon_root) || !canon_target.is_file() {
        return None;
    }
    let ext = canon_target
        .extension()
        .map(|x| x.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if ext != "wav" {
        return None;
    }
    canon_target.to_str().map(|s| s.to_string())
}

fn reject_symlink_components(root: &Path, rel: &str) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for component in rel.split('/').filter(|part| !part.is_empty()) {
        current.push(component);
        if fs::symlink_metadata(&current)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(format!(
                "sample tree contains a symlink: {}",
                current.display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "samples_tests.rs"]
mod tests;
