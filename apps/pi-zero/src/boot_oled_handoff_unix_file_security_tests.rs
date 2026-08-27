use super::super::*;
use super::test_directory;
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[test]
fn startup_fatal_replacement_is_atomic_and_leaves_no_temporary_file() {
    let path = test_directory("fatal-replacement");
    let directory = HandoffDirectory::open_existing_at(&path).unwrap();
    let boot_id = directory.identity.boot_id.clone();
    write_fatal(
        &directory,
        &StartupFatal::new(boot_id.clone(), StartupFatalCode::TrellisUnavailable),
    )
    .unwrap();
    write_fatal(
        &directory,
        &StartupFatal::new(boot_id, StartupFatalCode::NeokeyUnavailable),
    )
    .unwrap();
    assert_eq!(
        read_fatal(&directory).unwrap().unwrap().code,
        StartupFatalCode::NeokeyUnavailable
    );
    assert_no_temporary_files(&path);
    let _ = fs::remove_dir_all(path);
}

#[test]
fn owned_oversized_startup_fatal_is_replaced() {
    let path = test_directory("fatal-oversized-replace");
    let directory = HandoffDirectory::open_existing_at(&path).unwrap();
    let fatal_path = path.join(FATAL_NAME);
    fs::write(&fatal_path, vec![b'x'; MAX_FATAL_BYTES + 1]).unwrap();
    set_mode(&fatal_path, FATAL_MODE);

    publish_fatal_at(&path, StartupFatalCode::OledUnavailable).unwrap();

    assert_eq!(
        read_fatal(&directory).unwrap().unwrap().code,
        StartupFatalCode::OledUnavailable
    );
    let _ = fs::remove_dir_all(path);
}

