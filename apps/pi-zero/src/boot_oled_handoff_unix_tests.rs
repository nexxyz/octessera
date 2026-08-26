use super::files::*;
use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

#[path = "boot_oled_handoff_unix_file_security_tests.rs"]
mod file_security_tests;

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
fn startup_fatal_contract_is_exact_and_allowlisted() {
    let path = test_directory("fatal-contract");
    let directory = HandoffDirectory::open_existing_at(&path).unwrap();
    let boot_id = directory.identity.boot_id.clone();
    for code in [
        StartupFatalCode::TrellisUnavailable,
        StartupFatalCode::NeokeyUnavailable,
        StartupFatalCode::ControlsUnavailable,
        StartupFatalCode::AudioUnavailable,
        StartupFatalCode::OledUnavailable,
        StartupFatalCode::StartupFailed,
    ] {
        publish_fatal_at(&path, code).unwrap();
        let bytes = fs::read(path.join(FATAL_NAME)).unwrap();
        assert!(bytes.len() <= MAX_FATAL_BYTES);
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "schema": 1,
                "bootId": boot_id.clone(),
                "code": code.as_str(),
            })
        );
        assert_eq!(read_fatal(&directory).unwrap().unwrap().code, code);
    }
    assert_eq!(
        fs::metadata(path.join(FATAL_NAME))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        FATAL_MODE
    );
    let _ = fs::remove_dir_all(path);
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
    assert!(native.mark_failed_result().is_err());
    assert_eq!(read_status(&native.directory).unwrap().unwrap(), expected);
    native.reacquire_existing().unwrap();
    let _ = fs::remove_dir_all(path);
}

#[test]
fn failed_status_recovers_through_release_and_native_ownership_with_matching_ids() {
    let path = test_directory("failed-recovery-sequence");
    let mut animator = animator_start_at(&path).unwrap();
    animator.mark_failed();
    let failed = read_status(&animator.directory).unwrap().unwrap();
    let request_id = failed.request_id.clone();
    assert_eq!(failed.phase, HandoffPhase::Failed);

    assert!(animator.stop_requested().unwrap());
    assert_eq!(
        read_status(&animator.directory).unwrap().unwrap().phase,
        HandoffPhase::ReleaseRequested
    );
    animator.release().unwrap();
    let released = read_status(&HandoffDirectory::open_existing_at(&path).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(released.phase, HandoffPhase::Released);

    let native = native_attach_at(&path).unwrap();
    let native_owned = read_status(&native.directory).unwrap().unwrap();
    assert_eq!(native_owned.phase, HandoffPhase::NativeOwned);
    assert_eq!(native_owned.boot_id, failed.boot_id);
    assert_eq!(native_owned.request_id, request_id);
    drop(native);
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
    native.mark_failed_result().unwrap();
    drop(native);
    drop(native_attach_at(&path).unwrap());
    let _ = fs::remove_dir_all(path);
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
