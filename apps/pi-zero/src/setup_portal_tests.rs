use super::*;
use crate::setup_portal_files::{
    create_request_marker_with_publisher_for_test, validate_public_metadata, SetupFileError,
    SetupFileKind, SetupMetadata, SetupPortalPaths,
};
use playback_runtime::{RuntimePlatformEffect, RuntimeSetupPortalPhase, RuntimeStoreResult};
use serde_json::{json, Value};
use std::fs;
#[cfg(any(unix, windows))]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

const BOOT: &str = "01234567-89ab-cdef-0123-456789abcdef";
const ATTEMPT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const NEW_ATTEMPT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct TestDirectory {
    root: PathBuf,
    paths: SetupPortalPaths,
}

impl TestDirectory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "octessera-setup-portal-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let public = root.join("public");
        Self {
            paths: SetupPortalPaths {
                request: root.join("request").join("setup-portal.request"),
                receipts: public.join("receipts"),
                current: public.join("current.json"),
                public,
                boot_id: root.join("boot-id"),
            },
            root,
        }
    }

    #[cfg(any(unix, windows))]
    fn prepare(&self) {
        fs::create_dir_all(self.paths.request.parent().unwrap()).unwrap();
        fs::create_dir_all(&self.paths.receipts).unwrap();
        fs::set_permissions(self.paths.public.clone(), permissions(0o750)).unwrap();
        fs::set_permissions(self.paths.receipts.clone(), permissions(0o750)).unwrap();
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(any(unix, windows))]
fn permissions(mode: u32) -> fs::Permissions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::Permissions::from_mode(mode)
    }
    #[cfg(windows)]
    {
        let _ = mode;
        fs::metadata(".").unwrap().permissions()
    }
}

fn environment(
    directory: &TestDirectory,
    clock: Arc<AtomicU64>,
    random_value: u8,
) -> SetupPortalEnvironment {
    let next_random = Arc::new(AtomicU8::new(random_value));
    SetupPortalEnvironment::test(
        directory.paths.clone(),
        0,
        Arc::new(move || clock.load(Ordering::SeqCst)),
        Arc::new(move |bytes| {
            bytes.fill(next_random.fetch_add(1, Ordering::SeqCst));
            Ok(())
        }),
        Arc::new(|| Ok(BOOT.into())),
    )
}

fn request(id: &str, revision: u64) -> RuntimePlatformRequest {
    RuntimePlatformRequest::new(
        RuntimePlatformEffect::SetupPortalOpen,
        id.into(),
        Some(revision),
    )
}

fn setup_status(phase: RuntimeSetupPortalPhase) -> Value {
    let mut status = json!({
        "type": "setup_portal_status",
        "phase": phase,
        "rebootRequired": false
    });
    if phase == RuntimeSetupPortalPhase::Starting {
        status["disposition"] = json!("accepted");
    }
    if phase == RuntimeSetupPortalPhase::PortalReady {
        status["portalSuffix"] = json!("abcd");
    }
    if phase == RuntimeSetupPortalPhase::Failed {
        status["errorCode"] = json!("operation_failed");
    }
    status
}

#[cfg(any(unix, windows))]
fn write_envelope(path: &Path, attempt: &str, sequence: u64, status: Value) {
    fs::write(
        path,
        serde_json::to_vec(&json!({
            "schema": 1,
            "bootId": BOOT,
            "attemptId": attempt,
            "sequence": sequence,
            "status": status
        }))
        .unwrap(),
    )
    .unwrap();
    fs::set_permissions(path, permissions(0o640)).unwrap();
}

#[cfg(any(unix, windows))]
fn write_receipt(directory: &TestDirectory, token: &str, attempt: &str, sequence: u64) {
    write_envelope(
        &directory.paths.receipts.join(format!("{token}.json")),
        attempt,
        sequence,
        setup_status(RuntimeSetupPortalPhase::Starting),
    );
}

#[cfg(any(unix, windows))]
fn result(message: HostMessage) -> RuntimeStoreResult {
    let HostMessage::RuntimeResult { result } = message else {
        panic!("expected runtime result")
    };
    result
}

