use super::*;
use crate::setup_portal_files::{
    create_request_marker_with_publisher_for_test, validate_public_metadata, SetupFileError,
    SetupFileKind, SetupMetadata, SetupPortalPaths,
};
use playback_runtime::{
    RuntimePlatformEffect, RuntimeSetupPortalDisposition, RuntimeSetupPortalErrorCode,
    RuntimeSetupPortalPhase, RuntimeStoreResult,
};
use serde_json::{json, Value};
use std::fs;
#[cfg(any(unix, windows))]
use std::path::Path;
use std::path::PathBuf;

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
                request: root.join("request").join("inbox").join("start"),
                current: public.join("current.json"),
                public,
            },
            root,
        }
    }

    #[cfg(any(unix, windows))]
    fn prepare(&self) {
        prepare_request_parent(self.paths.request.parent().unwrap());
        fs::create_dir_all(&self.paths.public).unwrap();
        fs::set_permissions(&self.paths.public, permissions(0o750)).unwrap();
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(any(unix, windows))]
fn prepare_request_parent(path: &Path) {
    fs::create_dir_all(path).unwrap();
    #[cfg(unix)]
    fs::set_permissions(path, permissions(0o700)).unwrap();
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

fn environment(directory: &TestDirectory) -> SetupPortalEnvironment {
    #[cfg(unix)]
    let status_group = unsafe { libc::getegid() };
    #[cfg(windows)]
    let status_group = 0;
    SetupPortalEnvironment::test(directory.paths.clone(), status_group)
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
    match phase {
        RuntimeSetupPortalPhase::Starting => {
            status["disposition"] = json!("accepted");
        }
        RuntimeSetupPortalPhase::PortalReady => {
            status["portalSuffix"] = json!("abcd");
        }
        RuntimeSetupPortalPhase::Failed => {
            status["errorCode"] = json!("operation_failed");
        }
        RuntimeSetupPortalPhase::TimedOut => {
            status["errorCode"] = json!("unavailable");
        }
        RuntimeSetupPortalPhase::Unsupported => {
            status["errorCode"] = json!("unsupported");
        }
        RuntimeSetupPortalPhase::Finalizing | RuntimeSetupPortalPhase::Succeeded => {}
    }
    status
}

#[cfg(any(unix, windows))]
fn write_status(directory: &TestDirectory, phase: RuntimeSetupPortalPhase) {
    fs::write(
        &directory.paths.current,
        serde_json::to_vec(&json!({
            "schema": 1,
            "status": setup_status(phase)
        }))
        .unwrap(),
    )
    .unwrap();
    fs::set_permissions(&directory.paths.current, permissions(0o640)).unwrap();
}

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

#[cfg(unix)]
#[test]
fn fixed_marker_is_exact_private_and_create_new_rejects_existing_and_symlink() {
    let directory = TestDirectory::new();
    prepare_request_parent(directory.paths.request.parent().unwrap());
    create_request_marker(&directory.paths.request).unwrap();
    assert_eq!(fs::read(&directory.paths.request).unwrap(), b"start\n");
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
        create_request_marker(&directory.paths.request),
        Err(SetupFileError::Exists)
    );
    fs::remove_file(&directory.paths.request).unwrap();
    std::os::unix::fs::symlink(directory.root.join("missing"), &directory.paths.request).unwrap();
    assert_eq!(
        create_request_marker(&directory.paths.request),
        Err(SetupFileError::Unsafe)
    );
}

#[cfg(any(unix, windows))]
#[test]
fn marker_publication_never_exposes_partial_final_content() {
    let directory = TestDirectory::new();
    prepare_request_parent(directory.paths.request.parent().unwrap());
    let result = create_request_marker_with_publisher_for_test(
        &directory.paths.request,
        |source, destination| {
            assert_eq!(fs::read(source).unwrap(), b"start\n");
            assert!(!destination.exists());
            fs::rename(source, destination).unwrap();
            Ok(())
        },
    );
    assert_eq!(result, Ok(()));
    assert_eq!(fs::read(&directory.paths.request).unwrap(), b"start\n");
}

#[cfg(unix)]
#[test]
fn marker_parent_requires_current_private_real_directory() {
    use std::os::unix::fs::PermissionsExt;
    let directory = TestDirectory::new();
    let parent = directory.paths.request.parent().unwrap();
    fs::create_dir_all(parent).unwrap();
    fs::set_permissions(parent, fs::Permissions::from_mode(0o750)).unwrap();
    assert_eq!(
        create_request_marker(&directory.paths.request),
        Err(SetupFileError::Unsafe)
    );
}

#[cfg(any(unix, windows))]
#[test]
fn setup_service_publishes_fixed_marker_and_already_running_disposition() {
    let directory = TestDirectory::new();
    directory.prepare();
    let service = SetupPortalService::test(environment(&directory));
    assert_eq!(
        service.start(&request("accepted", 1)).unwrap(),
        RuntimeSetupPortalDisposition::Accepted
    );
    assert_eq!(fs::read(&directory.paths.request).unwrap(), b"start\n");

    let second = SetupPortalService::test(environment(&directory));
    assert_eq!(
        second.start(&request("already", 2)).unwrap(),
        RuntimeSetupPortalDisposition::AlreadyRunning
    );
    write_status(&directory, RuntimeSetupPortalPhase::PortalReady);
    assert!(matches!(
        result(second.poll_one().unwrap()),
        RuntimeStoreResult::Identified { result, .. }
            if matches!(result.as_ref(), RuntimeStoreResult::SetupPortalStatus { status } if status.phase == RuntimeSetupPortalPhase::PortalReady)
    ));
}

