use super::recovery::{
    broken_files, factory_documents, loadable_patch_backup, restore_factory_files,
    restore_patch_backup, PATCH_FILE, SYSTEM_FILE,
};
use super::{choice_lines, step, Choice};
use crate::pi_boot_config::read_composed_runtime_config;
use std::path::PathBuf;

fn store(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "octessera-store-fault-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let full =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    crate::pi_store_test_support::write_pair(&dir, &full);
    dir
}

fn names(dir: &PathBuf) -> Vec<String> {
    let mut names = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[test]
fn a_missing_system_file_is_restored_and_the_patch_is_left_alone() {
    let dir = store("missing-system");
    let factory = factory_documents().unwrap();
    std::fs::remove_file(dir.join(SYSTEM_FILE)).unwrap();
    let patch_before = std::fs::read(dir.join(PATCH_FILE)).unwrap();
    assert!(read_composed_runtime_config(&dir).is_err());

    let replaced = restore_factory_files(&dir, &factory, "2026-11-19-120000").unwrap();

    assert_eq!(replaced, [SYSTEM_FILE]);
    assert_eq!(std::fs::read(dir.join(PATCH_FILE)).unwrap(), patch_before);
    assert_eq!(names(&dir), [PATCH_FILE, SYSTEM_FILE]);
    assert!(read_composed_runtime_config(&dir).is_ok());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_damaged_patch_is_kept_beside_its_factory_replacement() {
    let dir = store("damaged-patch");
    let factory = factory_documents().unwrap();
    std::fs::write(dir.join(PATCH_FILE), b"{ not json").unwrap();
    let system_before = std::fs::read(dir.join(SYSTEM_FILE)).unwrap();

    let replaced = restore_factory_files(&dir, &factory, "2026-11-19-120000").unwrap();

    assert_eq!(replaced, [PATCH_FILE]);
    assert_eq!(std::fs::read(dir.join(SYSTEM_FILE)).unwrap(), system_before);
    let kept = format!("{PATCH_FILE}.broken-2026-11-19-120000");
    assert_eq!(std::fs::read(dir.join(&kept)).unwrap(), b"{ not json");
    assert!(read_composed_runtime_config(&dir).is_ok());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_system_file_with_an_unknown_setting_counts_as_broken() {
    let dir = store("unknown-key");
    let factory = factory_documents().unwrap();
    let mut system: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join(SYSTEM_FILE)).unwrap()).unwrap();
    system
        .as_object_mut()
        .unwrap()
        .insert("fromTheFuture".into(), serde_json::json!(true));
    std::fs::write(dir.join(SYSTEM_FILE), serde_json::to_vec(&system).unwrap()).unwrap();

    assert_eq!(broken_files(&dir, &factory), [SYSTEM_FILE]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn two_unreadable_files_are_both_restored() {
    let dir = store("both");
    let factory = factory_documents().unwrap();
    std::fs::write(dir.join(SYSTEM_FILE), b"").unwrap();
    std::fs::write(dir.join(PATCH_FILE), b"").unwrap();

    assert_eq!(broken_files(&dir, &factory), [SYSTEM_FILE, PATCH_FILE]);
    restore_factory_files(&dir, &factory, "stamp").unwrap();
    restore_factory_files(&dir, &factory, "stamp").unwrap();

    assert!(read_composed_runtime_config(&dir).is_ok());
    assert!(names(&dir).contains(&format!("{SYSTEM_FILE}.broken-stamp")));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_restore_screen_fits_the_oled_and_marks_the_choice() {
    let with_backup = [Choice::LastBackup, Choice::Factory, Choice::ShutDown];
    let without_backup = [Choice::Factory, Choice::ShutDown];
    for choices in [&with_backup[..], &without_backup[..]] {
        for choice in choices {
            for note in [None, Some("Restore failed.")] {
                let lines = choice_lines(choices, *choice, note);
                assert!(lines.len() <= 7, "{lines:?}");
                assert!(lines.iter().all(|line| line.chars().count() <= 19));
            }
        }
    }
    let lines = choice_lines(&with_backup, Choice::ShutDown, None);
    assert!(lines.contains(&"  Last backup".to_string()));
    assert!(lines.contains(&"> Shut down".to_string()));
    assert_eq!(step(&with_backup, Choice::ShutDown, 1), Choice::LastBackup);
    assert_eq!(step(&with_backup, Choice::LastBackup, -1), Choice::ShutDown);
}

fn write_backup(dir: &std::path::Path, stamp: u64, document: &serde_json::Value) {
    let backups = dir.join("backups");
    std::fs::create_dir_all(&backups).unwrap();
    std::fs::write(
        backups.join(format!("bak-{stamp}.json")),
        serde_json::to_vec(document).unwrap(),
    )
    .unwrap();
}

#[test]
fn the_newest_patch_backup_that_loads_replaces_a_broken_patch_and_the_broken_one_is_kept() {
    let dir = store("backup-restore");
    let factory = factory_documents().unwrap();
    let good: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join(PATCH_FILE)).unwrap()).unwrap();
    let mut from_newer = good.clone();
    from_newer["runtimeConfig"]["fromANewerVersion"] = serde_json::json!(true);
    write_backup(&dir, 1_000, &good);
    write_backup(&dir, 2_000, &from_newer);
    std::fs::write(dir.join(PATCH_FILE), b"{ torn").unwrap();

    let backup = loadable_patch_backup(&dir, &factory).unwrap();
    assert_eq!(backup, good);
    restore_patch_backup(&dir, &backup, "stamp").unwrap();

    assert!(read_composed_runtime_config(&dir).is_ok());
    let kept = format!("{PATCH_FILE}.broken-stamp");
    assert_eq!(std::fs::read(dir.join(kept)).unwrap(), b"{ torn");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn no_backup_is_offered_when_the_system_file_is_the_broken_one() {
    let dir = store("backup-system-broken");
    let factory = factory_documents().unwrap();
    let good: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join(PATCH_FILE)).unwrap()).unwrap();
    write_backup(&dir, 1_000, &good);
    std::fs::write(dir.join(SYSTEM_FILE), b"").unwrap();

    assert!(loadable_patch_backup(&dir, &factory).is_none());
    let _ = std::fs::remove_dir_all(dir);
}
