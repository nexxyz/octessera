use super::*;
use std::sync::{Arc, Mutex};

fn root(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "octessera-orange-apply-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn boot(index: char) -> &'static str {
    match index {
        'a' => "01234567-89ab-cdef-0123-456789abcdef",
        _ => "fedcba98-7654-3210-fedc-ba9876543210",
    }
}

#[test]
fn apply_rolls_back_exact_bytes_and_restores_mode() {
    let directory = root("bytes");
    let default = default_path(&directory);
    let prior = b"{\n  \"not-json\": [1, 2, 3]\n}\0";
    fs::write(&default, prior).unwrap();
    let transaction = prepare_at(&directory, &serde_json::json!({"new": true}), boot('a')).unwrap();
    assert_eq!(
        fs::read(&default).unwrap(),
        serde_json::to_vec_pretty(&serde_json::json!({"new": true})).unwrap()
    );
    transaction.rollback().unwrap();
    assert_eq!(fs::read(&default).unwrap(), prior);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(default).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }
    assert!(!transaction_path(&directory).exists());
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn recovery_restores_same_boot_and_retains_different_boot() {
    let directory = root("recovery");
    let prior = b"old bytes";
    fs::write(default_path(&directory), prior).unwrap();
    prepare_at(&directory, &serde_json::json!({"new": true}), boot('a')).unwrap();
    recover_startup_at(&directory, boot('a')).unwrap();
    assert_eq!(fs::read(default_path(&directory)).unwrap(), prior);
    assert!(!transaction_path(&directory).exists());

    prepare_at(&directory, &serde_json::json!({"new": false}), boot('a')).unwrap();
    recover_startup_at(&directory, boot('b')).unwrap();
    assert_eq!(
        fs::read(default_path(&directory)).unwrap(),
        b"{\n  \"new\": false\n}"
    );
    assert!(!transaction_path(&directory).exists());
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn record_left_before_new_write_is_recovered_idempotently() {
    let directory = root("crash-before-default");
    let prior = b"prior";
    fs::write(default_path(&directory), prior).unwrap();
    let record = OrangeApplyRecord {
        schema: 1,
        boot_id: boot('a').into(),
        prior_default_bytes: Some(prior.into()),
    };
    write_record(&directory, &record).unwrap();
    recover_startup_at(&directory, boot('a')).unwrap();
    recover_startup_at(&directory, boot('a')).unwrap();
    assert_eq!(fs::read(default_path(&directory)).unwrap(), prior);
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn malformed_transaction_fails_closed_without_touching_default() {
    let directory = root("malformed");
    let default = default_path(&directory);
    fs::write(&default, b"new config").unwrap();
    fs::write(
        transaction_path(&directory),
        br#"{"schema":1,"boot_id":"not-a-boot","prior_default_bytes":null}"#,
    )
    .unwrap();
    assert!(recover_startup_at(&directory, boot('a')).is_err());
    assert_eq!(fs::read(default).unwrap(), b"new config");
    assert!(transaction_path(&directory).exists());
    let _ = fs::remove_dir_all(directory);
}

struct OrderedHost {
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl OrangeApplyHost for OrderedHost {
    fn panic_external_midi(&mut self) -> Result<(), String> {
        self.events.lock().unwrap().push("midi-panic");
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), String> {
        self.events.lock().unwrap().push("internal-silence");
        Ok(())
    }
}

struct FailingHost {
    events: Arc<Mutex<Vec<&'static str>>>,
    panic_failure: bool,
    silence_failure: bool,
}

impl OrangeApplyHost for FailingHost {
    fn panic_external_midi(&mut self) -> Result<(), String> {
        self.events.lock().unwrap().push("midi-panic");
        if self.panic_failure {
            Err("panic failed".into())
        } else {
            Ok(())
        }
    }

    fn silence_internal_audio(&mut self) -> Result<(), String> {
        self.events.lock().unwrap().push("internal-silence");
        if self.silence_failure {
            Err("silence failed".into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn apply_orders_panic_silence_helper_then_teardown() {
    let directory = root("order");
    fs::write(default_path(&directory), b"old").unwrap();
    let transaction = prepare_at(&directory, &serde_json::json!({"new": true}), boot('a')).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut host = OrderedHost {
        events: events.clone(),
    };
    let helper_events = events.clone();
    resolve_shutdown_request_with_helper(
        OrangeShutdownRequest::ApplyDeviceConfig(transaction),
        &mut host,
        move || {
            helper_events.lock().unwrap().push("helper");
            OrangeHelperOutcome::Accepted
        },
    )
    .unwrap();
    events.lock().unwrap().push("teardown");
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["midi-panic", "internal-silence", "helper", "teardown"]
    );
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn apply_outcome_matrix_has_expected_exit_policy() {
    for (outcome, code, restores) in [
        (OrangeHelperOutcome::Accepted, 0, false),
        (OrangeHelperOutcome::Rejected, 1, true),
        (OrangeHelperOutcome::NotSubmitted, 1, true),
        (OrangeHelperOutcome::Indeterminate, 78, false),
    ] {
        let directory = root("matrix");
        fs::write(default_path(&directory), b"old").unwrap();
        let transaction =
            prepare_at(&directory, &serde_json::json!({"new": true}), boot('a')).unwrap();
        let mut host = OrderedHost {
            events: Arc::new(Mutex::new(Vec::new())),
        };
        let result = resolve_shutdown_request_with_helper(
            OrangeShutdownRequest::ApplyDeviceConfig(transaction),
            &mut host,
            || outcome,
        );
        assert_eq!(
            result.as_ref().err().map_or(0, OrangeRunError::exit_code),
            code
        );
        assert_eq!(
            fs::read(default_path(&directory)).unwrap() == b"old",
            restores
        );
        let _ = fs::remove_dir_all(directory);
    }
}

#[test]
fn silence_failure_rolls_back_before_ordinary_exit() {
    let directory = root("silence-failure");
    fs::write(default_path(&directory), b"old").unwrap();
    let transaction = prepare_at(&directory, &serde_json::json!({"new": true}), boot('a')).unwrap();
    let mut host = FailingHost {
        events: Arc::new(Mutex::new(Vec::new())),
        panic_failure: false,
        silence_failure: true,
    };
    let result = resolve_shutdown_request_with_helper(
        OrangeShutdownRequest::ApplyDeviceConfig(transaction),
        &mut host,
        || panic!("helper must not run after silence failure"),
    )
    .unwrap_err();
    assert_eq!(result.exit_code(), 1);
    assert_eq!(fs::read(default_path(&directory)).unwrap(), b"old");
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn rollback_failure_is_special_exit_78() {
    let directory = root("rollback-failure");
    fs::write(default_path(&directory), b"old").unwrap();
    let transaction = prepare_at(&directory, &serde_json::json!({"new": true}), boot('a')).unwrap();
    fs::remove_file(default_path(&directory)).unwrap();
    fs::create_dir(default_path(&directory)).unwrap();
    let mut host = OrderedHost {
        events: Arc::new(Mutex::new(Vec::new())),
    };
    let result = resolve_shutdown_request_with_helper(
        OrangeShutdownRequest::ApplyDeviceConfig(transaction),
        &mut host,
        || OrangeHelperOutcome::Rejected,
    )
    .unwrap_err();
    assert_eq!(result.exit_code(), 78);
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn ordinary_and_special_exit_mapping_is_typed() {
    assert_eq!(OrangeRunError::Ordinary("ordinary".into()).exit_code(), 1);
    assert_eq!(
        OrangeRunError::SpecialExit78("special".into()).exit_code(),
        78
    );
}

#[test]
fn ordinary_reboot_resolution_defers_helper_until_teardown() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut host = OrderedHost {
        events: events.clone(),
    };
    let result =
        resolve_shutdown_request_with_helper(OrangeShutdownRequest::Reboot, &mut host, || {
            events.lock().unwrap().push("helper");
            OrangeHelperOutcome::Accepted
        })
        .unwrap();
    assert!(matches!(
        &result,
        OrangeShutdownResolution::Power {
            action: OrangePowerAction::Reboot,
            safety_failure: None
        }
    ));
    events.lock().unwrap().push("teardown");
    finish_shutdown_resolution_with_helper(result, || {
        events.lock().unwrap().push("helper");
        OrangeHelperOutcome::Accepted
    })
    .unwrap();
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["midi-panic", "internal-silence", "teardown", "helper"]
    );
}

#[test]
fn ordinary_reboot_safety_failure_attempts_both_and_fails_closed() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut host = FailingHost {
        events: events.clone(),
        panic_failure: true,
        silence_failure: false,
    };
    let resolution =
        resolve_shutdown_request_with_helper(OrangeShutdownRequest::Reboot, &mut host, || {
            panic!("ordinary helper must wait for safe shutdown")
        })
        .unwrap();
    events.lock().unwrap().push("teardown");
    let error = finish_shutdown_resolution_with_helper(resolution, || {
        panic!("ordinary helper must not run after safety failure")
    })
    .unwrap_err();
    assert_eq!(error.exit_code(), 1);
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["midi-panic", "internal-silence", "teardown"]
    );
}

#[test]
fn ordinary_shutdown_completes_after_both_safety_steps() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut host = OrderedHost {
        events: events.clone(),
    };
    let resolution =
        resolve_shutdown_request_with_helper(OrangeShutdownRequest::Shutdown, &mut host, || {
            events.lock().unwrap().push("poweroff");
            OrangeHelperOutcome::Accepted
        })
        .unwrap();
    assert!(matches!(
        resolution,
        OrangeShutdownResolution::Power {
            action: OrangePowerAction::Shutdown,
            safety_failure: None
        }
    ));
    events.lock().unwrap().push("teardown");
    finish_shutdown_resolution_with_helper(resolution, || {
        events.lock().unwrap().push("poweroff");
        OrangeHelperOutcome::Accepted
    })
    .unwrap();
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["midi-panic", "internal-silence", "teardown", "poweroff"]
    );
}

#[test]
fn ordinary_shutdown_safety_failure_prevents_power_action() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut host = FailingHost {
        events: events.clone(),
        panic_failure: false,
        silence_failure: true,
    };
    let resolution =
        resolve_shutdown_request_with_helper(OrangeShutdownRequest::Shutdown, &mut host, || {
            panic!("shutdown helper must wait for safe shutdown")
        })
        .unwrap();
    events.lock().unwrap().push("teardown");
    let error = finish_shutdown_resolution_with_helper(resolution, || {
        panic!("shutdown helper must not run after safety failure")
    })
    .unwrap_err();
    assert_eq!(error.exit_code(), 1);
    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["midi-panic", "internal-silence", "teardown"]
    );
}

#[test]
fn ordinary_power_outcome_matrix_is_typed_and_never_panics() {
    for (action, outcome) in [
        (OrangePowerAction::Reboot, OrangeHelperOutcome::Rejected),
        (
            OrangePowerAction::Reboot,
            OrangeHelperOutcome::Indeterminate,
        ),
        (
            OrangePowerAction::Shutdown,
            OrangeHelperOutcome::NotSubmitted,
        ),
    ] {
        let mut host = OrderedHost {
            events: Arc::new(Mutex::new(Vec::new())),
        };
        let request = match action {
            OrangePowerAction::Reboot => OrangeShutdownRequest::Reboot,
            OrangePowerAction::Shutdown => OrangeShutdownRequest::Shutdown,
        };
        let resolution =
            resolve_shutdown_request_with_helper(request, &mut host, || outcome).unwrap();
        let error = finish_shutdown_resolution_with_helper(resolution, || outcome).unwrap_err();
        assert_eq!(error.exit_code(), 1);
    }
}
