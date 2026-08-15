use super::files::*;
use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn request_ids_and_boot_ids_are_strict() {
    assert!(valid_request_id("0123456789abcdef0123456789abcdef"));
    assert!(!valid_request_id("0123456789ABCDEF0123456789abcdef"));
    assert!(valid_boot_id("01234567-89ab-cdef-0123-456789abcdef"));
    assert!(!valid_boot_id("01234567-89AB-cdef-0123-456789abcdef"));
}

#[test]
fn status_contract_rejects_unknown_keys_and_missing_request_id() {
    let boot_id = "01234567-89ab-cdef-0123-456789abcdef";
    let missing = serde_json::json!({
        "schema": 1, "phase": "released", "bootId": boot_id, "pid": 1, "cycleCount": 2,
    });
    assert!(files::parse_status_for_test(&missing).is_err());
    let unknown = serde_json::json!({
        "schema": 1, "phase": "animating", "bootId": boot_id, "pid": 1, "cycleCount": 2, "extra": false,
    });
    assert!(files::parse_status_for_test(&unknown).is_err());
    let failed_without_request = serde_json::json!({
        "schema": 1, "phase": "failed", "bootId": boot_id, "pid": 1, "cycleCount": 2,
    });
    assert!(files::parse_status_for_test(&failed_without_request).is_err());
    let zero_pid = serde_json::json!({
        "schema": 1, "phase": "animating", "bootId": boot_id, "pid": 0, "cycleCount": 2,
    });
    assert!(files::parse_status_for_test(&zero_pid).is_err());
}

#[test]
fn animator_and_native_restart_share_one_stable_lock() {
    let path = test_directory("handoff");
    let mut animator = animator_start_at(&path).unwrap();
    let lock = open_lock(&animator.directory, false).unwrap();
    assert!(flock(&lock, true).is_err());
    let (ready_tx, ready_rx) = mpsc::channel();
    let native_path = path.clone();
    let native = std::thread::spawn(move || {
        ready_tx.send(()).unwrap();
        native_attach_at(&native_path).unwrap()
    });
    ready_rx.recv().unwrap();
    std::thread::sleep(Duration::from_millis(20));
    assert!(animator.stop_requested().unwrap());
    animator.release().unwrap();
    let mut guard = native.join().unwrap();
    guard.mark_first_menu_rendered().unwrap();
    drop(guard);
    drop(native_attach_at(&path).unwrap());
    let _ = fs::remove_dir_all(path);
}

#[test]
fn native_reacquire_preserves_first_menu_rendered_status() {
    let path = test_directory("preserving-reacquire");
    let mut animator = animator_start_at(&path).unwrap();
    let status = read_status(&animator.directory).unwrap().unwrap();
    create_or_attach_stop(&animator.directory, &status).unwrap();
    animator.stop_requested().unwrap();
    animator.release().unwrap();
    let mut native = native_attach_at(&path).unwrap();
    native.mark_first_menu_rendered().unwrap();
    let expected = read_status(&native.directory).unwrap().unwrap();
    native.detach_preserving().unwrap();
    native.reacquire_existing().unwrap();
    assert_eq!(read_status(&native.directory).unwrap().unwrap(), expected);
    let _ = fs::remove_dir_all(path);
}

#[test]
fn detached_native_guard_cannot_mark_failed_before_reacquire() {
    let path = test_directory("detached-failure");
    let mut animator = animator_start_at(&path).unwrap();
    let status = read_status(&animator.directory).unwrap().unwrap();
    create_or_attach_stop(&animator.directory, &status).unwrap();
    animator.stop_requested().unwrap();
    animator.release().unwrap();

    let mut native = native_attach_at(&path).unwrap();
    let expected = read_status(&native.directory).unwrap().unwrap();
    native.detach_preserving().unwrap();
    native.mark_failed();
    assert_eq!(read_status(&native.directory).unwrap().unwrap(), expected);
    native.reacquire_existing().unwrap();
    let _ = fs::remove_dir_all(path);
}

#[test]
fn symlink_and_hardlink_entries_are_not_accepted() {
    let path = test_directory("unsafe");
    let request = serde_json::json!({
        "schema": 1,
        "bootId": current_boot_id().unwrap(),
        "pid": 1,
        "requestId": "0123456789abcdef0123456789abcdef",
    });
    fs::write(
        path.join(STATUS_NAME),
        serde_json::to_vec(&request).unwrap(),
    )
    .unwrap();
    let mut permissions = fs::metadata(path.join(STATUS_NAME)).unwrap().permissions();
    permissions.set_mode(STOP_MODE);
    fs::set_permissions(path.join(STATUS_NAME), permissions).unwrap();
    let _ = std::os::unix::fs::symlink(path.join(STATUS_NAME), path.join(STOP_NAME));
    assert!(read_stop(&HandoffDirectory::open_runtime_at(&path).unwrap()).is_err());
    let _ = fs::remove_file(path.join(STOP_NAME));
    fs::hard_link(path.join(STATUS_NAME), path.join(STOP_NAME)).unwrap();
    assert!(read_stop(&HandoffDirectory::open_runtime_at(&path).unwrap()).is_err());
    let _ = fs::remove_dir_all(path);
}