#[test]
fn metadata_seam_rejects_wrong_owner_group_mode_and_types() {
    let valid = SetupMetadata::regular(0, 7, 0o640, 10);
    assert!(validate_public_metadata(valid, 0, 7, 0o640, false).is_ok());
    for metadata in [
        SetupMetadata {
            uid: Some(1),
            ..valid
        },
        SetupMetadata {
            gid: Some(8),
            ..valid
        },
        SetupMetadata {
            mode: 0o644,
            ..valid
        },
        SetupMetadata { nlink: 2, ..valid },
        SetupMetadata {
            kind: SetupFileKind::Symlink,
            ..valid
        },
    ] {
        assert!(validate_public_metadata(metadata, 0, 7, 0o640, false).is_err());
    }
    assert!(
        validate_public_metadata(SetupMetadata::directory(0, 7, 0o750), 0, 7, 0o750, true).is_ok()
    );
}

#[test]
fn local_start_failure_is_an_identified_setup_status() {
    let message = start_failure_message(
        &request("local-failure", 6),
        SetupPortalFailure::from_file(SetupFileError::Exists),
    );
    let result = result(message);
    assert!(matches!(
        result,
        RuntimeStoreResult::Identified {
            request_id,
            revision: Some(6),
            result
        } if request_id == "local-failure"
            && matches!(result.as_ref(), RuntimeStoreResult::SetupPortalStatus { status } if status.phase == RuntimeSetupPortalPhase::Failed && status.error_code == Some(playback_runtime::RuntimeSetupPortalErrorCode::OperationFailed))
    ));
}

#[cfg(unix)]
#[test]
fn marker_is_exact_private_and_create_new_rejects_existing_and_symlink() {
    let directory = TestDirectory::new();
    fs::create_dir_all(directory.paths.request.parent().unwrap()).unwrap();
    let token = "0123456789abcdef0123456789abcdef";
    create_request_marker(&directory.paths.request, token).unwrap();
    assert_eq!(
        fs::read(&directory.paths.request).unwrap(),
        format!("{token}\n").as_bytes()
    );
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        fs::metadata(&directory.paths.request)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        create_request_marker(&directory.paths.request, "abcdef0123456789abcdef0123456789"),
        Err(SetupFileError::Exists)
    );
    let _ = fs::remove_file(&directory.paths.request);
    std::os::unix::fs::symlink(directory.root.join("missing"), &directory.paths.request).unwrap();
    assert!(create_request_marker(&directory.paths.request, token).is_err());
}

#[cfg(any(unix, windows))]
#[test]
fn marker_publication_never_exposes_partial_final_content() {
    let directory = TestDirectory::new();
    fs::create_dir_all(directory.paths.request.parent().unwrap()).unwrap();
    let expected = b"0123456789abcdef0123456789abcdef\n".to_vec();
    let observed_partial = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observer_path = directory.paths.request.clone();
    let observer_partial = observed_partial.clone();
    let observer_stop = stop.clone();
    let observer_expected = expected.clone();
    let observer = std::thread::spawn(move || {
        while !observer_stop.load(Ordering::SeqCst) {
            if let Ok(metadata) = fs::symlink_metadata(&observer_path) {
                if !metadata.file_type().is_file()
                    || fs::read(&observer_path).is_ok_and(|content| content != observer_expected)
                {
                    observer_partial.store(true, Ordering::SeqCst);
                    break;
                }
            }
        }
    });
    create_request_marker(&directory.paths.request, "0123456789abcdef0123456789abcdef").unwrap();
    stop.store(true, Ordering::SeqCst);
    observer.join().unwrap();
    assert!(!observed_partial.load(Ordering::SeqCst));
    assert_eq!(fs::read(&directory.paths.request).unwrap(), expected);
}