#[test]
fn startup_fatal_rejects_malformed_and_wrong_boot_payloads() {
    let path = test_directory("fatal-invalid");
    let directory = HandoffDirectory::open_existing_at(&path).unwrap();
    let fatal_path = path.join(FATAL_NAME);
    fs::write(&fatal_path, b"not json").unwrap();
    set_mode(&fatal_path, FATAL_MODE);
    assert!(read_fatal(&directory).is_err());
    fs::write(&fatal_path, vec![b'x'; MAX_FATAL_BYTES + 1]).unwrap();
    set_mode(&fatal_path, FATAL_MODE);
    assert!(read_fatal(&directory).is_err());

    fs::write(
        &fatal_path,
        serde_json::to_vec(&serde_json::json!({
            "schema": 1,
            "bootId": directory.identity.boot_id.clone(),
            "code": "free_text",
        }))
        .unwrap(),
    )
    .unwrap();
    set_mode(&fatal_path, FATAL_MODE);
    assert!(read_fatal(&directory).is_err());

    fs::write(
        &fatal_path,
        serde_json::to_vec(&serde_json::json!({
            "schema": 1,
            "bootId": directory.identity.boot_id.clone(),
            "code": "startup_failed",
            "extra": false,
        }))
        .unwrap(),
    )
    .unwrap();
    set_mode(&fatal_path, FATAL_MODE);
    assert!(read_fatal(&directory).is_err());

    fs::write(
        &fatal_path,
        serde_json::to_vec(&serde_json::json!({
            "schema": 1,
            "bootId": "01234567-89ab-cdef-0123-456789abcdef",
            "code": "startup_failed",
        }))
        .unwrap(),
    )
    .unwrap();
    set_mode(&fatal_path, FATAL_MODE);
    if directory.identity.boot_id == "01234567-89ab-cdef-0123-456789abcdef" {
        panic!("test boot ID unexpectedly matched the kernel boot ID");
    }
    assert!(read_fatal(&directory).is_err());
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
fn startup_fatal_symlink_and_hardlink_entries_are_not_accepted() {
    let path = test_directory("fatal-unsafe");
    let directory = HandoffDirectory::open_runtime_at(&path).unwrap();
    let target = path.with_extension("fatal-target");
    let payload = serde_json::to_vec(&serde_json::json!({
        "schema": 1,
        "bootId": directory.identity.boot_id.clone(),
        "code": "startup_failed",
    }))
    .unwrap();
    fs::write(&target, payload).unwrap();
    set_mode(&target, FATAL_MODE);
    std::os::unix::fs::symlink(&target, path.join(FATAL_NAME)).unwrap();
    assert!(read_fatal(&directory).is_err());
    assert!(publish_fatal_at(&path, StartupFatalCode::StartupFailed).is_err());
    fs::remove_file(path.join(FATAL_NAME)).unwrap();
    fs::hard_link(&target, path.join(FATAL_NAME)).unwrap();
    assert!(read_fatal(&directory).is_err());
    assert!(publish_fatal_at(&path, StartupFatalCode::StartupFailed).is_err());
    let _ = fs::remove_dir_all(path);
    let _ = fs::remove_file(target);
}

#[test]
fn startup_fatal_wrong_mode_and_owner_are_rejected_without_reading_content() {
    let path = test_directory("fatal-metadata");
    let directory = HandoffDirectory::open_existing_at(&path).unwrap();
    let fatal_path = path.join(FATAL_NAME);
    fs::write(&fatal_path, vec![b'x'; MAX_FATAL_BYTES + 1]).unwrap();
    set_mode(&fatal_path, FATAL_MODE);

    let wrong_uid = RuntimeIdentity {
        uid: if directory.identity.uid == u32::MAX {
            0
        } else {
            directory.identity.uid + 1
        },
        gid: directory.identity.gid,
        boot_id: directory.identity.boot_id.clone(),
    };
    let wrong_uid_directory = HandoffDirectory {
        file: directory.file.try_clone().unwrap(),
        identity: wrong_uid,
    };
    assert!(write_fatal(
        &wrong_uid_directory,
        &StartupFatal::new(
            directory.identity.boot_id.clone(),
            StartupFatalCode::StartupFailed
        ),
    )
    .is_err());
    assert!(clear_fatal(&wrong_uid_directory).is_err());

    let wrong_gid = RuntimeIdentity {
        uid: directory.identity.uid,
        gid: if directory.identity.gid == u32::MAX {
            0
        } else {
            directory.identity.gid + 1
        },
        boot_id: directory.identity.boot_id.clone(),
    };
    let wrong_gid_directory = HandoffDirectory {
        file: directory.file.try_clone().unwrap(),
        identity: wrong_gid,
    };
    assert!(clear_fatal(&wrong_gid_directory).is_err());

    set_mode(&fatal_path, 0o640);
    assert!(write_fatal(
        &directory,
        &StartupFatal::new(
            directory.identity.boot_id.clone(),
            StartupFatalCode::StartupFailed
        ),
    )
    .is_err());
    assert!(clear_fatal(&directory).is_err());
    let _ = fs::remove_dir_all(path);
}

#[test]
fn startup_fatal_is_cleared_before_native_attach() {
    let path = test_directory("fatal-clear-before-attach");
    let mut animator = animator_start_at(&path).unwrap();
    let directory = HandoffDirectory::open_existing_at(&path).unwrap();
    let fatal_path = path.join(FATAL_NAME);
    fs::write(&fatal_path, vec![b'x'; MAX_FATAL_BYTES + 1]).unwrap();
    set_mode(&fatal_path, FATAL_MODE);
    let status = read_status(&animator.directory).unwrap().unwrap();
    create_or_attach_stop(&animator.directory, &status).unwrap();
    assert!(animator.stop_requested().unwrap());
    animator.release().unwrap();
    let native = native_attach_after_startup_clear_at(&path).unwrap();
    assert!(read_fatal(&directory).unwrap().is_none());
    drop(native);
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

        let fatal = StartupFatal::new(
            directory.identity.boot_id.clone(),
            StartupFatalCode::StartupFailed,
        );
        inject_atomic_failure(failure);
        assert!(write_fatal(&directory, &fatal).is_err());
        assert_no_temporary_files(&path);
        let _ = fs::remove_dir_all(path);
    }
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
                .starts_with(".stop.request.tmp-")
            && !entry
                .file_name()
                .to_string_lossy()
                .starts_with(".fatal.json.tmp-")));
}

fn set_mode(path: &std::path::Path, mode: u32) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions).unwrap();
}
