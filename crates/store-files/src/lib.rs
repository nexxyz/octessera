use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
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

/// Renames `dir/name` to `dir/<name>.<label>-<stamp>` (a numeric suffix keeps it from replacing an
/// earlier one); a missing file is left alone.
pub fn keep_file_aside(dir: &Path, name: &str, label: &str, stamp: &str) -> Result<(), String> {
    let path = dir.join(name);
    if !path.exists() {
        return Ok(());
    }
    let kept_name = format!("{name}.{label}-{stamp}");
    let mut kept = dir.join(&kept_name);
    let mut suffix = 1;
    while kept.exists() {
        kept = dir.join(format!("{kept_name}-{suffix}"));
        suffix += 1;
    }
    fs::rename(&path, &kept).map_err(|error| format!("could not keep {name}: {error}"))
}

pub fn write_json_atomically(path: &Path, payload: &serde_json::Value) -> Result<(), String> {
    let content = serde_json::to_vec_pretty(payload).map_err(|e| e.to_string())?;
    write_atomically(path, |file| file.write_all(&content)).map_err(|error| error.to_string())
}

/// Writes through a temp file in the same directory, syncs it, renames it over `path` and syncs
/// the directory. The error keeps its kind and names the path.
pub fn write_atomically(
    path: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    write_atomically_inner(path, write)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))
}

fn write_atomically_inner(
    path: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let tmp = path.with_file_name(format!(
        ".{}.tmp-{}-{nanos}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file"),
        std::process::id(),
    ));
    let result = (|| {
        let mut file = File::create(&tmp)?;
        write(&mut file)?;
        file.sync_all()?;
        drop(file);
        replace_file(&tmp, path)?;
        if let Some(parent) = path.parent() {
            let _ = File::open(parent).and_then(|dir| dir.sync_all());
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

/// Each backup is named after the later of the clock and the newest backup plus one, so a clock
/// that went back after a reboot still names new backups newest and pruning keeps them.
fn write_rotated_backup_at(
    dir: &Path,
    payload: &serde_json::Value,
    millis: u128,
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let mut backups = Vec::new();
    for entry in fs::read_dir(dir).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if let Some(stamp) = backup_stamp(&path) {
            backups.push((stamp, path));
        }
    }
    let stamp = backups
        .iter()
        .map(|(stamp, _)| stamp + 1)
        .max()
        .map_or(millis, |next| next.max(millis));
    let path = dir.join(format!("bak-{stamp}.json"));
    write_json_atomically(&path, payload)?;
    backups.push((stamp, path));
    backups.sort();
    for (_, path) in backups
        .iter()
        .take(backups.len().saturating_sub(KEPT_BACKUPS))
    {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// The rotated backups in `dir`, newest first, in the same order pruning uses.
pub fn rotated_backups_newest_first(dir: &Path) -> Vec<PathBuf> {
    let mut backups: Vec<(u128, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            backup_stamp(&path).map(|stamp| (stamp, path))
        })
        .collect();
    backups.sort_by_key(|(stamp, _)| std::cmp::Reverse(*stamp));
    backups.into_iter().map(|(_, path)| path).collect()
}

fn backup_stamp(path: &Path) -> Option<u128> {
    path.file_name()?
        .to_str()?
        .strip_prefix("bak-")?
        .strip_suffix(".json")?
        .parse()
        .ok()
}

#[cfg(not(windows))]
fn replace_file(tmp: &Path, path: &Path) -> io::Result<()> {
    fs::rename(tmp, path)
}

#[cfg(windows)]
fn replace_file(tmp: &Path, path: &Path) -> io::Result<()> {
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
        Err(io::Error::last_os_error())
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
