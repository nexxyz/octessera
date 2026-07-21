use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) fn valid_preset_name(name: &str) -> bool {
    playback_runtime::is_valid_preset_name(name)
}

pub(crate) fn preset_file_path(presets_dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !valid_preset_name(name) {
        return Err(format!("Unsafe preset name: {name:?}"));
    }
    Ok(presets_dir.join(format!("{name}.json")))
}

pub(crate) fn preset_patch_file_path(presets_dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !valid_preset_name(name) {
        return Err(format!("Unsafe preset name: {name:?}"));
    }
    Ok(presets_dir.join("patches").join(format!("{name}.json")))
}

pub(crate) fn preset_load_file_path(presets_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let patch = preset_patch_file_path(presets_dir, name)?;
    if patch.is_file() {
        return Ok(patch);
    }
    preset_file_path(presets_dir, name)
}

pub(crate) fn preset_name_from_file_name(file_name: &str) -> Option<String> {
    let name = file_name.strip_suffix(".json")?;
    valid_preset_name(name).then(|| name.to_string())
}

pub(crate) fn atomic_write_json(path: &Path, payload: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_vec_pretty(payload).map_err(|e| e.to_string())?;
    let tmp = path.with_file_name(format!(
        ".{}.tmp-{}-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("json"),
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    let result = (|| {
        let mut file = File::create(&tmp).map_err(|e| e.to_string())?;
        file.write_all(&content).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        replace_file(&tmp, path)?;
        if let Some(parent) = path.parent() {
            let _ = OpenOptions::new()
                .read(true)
                .open(parent)
                .and_then(|dir| dir.sync_all());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(not(windows))]
fn replace_file(tmp: &Path, path: &Path) -> Result<(), String> {
    fs::rename(tmp, path).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn replace_file(tmp: &Path, path: &Path) -> Result<(), String> {
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    let tmp = wide_path(tmp.as_os_str());
    let path = wide_path(path.as_os_str());
    let ok = unsafe {
        MoveFileExW(
            tmp.as_ptr(),
            path.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn wide_path(path: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.encode_wide().chain([0]).collect()
}
