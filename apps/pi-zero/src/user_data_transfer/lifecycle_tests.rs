use super::close_race_tests::{wait_for_received_upload, wait_for_server_exit};
use super::restore_tests::{restore_archive, restore_request, restore_service};
use super::*;
use playback_runtime::{RuntimePlatformEffect, RuntimePlatformRequest};
use std::fs;
use std::thread;
use std::time::{Duration, Instant};

fn status(service: &UserDataTransferService, code: &str) -> String {
    String::from_utf8(restore_request(service, "GET", "/restore/status", code, &[]).1).unwrap()
}

fn press(service: &UserDataTransferService, input: serde_json::Value) {
    service.handle_physical_input(&input);
}

fn upload(service: &UserDataTransferService, code: &str) -> u16 {
    let archive = restore_archive(service, false);
    restore_request(service, "POST", "/restore", code, &archive).0
}

fn transfer_leftovers(store: &Path) -> Vec<String> {
    fs::read_dir(store)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".user-data-"))
        .collect()
}

#[test]
fn a_transfer_session_runs_cancel_expiry_restore_close_and_reopen_in_order() {
    let (service, root) = restore_service("lifecycle");
    let store = service.inner.store_dir.clone();
    service.start().unwrap();
    let code = service.test_code().unwrap();

    assert_eq!(upload(&service, &code), 202);
    assert!(status(&service, &code).contains("confirmation_required"));
    press(
        &service,
        serde_json::json!({"type":"button_a","pressed":true}),
    );
    assert!(status(&service, &code).contains("cancelled"));
    assert!(transfer_leftovers(&store).is_empty());

    assert_eq!(upload(&service, &code), 202);
    if let Ok(mut state) = service.inner.state.lock() {
        if let RestoreState::Pending(pending) = &mut state.restore {
            pending.expires_at = Instant::now() - Duration::from_secs(1);
        }
    }
    service.expire_if_needed();
    assert!(status(&service, &code).contains("timed_out"));
    assert!(transfer_leftovers(&store).is_empty());
    press(
        &service,
        serde_json::json!({"type":"encoder_press","id":"main"}),
    );
    assert!(!status(&service, &code).contains("restoring"));

    assert_eq!(upload(&service, &code), 202);
    press(
        &service,
        serde_json::json!({"type":"encoder_press","id":"main"}),
    );
    service.join_restore_worker();
    assert!(status(&service, &code).contains("restored"));
    service.store_write_barrier().acknowledge();

    let close = RuntimePlatformRequest::new(
        RuntimePlatformEffect::UserDataTransferClose,
        "close".into(),
        None,
    );
    service.close(&close);
    assert!(!service.confirm_pending_restore(true));
    wait_for_server_exit(&service);
    service.start().unwrap();
    let reopened = service.test_code().unwrap();
    assert!(status(&service, &reopened).contains("idle"));
    assert!(transfer_leftovers(&store).is_empty());
    service.stop();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn staging_that_finishes_after_close_never_arms_and_reopen_never_waits_for_it() {
    let (service, root) = restore_service("lifecycle-close-race");
    let store = service.inner.store_dir.clone();
    let original = fs::read(store.join("system.json")).unwrap();
    service.start().unwrap();
    let code = service.test_code().unwrap();
    let archive = restore_archive(&service, false);
    let length = archive.len() as u64;
    let store_guard = service.inner.stage_gate.lock().unwrap();
    let uploader = {
        let service = service.clone();
        let code = code.clone();
        thread::spawn(move || restore_request(&service, "POST", "/restore", &code, &archive).0)
    };
    wait_for_received_upload(&store, length);
    service.close(&RuntimePlatformRequest::new(
        RuntimePlatformEffect::UserDataTransferClose,
        "close".into(),
        None,
    ));

    let reopen_started = Instant::now();
    assert_eq!(
        service.start(),
        Err("Still closing, try again in a moment".to_string())
    );
    assert!(reopen_started.elapsed() < Duration::from_secs(1));
    drop(store_guard);
    assert_eq!(uploader.join().unwrap(), 410);
    press(
        &service,
        serde_json::json!({"type":"encoder_press","id":"main"}),
    );

    assert!(!service.confirm_pending_restore(true));
    assert_eq!(fs::read(store.join("system.json")).unwrap(), original);
    assert!(transfer_leftovers(&store).is_empty());
    wait_for_server_exit(&service);
    service.start().unwrap();
    service.stop();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_restore_refused_by_unfinished_staging_reports_its_own_phase() {
    let (service, root) = restore_service("unfinished-staging");
    let leftover = root
        .join(crate::user_data_restore::STAGING_DIR_NAME)
        .join(".octessera-store-old-earlier-session");
    fs::create_dir_all(&leftover).unwrap();
    let identity = RuntimePlatformRequest::new(
        RuntimePlatformEffect::SetupPortalOpen,
        "unfinished".into(),
        None,
    );
    service.start_session(Some(&identity)).unwrap();
    let code = service.test_code().unwrap();

    assert_eq!(upload(&service, &code), 202);
    press(
        &service,
        serde_json::json!({"type":"encoder_press","id":"main"}),
    );
    service.join_restore_worker();

    let phases = std::iter::from_fn(|| service.take_runtime_status())
        .filter_map(|message| match message {
            playback_runtime::HostMessage::RuntimeResult {
                result: playback_runtime::RuntimeStoreResult::Identified { result, .. },
            } => match *result {
                playback_runtime::RuntimeStoreResult::UserDataRestoreStatus { status } => {
                    Some(status.phase)
                }
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        phases,
        vec![
            playback_runtime::RuntimeUserDataRestorePhase::Restoring,
            playback_runtime::RuntimeUserDataRestorePhase::EarlierRestoreUnfinished
        ]
    );
    assert!(leftover.exists());
    service.stop();
    let _ = fs::remove_dir_all(root);
}
