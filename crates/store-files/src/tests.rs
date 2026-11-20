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

#[test]
fn backups_in_the_same_millisecond_keep_distinct_sortable_names() {
    let dir = temp_dir("same-millis");

    write_rotated_backup_at(
        &dir,
        &serde_json::json!({ "first": true }),
        1_700_000_000_000,
    )
    .unwrap();
    write_rotated_backup_at(
        &dir,
        &serde_json::json!({ "second": true }),
        1_700_000_000_000,
    )
    .unwrap();

    let names = file_names(&dir);
    assert_eq!(names, ["bak-1700000000000.json", "bak-1700000000001.json"]);
    assert_eq!(
        read_json_file(&dir.join(&names[1])).unwrap(),
        Some(serde_json::json!({ "second": true }))
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_clock_that_went_back_still_names_the_new_backup_newest_and_keeps_it() {
    let dir = temp_dir("clock-back");
    for index in 0..KEPT_BACKUPS {
        let millis = 1_700_000_000_000 + index as u128 * 1_000;
        write_rotated_backup_at(&dir, &serde_json::json!({ "old": index }), millis).unwrap();
    }

    write_rotated_backup_at(
        &dir,
        &serde_json::json!({ "after reboot": true }),
        1_600_000_000_000,
    )
    .unwrap();

    let names = file_names(&dir);
    assert_eq!(names.len(), KEPT_BACKUPS);
    assert!(!names.contains(&"bak-1700000000000.json".to_string()));
    let newest = format!("bak-{}.json", 1_700_000_000_000_u128 + 19_000 + 1);
    assert_eq!(names.last(), Some(&newest));
    assert_eq!(
        read_json_file(&dir.join(&newest)).unwrap(),
        Some(serde_json::json!({ "after reboot": true }))
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn backups_are_listed_newest_first_by_their_number_not_their_text() {
    let dir = temp_dir("newest-first");
    for stamp in ["999", "1000", "20"] {
        fs::write(dir.join(format!("bak-{stamp}.json")), "{}").unwrap();
    }
    fs::write(dir.join("bak-notes.json"), "{}").unwrap();

    let names: Vec<String> = rotated_backups_newest_first(&dir)
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();

    assert_eq!(names, ["bak-1000.json", "bak-999.json", "bak-20.json"]);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_failed_atomic_write_names_the_path_and_leaves_no_temp_file() {
    let dir = temp_dir("atomic-failure");
    let path = dir.join("data.bin");
    fs::write(&path, b"old").unwrap();

    let error = write_atomically(&path, |_| Err(io::Error::other("boom"))).unwrap_err();

    assert!(error.to_string().contains("data.bin"));
    assert!(error.to_string().contains("boom"));
    assert_eq!(fs::read(&path).unwrap(), b"old");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn kept_aside_file_gets_a_free_name_and_a_missing_file_is_left_alone() {
    let dir = temp_dir("aside");
    keep_file_aside(&dir, "system.json", "pre-reset", "t1").unwrap();
    assert!(file_names(&dir).is_empty());

    fs::write(dir.join("system.json"), "one").unwrap();
    keep_file_aside(&dir, "system.json", "pre-reset", "t1").unwrap();
    fs::write(dir.join("system.json"), "two").unwrap();
    keep_file_aside(&dir, "system.json", "pre-reset", "t1").unwrap();

    assert_eq!(
        file_names(&dir),
        vec!["system.json.pre-reset-t1", "system.json.pre-reset-t1-1"]
    );
    assert_eq!(
        fs::read_to_string(dir.join("system.json.pre-reset-t1")).unwrap(),
        "one"
    );
}

#[test]
fn copied_aside_file_keeps_the_original_in_place() {
    let dir = temp_dir("copy-aside");
    copy_file_aside(&dir, "system.json", "pre-reset", "t1").unwrap();
    assert!(file_names(&dir).is_empty());

    fs::write(dir.join("system.json"), "one").unwrap();
    copy_file_aside(&dir, "system.json", "pre-reset", "t1").unwrap();
    copy_file_aside(&dir, "system.json", "pre-reset", "t1").unwrap();

    assert_eq!(
        file_names(&dir),
        vec![
            "system.json",
            "system.json.pre-reset-t1",
            "system.json.pre-reset-t1-1"
        ]
    );
    assert_eq!(fs::read_to_string(dir.join("system.json")).unwrap(), "one");
    assert_eq!(
        fs::read_to_string(dir.join("system.json.pre-reset-t1")).unwrap(),
        "one"
    );
}
