use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const REQUEST_ID: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn oled_failure_publishes_fatal_before_failed_and_keeps_the_lock() {
    let (path, guard) = native_guard("failure-order");
    inject_atomic_failure(AtomicFailure::StatusWrite);

    let error = guard
        .mark_unavailable_and_failed(StartupFatalCode::OledUnavailable)
        .unwrap_err();

    assert!(error.contains("failed status publication failed"));
    assert_eq!(
        read_fatal_at(&path).unwrap().unwrap().code,
        StartupFatalCode::OledUnavailable
    );
    let status = read_status_at(&path);
    assert_eq!(status.phase, HandoffPhase::NativeOwned);
    assert_eq!(status.request_id.as_deref(), Some(REQUEST_ID));
    assert_lock_is_held(&path);
    guard.write_failed_status().unwrap();
    assert_eq!(read_status_at(&path).phase, HandoffPhase::Failed);
    drop(guard);
    remove_test_directory(path);
}

#[test]
fn fatal_failure_does_not_prevent_failed_status_attempt() {
    let (path, guard) = native_guard("fatal-failure");
    inject_atomic_failure(AtomicFailure::FatalWrite);

    let error = guard
        .mark_unavailable_and_failed(StartupFatalCode::OledUnavailable)
        .unwrap_err();

    assert!(error.contains("fatal publication failed"));
    assert!(read_fatal_at(&path).unwrap().is_none());
    let status = read_status_at(&path);
    assert_eq!(status.phase, HandoffPhase::Failed);
    assert_eq!(status.boot_id, current_boot_id().unwrap());
    assert_eq!(status.request_id.as_deref(), Some(REQUEST_ID));
    assert_lock_is_held(&path);
    drop(guard);
    remove_test_directory(path);
}

#[test]
fn fatal_and_failed_status_errors_are_both_reported() {
    let (path, guard) = native_guard("both-failures");
    inject_atomic_failure(AtomicFailure::FatalAndStatusWrite);

    let error = guard
        .mark_unavailable_and_failed(StartupFatalCode::OledUnavailable)
        .unwrap_err();

    assert!(error.contains("fatal publication failed"));
    assert!(error.contains("failed status publication failed"));
    assert_eq!(read_status_at(&path).phase, HandoffPhase::NativeOwned);
    assert_lock_is_held(&path);
    drop(guard);
    remove_test_directory(path);
}

#[test]
fn physical_failure_path_uses_oled_unavailable() {
    let (path, guard) = native_guard("physical-failure");

    guard.mark_failed_result().unwrap();

    assert_eq!(
        read_fatal_at(&path).unwrap().unwrap().code,
        StartupFatalCode::OledUnavailable
    );
    assert_eq!(read_status_at(&path).phase, HandoffPhase::Failed);
    drop(guard);
    remove_test_directory(path);
}

#[test]
fn handoff_status_failure_uses_generic_startup_failed() {
    let (path, mut guard) = native_guard("status-failure");
    inject_atomic_failure(AtomicFailure::StatusWrite);

    assert!(guard.mark_first_menu_rendered().is_err());
    guard.mark_failed_result().unwrap();

    assert_eq!(
        read_fatal_at(&path).unwrap().unwrap().code,
        StartupFatalCode::StartupFailed
    );
    assert_eq!(read_status_at(&path).phase, HandoffPhase::Failed);
    drop(guard);
    remove_test_directory(path);
}

#[test]
fn failed_native_status_is_attachable_with_the_same_ids_and_clears_fatal_after_attach() {
    let (path, guard) = native_guard("failed-recovery");
    guard
        .mark_unavailable_and_failed(StartupFatalCode::OledUnavailable)
        .unwrap();
    let failed = read_status_at(&path);
    let stop = read_stop(&HandoffDirectory::open_existing_at(&path).unwrap())
        .unwrap()
        .unwrap();
    drop(guard);

    let recovered = native_attach_after_startup_clear_at(&path).unwrap();
    let native_owned = read_status_at(&path);
    let recovered_stop = read_stop(&HandoffDirectory::open_existing_at(&path).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(failed.phase, HandoffPhase::Failed);
    assert_eq!(native_owned.phase, HandoffPhase::NativeOwned);
    assert_eq!(native_owned.boot_id, failed.boot_id);
    assert_eq!(native_owned.cycle_count, failed.cycle_count);
    assert_eq!(native_owned.request_id, failed.request_id);
    assert_eq!(recovered_stop.boot_id, stop.boot_id);
    assert_eq!(recovered_stop.request_id, stop.request_id);
    assert!(read_fatal_at(&path).unwrap().is_none());
    drop(recovered);
    remove_test_directory(path);
}

#[test]
fn fatal_is_not_cleared_when_native_attach_fails() {
    let path = test_directory("attach-failure-keeps-fatal");
    let directory = HandoffDirectory::open_existing_at(&path).unwrap();
    write_fatal(
        &directory,
        &StartupFatal::new(
            directory.identity.boot_id.clone(),
            StartupFatalCode::OledUnavailable,
        ),
    )
    .unwrap();

    assert!(native_attach_after_startup_clear_at(&path).is_err());
    assert_eq!(
        read_fatal_at(&path).unwrap().unwrap().code,
        StartupFatalCode::OledUnavailable
    );
    remove_test_directory(path);
}

fn native_guard(label: &str) -> (PathBuf, NativeOledGuard) {
    let path = test_directory(label);
    let guard = super::native_guard_for_test(&path).unwrap();
    (path, guard)
}

fn read_fatal_at(path: &Path) -> Result<Option<StartupFatal>, String> {
    read_fatal(&HandoffDirectory::open_existing_at(path)?)
}

fn read_status_at(path: &Path) -> HandoffStatus {
    read_status(&HandoffDirectory::open_existing_at(path).unwrap())
        .unwrap()
        .unwrap()
}

fn assert_lock_is_held(path: &Path) {
    let directory = HandoffDirectory::open_existing_at(path).unwrap();
    let lock = open_lock(&directory, false).unwrap();
    assert!(flock(&lock, true).is_err());
}

fn test_directory(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "octessera-oled-orange-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    set_mode(&path, DIRECTORY_MODE);
    path
}

fn set_mode(path: &Path, mode: u32) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions).unwrap();
}

fn remove_test_directory(path: PathBuf) {
    fs::remove_dir_all(path).unwrap();
}