#[test]
fn native_attach_does_not_create_a_missing_handoff_root() {
    let path = std::env::temp_dir().join(format!(
        "octessera-oled-missing-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    assert!(native_attach_at(&path).is_err());
    assert!(!path.exists());
}

#[test]
fn animator_does_not_create_a_missing_systemd_handoff_root() {
    let path = std::env::temp_dir().join(format!(
        "octessera-oled-animator-missing-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    assert!(animator_start_at(&path).is_err());
    assert!(!path.exists());
}

#[test]
fn animator_restart_does_not_clobber_same_boot_terminal_state() {
    let path = test_directory("restart");
    let mut animator = animator_start_at(&path).unwrap();
    let status = read_status(&animator.directory).unwrap().unwrap();
    create_or_attach_stop(&animator.directory, &status).unwrap();
    assert!(animator.stop_requested().unwrap());
    animator.release().unwrap();
    assert!(animator_start_at(&path).is_err());
    assert!(
        read_status(&HandoffDirectory::open_existing_at(&path).unwrap())
            .unwrap()
            .is_some()
    );
    let _ = fs::remove_dir_all(path);
}

#[test]
fn atomic_write_failures_remove_scoped_temporary_files() {
    for failure in [
        AtomicFailure::Write,
        AtomicFailure::Sync,
        AtomicFailure::Rename,
    ] {
        let path = test_directory("atomic-failure");
        let directory = HandoffDirectory::open_existing_at(&path).unwrap();
        inject_atomic_failure(failure);
        let status = HandoffStatus::new(
            HandoffPhase::Animating,
            directory.identity.boot_id.clone(),
            0,
            None,
        );
        assert!(write_status(&directory, &status).is_err());
        assert_no_temporary_files(&path);
        let _ = fs::remove_dir_all(path);
    }
}

#[test]
fn concurrent_stop_publish_has_one_validated_winner() {
    let path = test_directory("stop-race");
    let boot_id = current_boot_id().unwrap();
    let status = HandoffStatus::new(HandoffPhase::Animating, boot_id, 0, None);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let mut threads = Vec::new();
    for _ in 0..2 {
        let path = path.clone();
        let barrier = std::sync::Arc::clone(&barrier);
        let status = status.clone();
        threads.push(std::thread::spawn(move || {
            let directory = HandoffDirectory::open_existing_at(&path).unwrap();
            barrier.wait();
            create_or_attach_stop(&directory, &status)
        }));
    }
    let first = threads.remove(0).join().unwrap().unwrap();
    let second = threads.remove(0).join().unwrap().unwrap();
    assert_eq!(first, second);
    let directory = HandoffDirectory::open_existing_at(&path).unwrap();
    assert_eq!(read_stop(&directory).unwrap().unwrap().request_id, first);
    let _ = fs::remove_dir_all(path);
}

#[test]
fn stop_transition_is_monotonic_at_frame_23() {
    let path = test_directory("monotonic");
    let mut animator = animator_start_at(&path).unwrap();
    for _ in 0..23 {
        animator.publish_cycle().unwrap();
    }
    let initial = read_status(&animator.directory).unwrap().unwrap();
    assert_eq!(initial.cycle_count, 23);
    let mut phases = vec![initial.phase];
    create_or_attach_stop(&animator.directory, &initial).unwrap();
    assert!(animator.stop_requested().unwrap());
    phases.push(read_status(&animator.directory).unwrap().unwrap().phase);
    assert!(animator.publish_cycle().is_err());
    assert_eq!(
        read_status(&animator.directory).unwrap().unwrap().phase,
        HandoffPhase::ReleaseRequested
    );
    animator.release().unwrap();
    phases.push(
        read_status(&HandoffDirectory::open_existing_at(&path).unwrap())
            .unwrap()
            .unwrap()
            .phase,
    );
    let mut native = native_attach_at(&path).unwrap();
    phases.push(read_status(&native.directory).unwrap().unwrap().phase);
    native.mark_first_menu_rendered().unwrap();
    phases.push(read_status(&native.directory).unwrap().unwrap().phase);
    assert_eq!(
        phases,
        [
            HandoffPhase::Animating,
            HandoffPhase::ReleaseRequested,
            HandoffPhase::Released,
            HandoffPhase::NativeOwned,
            HandoffPhase::FirstMenuRendered,
        ]
    );
    drop(native);
    let _ = fs::remove_dir_all(path);
}

#[test]
fn animator_and_native_failures_remain_recoverable_with_matching_stop() {
    for before_request in [true, false] {
        let path = test_directory(if before_request {
            "failed-before"
        } else {
            "failed-after"
        });
        let mut animator = animator_start_at(&path).unwrap();
        if !before_request {
            let status = read_status(&animator.directory).unwrap().unwrap();
            create_or_attach_stop(&animator.directory, &status).unwrap();
            assert!(animator.stop_requested().unwrap());
        }
        animator.mark_failed();
        drop(animator);
        let native = native_attach_at(&path).unwrap();
        drop(native);
        let _ = fs::remove_dir_all(path);
    }

    let path = test_directory("native-failure");
    let mut animator = animator_start_at(&path).unwrap();
    let status = read_status(&animator.directory).unwrap().unwrap();
    create_or_attach_stop(&animator.directory, &status).unwrap();
    assert!(animator.stop_requested().unwrap());
    animator.release().unwrap();
    let native = native_attach_at(&path).unwrap();
    native.mark_failed();
    drop(native);
    drop(native_attach_at(&path).unwrap());
    let _ = fs::remove_dir_all(path);
}

fn assert_no_temporary_files(path: &std::path::Path) {
    assert!(fs::read_dir(path)
        .unwrap()
        .filter_map(Result::ok)
        .all(|entry| !entry
            .file_name()
            .to_string_lossy()
            .starts_with(".status.json.tmp-")
            && !entry
                .file_name()
                .to_string_lossy()
                .starts_with(".stop.request.tmp-")));
}

fn test_directory(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "octessera-oled-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(DIRECTORY_MODE);
    fs::set_permissions(&path, permissions).unwrap();
    path
}