#[test]
fn backend_status_envelope_round_trips_all_image_phases_and_requires_type() {
    for phase in [
        RuntimeSetupPortalPhase::Starting,
        RuntimeSetupPortalPhase::PortalReady,
        RuntimeSetupPortalPhase::Finalizing,
        RuntimeSetupPortalPhase::Succeeded,
        RuntimeSetupPortalPhase::Failed,
        RuntimeSetupPortalPhase::TimedOut,
    ] {
        let envelope = json!({
            "schema": 1,
            "status": setup_status(phase.clone()),
        });
        let parsed = serde_json::from_value::<protocol::ValidatedStatusEnvelope>(envelope).unwrap();
        assert_eq!(parsed.status.phase, phase);
    }

    let mut missing_type = setup_status(RuntimeSetupPortalPhase::Starting);
    missing_type.as_object_mut().unwrap().remove("type");
    assert!(
        serde_json::from_value::<protocol::ValidatedStatusEnvelope>(json!({
            "schema": 1,
            "status": missing_type,
        }))
        .is_err()
    );
}

#[cfg(any(unix, windows))]
#[test]
fn old_terminal_is_ignored_until_fresh_starting_then_phases_do_not_regress() {
    let directory = TestDirectory::new();
    directory.prepare();
    let service = SetupPortalService::test(environment(&directory));
    service.start(&request("setup", 7)).unwrap();

    write_status(&directory, RuntimeSetupPortalPhase::Succeeded);
    assert!(service.poll_one().is_none());

    write_status(&directory, RuntimeSetupPortalPhase::Starting);
    assert!(matches!(
        result(service.poll_one().unwrap()),
        RuntimeStoreResult::Identified { result, .. }
            if matches!(result.as_ref(), RuntimeStoreResult::SetupPortalStatus { status } if status.phase == RuntimeSetupPortalPhase::Starting)
    ));
    write_status(&directory, RuntimeSetupPortalPhase::PortalReady);
    let ready = result(service.poll_one().unwrap());
    assert!(matches!(
        ready,
        RuntimeStoreResult::Identified { request_id, revision: Some(7), result }
            if request_id == "setup"
                && matches!(result.as_ref(), RuntimeStoreResult::SetupPortalStatus { status } if status.phase == RuntimeSetupPortalPhase::PortalReady)
    ));
    assert!(service.poll_one().is_none());

    write_status(&directory, RuntimeSetupPortalPhase::Starting);
    assert!(service.poll_one().is_none());
    write_status(&directory, RuntimeSetupPortalPhase::Finalizing);
    assert!(service.poll_one().is_some());
    write_status(&directory, RuntimeSetupPortalPhase::Succeeded);
    assert!(service.poll_one().is_some());
    assert_eq!(service.pending_count(), 0);
}

#[cfg(any(unix, windows))]
#[test]
fn new_request_ignores_previous_terminal_and_binds_results_locally() {
    let directory = TestDirectory::new();
    directory.prepare();
    write_status(&directory, RuntimeSetupPortalPhase::TimedOut);
    let service = SetupPortalService::test(environment(&directory));
    service.start(&request("new-request", 11)).unwrap();
    assert!(service.poll_one().is_none());

    write_status(&directory, RuntimeSetupPortalPhase::Starting);
    assert!(service.poll_one().is_some());
    write_status(&directory, RuntimeSetupPortalPhase::PortalReady);
    let ready = result(service.poll_one().unwrap());
    assert!(matches!(
        ready,
        RuntimeStoreResult::Identified { request_id, revision: Some(11), .. }
            if request_id == "new-request"
    ));
}

#[cfg(any(unix, windows))]
#[test]
fn status_envelope_is_single_current_file_and_rejects_identity_fields() {
    let directory = TestDirectory::new();
    directory.prepare();
    let service = SetupPortalService::test(environment(&directory));
    service.start(&request("malformed", 1)).unwrap();
    fs::write(
        &directory.paths.current,
        serde_json::to_vec(&json!({
            "schema": 1,
            "bootId": "old",
            "status": setup_status(RuntimeSetupPortalPhase::Succeeded)
        }))
        .unwrap(),
    )
    .unwrap();
    fs::set_permissions(&directory.paths.current, permissions(0o640)).unwrap();
    let failed = result(service.poll_one().unwrap());
    assert!(matches!(
        failed,
        RuntimeStoreResult::Identified { result, .. }
            if matches!(result.as_ref(), RuntimeStoreResult::SetupPortalStatus { status } if status.phase == RuntimeSetupPortalPhase::Failed && status.error_code == Some(RuntimeSetupPortalErrorCode::InvalidPayload))
    ));
}

#[test]
fn start_failure_is_identified() {
    let message = start_failure_message(
        &request("local-failure", 6),
        SetupPortalFailure::from_file(SetupFileError::Permission),
    );
    let result = result(message);
    assert!(matches!(
        result,
        RuntimeStoreResult::Identified {
            request_id,
            revision: Some(6),
            result
        } if request_id == "local-failure"
            && matches!(result.as_ref(), RuntimeStoreResult::SetupPortalStatus { status } if status.phase == RuntimeSetupPortalPhase::Failed)
    ));
}
