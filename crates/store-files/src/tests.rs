use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn temp_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "octessera-store-files-{prefix}-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn file_names(dir: &Path) -> Vec<String> {
    let mut names = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[test]
fn atomic_json_write_overwrites_existing_file_without_leaving_temp_files() {
    let dir = temp_dir("atomic");
    let path = dir.join("default.json");
    fs::write(&path, "{\"old\":true}").unwrap();

    write_json_atomically(&path, &serde_json::json!({ "new": true })).unwrap();

    assert_eq!(
        read_json_file(&path).unwrap(),
        Some(serde_json::json!({ "new": true }))
    );
    assert_eq!(file_names(&dir), vec!["default.json".to_string()]);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn atomic_json_write_creates_missing_parent_directories() {
    let dir = temp_dir("parents");
    let path = dir.join("patches").join("Jam.json");

    write_json_atomically(&path, &serde_json::json!({ "patch": true })).unwrap();

    assert_eq!(
        read_json_file(&path).unwrap(),
        Some(serde_json::json!({ "patch": true }))
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn read_json_file_reports_missing_as_none_and_rejects_invalid_json() {
    let dir = temp_dir("read");
    assert_eq!(read_json_file(&dir.join("missing.json")).unwrap(), None);
    assert_eq!(read_json_file(&dir).unwrap(), None);
    fs::write(dir.join("broken.json"), "{").unwrap();
    assert!(read_json_file(&dir.join("broken.json")).is_err());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn rotated_backup_keeps_the_latest_twenty_and_ignores_other_files() {
    let dir = temp_dir("backups");
    for index in 0..KEPT_BACKUPS {
        fs::write(dir.join(format!("bak-{index:03}.json")), "{}").unwrap();
    }
    fs::write(dir.join("notes.txt"), "keep").unwrap();

    write_rotated_backup(&dir, &serde_json::json!({ "latest": true })).unwrap();

    let names = file_names(&dir);
    let backups = names
        .iter()
        .filter(|name| name.starts_with("bak-"))
        .collect::<Vec<_>>();
    assert_eq!(backups.len(), KEPT_BACKUPS);
    assert!(!names.contains(&"bak-000.json".to_string()));
    assert!(names.contains(&"bak-001.json".to_string()));
    assert!(names.contains(&"notes.txt".to_string()));
    let newest = backups.last().unwrap();
    assert_eq!(
        read_json_file(&dir.join(newest)).unwrap(),
        Some(serde_json::json!({ "latest": true }))
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn rotated_backup_creates_the_backup_directory() {
    let dir = temp_dir("backup-dir");
    let backups = dir.join("backups");

    write_rotated_backup(&backups, &serde_json::json!({})).unwrap();

    assert_eq!(file_names(&backups).len(), 1);
    let _ = fs::remove_dir_all(dir);
}
