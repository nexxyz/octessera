use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const KEPT_BACKUPS: usize = 20;

pub fn read_json_file(path: &Path) -> Result<Option<serde_json::Value>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|error| error.to_string())
}

pub fn write_json_atomically(path: &Path, payload: &serde_json::Value) -> Result<(), String> {
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
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
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

pub fn write_rotated_backup(dir: &Path, payload: &serde_json::Value) -> Result<(), String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    write_rotated_backup_at(dir, payload, millis)
}

/// A backup name already taken in this millisecond moves to the next free one, so names stay
/// unique and keep sorting oldest first.
fn write_rotated_backup_at(
    dir: &Path,
    payload: &serde_json::Value,
    mut millis: u128,
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    while dir.join(format!("bak-{millis}.json")).exists() {
        millis += 1;
    }
    write_json_atomically(&dir.join(format!("bak-{millis}.json")), payload)?;
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with("bak-") && name.ends_with(".json") {
            paths.push(path);
        }
    }
    paths.sort();
    for path in paths.iter().take(paths.len().saturating_sub(KEPT_BACKUPS)) {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
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

#[cfg(test)]
mod tests;
