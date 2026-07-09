use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_json_write_overwrites_existing_file() {
        let dir = std::env::temp_dir().join(format!(
            "octessera-pi-atomic-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("default.json");
        std::fs::write(&path, "{\"old\":true}").unwrap();

        atomic_write_json(&path, &serde_json::json!({ "new": true })).unwrap();

        let saved = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
            serde_json::json!({ "new": true })
        );
        assert!(std::fs::read_dir(&dir).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".tmp-")));
        let _ = std::fs::remove_dir_all(dir);
    }
}