#[cfg(any(unix, windows))]
#[test]
fn destination_exists_is_no_clobber_and_publication_rolls_back_tracking() {
    let directory = TestDirectory::new();
    fs::create_dir_all(directory.paths.request.parent().unwrap()).unwrap();
    let existing = b"existing request\n";
    fs::write(&directory.paths.request, existing).unwrap();
    let failure =
        create_request_marker(&directory.paths.request, "abcdef0123456789abcdef0123456789")
            .unwrap_err();
    assert_eq!(failure, SetupFileError::Exists);
    assert_eq!(fs::read(&directory.paths.request).unwrap(), existing);

    let directory = TestDirectory::new();
    directory.prepare();
    let service = SetupPortalService::test(environment(&directory, Arc::new(AtomicU64::new(1)), 9));
    fs::write(&directory.paths.request, existing).unwrap();
    let token = service.prepare(&request("publish-rollback", 1)).unwrap();
    assert!(service.publish(&token).is_err());
    assert_eq!(service.pending_count(), 0);
    assert_eq!(fs::read(&directory.paths.request).unwrap(), existing);
}

#[cfg(any(unix, windows))]
#[test]
fn root_claim_race_is_no_clobber_and_temp_is_cleaned() {
    let directory = TestDirectory::new();
    fs::create_dir_all(directory.paths.request.parent().unwrap()).unwrap();
    let existing = b"root-claimed request\n";
    let result = create_request_marker_with_publisher_for_test(
        &directory.paths.request,
        "0123456789abcdef0123456789abcdef",
        |_, destination| {
            fs::write(destination, existing).unwrap();
            Err(SetupFileError::Exists)
        },
    );
    assert_eq!(result, Err(SetupFileError::Exists));
    assert_eq!(fs::read(&directory.paths.request).unwrap(), existing);
    assert_eq!(
        fs::read_dir(directory.paths.request.parent().unwrap())
            .unwrap()
            .count(),
        1
    );
}

#[cfg(any(unix, windows))]
#[test]
fn accepted_receipt_binds_identity_and_root_sequence_revision() {
    #[cfg(unix)]
    if unsafe { libc::geteuid() } != 0 {
        return;
    }
    let directory = TestDirectory::new();
    directory.prepare();
    let clock = Arc::new(AtomicU64::new(1));
    let service = SetupPortalService::test(environment(&directory, clock, 1));
    let request = request("setup-portal-identity", 41);
    let token = service.start(&request).unwrap();
    fs::remove_file(&directory.paths.request).unwrap();
    write_receipt(&directory, &token, ATTEMPT, 4);
    write_envelope(
        &directory.paths.current,
        ATTEMPT,
        4,
        setup_status(RuntimeSetupPortalPhase::PortalReady),
    );
    let first = result(service.poll_one().unwrap());
    assert!(matches!(
        first,
        RuntimeStoreResult::Identified { request_id, revision: Some(4), result }
            if request_id == "setup-portal-identity"
                && matches!(result.as_ref(), RuntimeStoreResult::SetupPortalStatus { status } if status.phase == RuntimeSetupPortalPhase::PortalReady)
    ));
    assert!(service.poll_one().is_none());
    write_envelope(
        &directory.paths.current,
        ATTEMPT,
        5,
        setup_status(RuntimeSetupPortalPhase::Succeeded),
    );
    let terminal = result(service.poll_one().unwrap());
    assert!(matches!(
        terminal,
        RuntimeStoreResult::Identified {
            revision: Some(5),
            ..
        }
    ));
    assert_eq!(service.pending_count(), 0);
}

#[cfg(any(unix, windows))]
#[test]
fn app_restart_does_not_follow_old_current_until_new_receipt() {
    #[cfg(unix)]
    if unsafe { libc::geteuid() } != 0 {
        return;
    }
    let directory = TestDirectory::new();
    directory.prepare();
    let clock = Arc::new(AtomicU64::new(1));
    let old = SetupPortalService::test(environment(&directory, clock.clone(), 2));
    let old_token = old.start(&request("old", 1)).unwrap();
    fs::remove_file(&directory.paths.request).unwrap();
    write_receipt(&directory, &old_token, ATTEMPT, 1);
    write_envelope(
        &directory.paths.current,
        ATTEMPT,
        1,
        setup_status(RuntimeSetupPortalPhase::Starting),
    );
    let restarted = SetupPortalService::test(environment(&directory, clock, 3));
    let new_token = restarted.start(&request("new", 2)).unwrap();
    fs::remove_file(&directory.paths.request).unwrap();
    assert!(restarted.poll_one().is_none());
    write_receipt(&directory, &new_token, NEW_ATTEMPT, 2);
    write_envelope(
        &directory.paths.current,
        NEW_ATTEMPT,
        2,
        setup_status(RuntimeSetupPortalPhase::Starting),
    );
    let bound = result(restarted.poll_one().unwrap());
    assert!(
        matches!(bound, RuntimeStoreResult::Identified { request_id, revision: Some(2), .. } if request_id == "new")
    );
}

