use super::*;
use std::sync::{Arc, Mutex};

fn root(label: &str) -> PathBuf {
    let path = crate::test_temp_dir::unique_temp_path(&format!("octessera-orange-apply-{label}"));
    fs::create_dir_all(&path).unwrap();
    path
}
fn boot(index: char) -> &'static str {
    match index {
        'a' => "01234567-89ab-cdef-0123-456789abcdef",
        _ => "fedcba98-7654-3210-fedc-ba9876543210",
    }
}

fn write_default_patch(directory: &Path) -> Value {
    let full: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let documents = playback_runtime::split_system_patch_documents(&full).unwrap();
    fs::create_dir_all(directory.join("patches")).unwrap();
    fs::write(
        crate::platform_service::default_patch_path(directory),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();
    documents.system
}

fn prepare_valid(directory: &Path, boot_id: char) -> (OrangeDeviceApplyTransaction, Value) {
    let system = write_default_patch(directory);
    let transaction = prepare_at(directory, &system, boot(boot_id)).unwrap();
    (transaction, system)
}

#[test]
fn device_apply_accepts_local_samples_in_assigned_and_unassigned_slots() {
    let directory = root("local-sample-paths");
    let mut full: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    full["runtimeConfig"]["instruments"][0]["type"] = serde_json::json!("sampler");
    full["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] =
        serde_json::json!("userdata/User Kit/custom.wav");
    full["runtimeConfig"]["instruments"][0]["sample"]["slots"][4]["path"] =
        serde_json::json!("sd-card/octessera/samples/kick.wav");
    full["runtimeConfig"]["instruments"][0]["sample"]["assignments"] = serde_json::json!([
        { "level": null, "sampleSlot": 0, "x": 0, "y": 0 }
    ]);
    let documents = playback_runtime::split_local_system_patch_documents(&full).unwrap();
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        crate::platform_service::default_patch_path(&directory),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();

    let transaction = prepare_at(&directory, &documents.system, boot('a')).unwrap();
    transaction.rollback().unwrap();
    let patch = crate::platform_service::load_json(&crate::platform_service::default_patch_path(
        &directory,
    ))
    .unwrap()
    .unwrap();
    assert_eq!(
        patch["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"],
        "userdata/User Kit/custom.wav"
    );
    assert_eq!(
        patch["runtimeConfig"]["instruments"][0]["sample"]["slots"][4]["path"],
        "sd-card/octessera/samples/kick.wav"
    );
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn apply_rolls_back_exact_bytes_and_restores_mode() {
    let directory = root("bytes");
    let default = system_path(&directory);
    let prior = b"{\n  \"not-json\": [1, 2, 3]\n}\0";
    fs::write(&default, prior).unwrap();
    let (transaction, system) = prepare_valid(&directory, 'a');
    assert_eq!(
        fs::read(&default).unwrap(),
        serde_json::to_vec_pretty(&system).unwrap()
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
    fs::write(system_path(&directory), prior).unwrap();
    prepare_valid(&directory, 'a');
    recover_startup_at(&directory, boot('a')).unwrap();
    assert_eq!(fs::read(system_path(&directory)).unwrap(), prior);
    assert!(!transaction_path(&directory).exists());

    let (_, system) = prepare_valid(&directory, 'a');
    recover_startup_at(&directory, boot('b')).unwrap();
    assert_eq!(
        fs::read(system_path(&directory)).unwrap(),
        serde_json::to_vec_pretty(&system).unwrap()
    );
    assert!(!transaction_path(&directory).exists());
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn record_left_before_new_write_is_recovered_idempotently() {
    let directory = root("crash-before-default");
    let prior = b"prior";
    fs::write(system_path(&directory), prior).unwrap();
    write_default_patch(&directory);
    let record = OrangeApplyRecord {
        schema: 1,
        boot_id: boot('a').into(),
        prior_system_bytes: Some(prior.into()),
    };
    write_record(&directory, &record).unwrap();
    recover_startup_at(&directory, boot('a')).unwrap();
    recover_startup_at(&directory, boot('a')).unwrap();
    assert_eq!(fs::read(system_path(&directory)).unwrap(), prior);
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn malformed_transaction_fails_closed_without_touching_default() {
    let directory = root("malformed");
    let default = system_path(&directory);
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
fn apply_orders_panic_silence_reboot_request_then_teardown() {
    let directory = root("order");
    fs::write(system_path(&directory), b"old").unwrap();
    let (transaction, _) = prepare_valid(&directory, 'a');
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut host = OrderedHost {
        events: events.clone(),
    };
    let reboot_request_events = events.clone();
    resolve_shutdown_request_with_reboot_request(
        PowerRequest::ApplyDeviceConfig(transaction),
        &mut host,
        move || {
            reboot_request_events.lock().unwrap().push("reboot-request");
            OrangePowerRequestOutcome::Accepted
        },
    )
    .unwrap();
    events.lock().unwrap().push("teardown");
    assert_eq!(
        events.lock().unwrap().as_slice(),
        [
            "midi-panic",
            "internal-silence",
            "reboot-request",
            "teardown"
        ]
    );
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn apply_outcome_matrix_has_expected_exit_policy() {
    for (outcome, code, restores) in [
        (OrangePowerRequestOutcome::Accepted, 0, false),
        (OrangePowerRequestOutcome::Rejected, 1, true),
        (OrangePowerRequestOutcome::NotSubmitted, 1, true),
        (OrangePowerRequestOutcome::Indeterminate, 78, false),
    ] {
        let directory = root("matrix");
        fs::write(system_path(&directory), b"old").unwrap();
        let (transaction, _) = prepare_valid(&directory, 'a');
        let mut host = OrderedHost {
            events: Arc::new(Mutex::new(Vec::new())),
        };
        let result = resolve_shutdown_request_with_reboot_request(
            PowerRequest::ApplyDeviceConfig(transaction),
            &mut host,
            || outcome,
        );
        assert_eq!(
            result.as_ref().err().map_or(0, OrangeRunError::exit_code),
            code
        );
        assert_eq!(
            fs::read(system_path(&directory)).unwrap() == b"old",
            restores
        );
        let _ = fs::remove_dir_all(directory);
    }
}

#[test]
fn silence_failure_rolls_back_before_ordinary_exit() {
    let directory = root("silence-failure");
    fs::write(system_path(&directory), b"old").unwrap();
    let (transaction, _) = prepare_valid(&directory, 'a');
    let mut host = FailingHost {
        events: Arc::new(Mutex::new(Vec::new())),
        panic_failure: false,
        silence_failure: true,
    };
    let result = resolve_shutdown_request_with_reboot_request(
        PowerRequest::ApplyDeviceConfig(transaction),
        &mut host,
        || panic!("reboot request must not run after silence failure"),
    )
    .unwrap_err();
    assert_eq!(result.exit_code(), 1);
    assert_eq!(fs::read(system_path(&directory)).unwrap(), b"old");
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn rollback_failure_is_special_exit_78() {
    let directory = root("rollback-failure");
    fs::write(system_path(&directory), b"old").unwrap();
    let (transaction, _) = prepare_valid(&directory, 'a');
    fs::remove_file(system_path(&directory)).unwrap();
    fs::create_dir(system_path(&directory)).unwrap();
    let mut host = OrderedHost {
        events: Arc::new(Mutex::new(Vec::new())),
    };
    let result = resolve_shutdown_request_with_reboot_request(
        PowerRequest::ApplyDeviceConfig(transaction),
        &mut host,
        || OrangePowerRequestOutcome::Rejected,
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