#[cfg(any(unix, windows))]
#[test]
fn terminal_receipt_and_malformed_or_oversized_statuses_fail_typed() {
    #[cfg(unix)]
    if unsafe { libc::geteuid() } != 0 {
        return;
    }
    let directory = TestDirectory::new();
    directory.prepare();
    let clock = Arc::new(AtomicU64::new(1));
    let service = SetupPortalService::test(environment(&directory, clock.clone(), 4));
    let token = service.start(&request("terminal", 1)).unwrap();
    fs::remove_file(&directory.paths.request).unwrap();
    write_envelope(
        &directory.paths.receipts.join(format!("{token}.json")),
        ATTEMPT,
        3,
        setup_status(RuntimeSetupPortalPhase::Failed),
    );
    let failed = result(service.poll_one().unwrap());
    assert!(
        matches!(failed, RuntimeStoreResult::Identified { revision: Some(3), result, .. } if result.error_facts().is_some())
    );

    let malformed = json!({"schema": 1, "bootId": BOOT, "attemptId": ATTEMPT, "sequence": 1, "status": {"type": "setup_portal_status", "phase": "succeeded", "rebootRequired": false, "secret": "x"}});
    assert!(serde_json::from_value::<ValidatedStatusEnvelope>(malformed).is_err());
    let oversized = "{".repeat(crate::setup_portal_files::MAX_STATUS_BYTES as usize + 1);
    fs::write(&directory.paths.current, oversized).unwrap();
    fs::set_permissions(&directory.paths.current, permissions(0o640)).unwrap();
    let service = SetupPortalService::test(environment(&directory, clock.clone(), 5));
    let token = service.start(&request("oversized", 2)).unwrap();
    fs::remove_file(&directory.paths.request).unwrap();
    write_receipt(&directory, &token, ATTEMPT, 1);
    assert!(service.poll_one().is_some());
    clock.store(20_000, Ordering::SeqCst);
}

#[test]
fn status_envelope_rejects_bad_boot_attempt_and_sequence() {
    for value in [
        json!({"schema": 1, "bootId": "bad", "attemptId": ATTEMPT, "sequence": 1, "status": setup_status(RuntimeSetupPortalPhase::Succeeded)}),
        json!({"schema": 1, "bootId": BOOT, "attemptId": "bad", "sequence": 1, "status": setup_status(RuntimeSetupPortalPhase::Succeeded)}),
        json!({"schema": 1, "bootId": BOOT, "attemptId": ATTEMPT, "sequence": 0, "status": setup_status(RuntimeSetupPortalPhase::Succeeded)}),
        json!({"schema": 2, "bootId": BOOT, "attemptId": ATTEMPT, "sequence": 1, "status": setup_status(RuntimeSetupPortalPhase::Succeeded)}),
    ] {
        assert!(serde_json::from_value::<ValidatedStatusEnvelope>(value).is_err());
    }
}

#[test]
fn poll_backpressure_buffers_without_polling_again() {
    let directory = TestDirectory::new();
    let clock = Arc::new(AtomicU64::new(1));
    let service = SetupPortalService::test(environment(&directory, clock, 7));
    service.buffer_result(HostMessage::RuntimeResult {
        result: RuntimeStoreResult::OperationSucceeded {
            operation: playback_runtime::RuntimeOperation::SetupPortal,
            request_id: Some("backpressure".into()),
            revision: Some(1),
        },
    });
    assert!(service.has_buffered_result());
    assert!(service.take_buffered_result().is_some());
}
