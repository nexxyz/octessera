use super::*;
use playback_runtime::{HostMessage, NativeRunnerConfig, RuntimeOperation, RuntimeStoreResult};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[path = "platform_native_autosave_backup_tests.rs"]
mod backup_tests;
#[path = "platform_native_autosave_failure_tests.rs"]
mod failure_tests;
#[path = "platform_native_load_admission_tests.rs"]
mod load_admission_tests;
#[path = "platform_local_patch_worker_tests.rs"]
mod local_patch_worker_tests;

fn runner_with_aux_mapping(auto_save: bool, backups: bool) -> NativeRunner {
    runner_with_aux_mapping_in_state(auto_save, backups, true)
}

fn runner_with_aux_mapping_in_state(auto_save: bool, backups: bool, playing: bool) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut full: Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    full["runtimeConfig"]["autoSaveDefault"] = json!(auto_save);
    full["runtimeConfig"]["rollingBackups"] = json!(backups);
    runner.apply_config_payload(full).unwrap();
    runner.skip_startup_splash();
    runner
        .test_focus_menu_item("aux:0:turn.instruments.0.synth.osc1.levelPct")
        .unwrap();
    input(&mut runner, json!({"type":"encoder_press","id":"main"}));
    if playing {
        input(&mut runner, json!({"type":"button_s","pressed":true}));
        input(&mut runner, json!({"type":"button_s","pressed":false}));
    }
    runner
}

fn input(runner: &mut NativeRunner, input: Value) -> Vec<playback_runtime::RunnerMessage> {
    runner
        .send_music_first(HostMessage::DeviceInput {
            input,
            request_snapshot: Some(false),
        })
        .unwrap()
}

fn aux_turn(runner: &mut NativeRunner, delta: i32) {
    input(
        runner,
        json!({"type":"encoder_turn","id":"aux1","delta":delta}),
    );
}

fn confirm_action(runner: &mut NativeRunner, key: &str) {
    runner.test_focus_menu_item(key).unwrap();
    input(runner, json!({"type":"encoder_press","id":"main"}));
    assert!(runner.test_confirmation_is_open());
    input(runner, json!({"type":"encoder_turn","id":"main","delta":1}));
    input(runner, json!({"type":"encoder_press","id":"main"}));
}

fn service_and_root(name: &str) -> (PiPlatformService, PathBuf) {
    let root =
        crate::test_temp_dir::unique_temp_path(&format!("octessera-pi-native-autosave-{name}"));
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let full: Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    let documents = playback_runtime::split_system_patch_documents(&full).unwrap();
    std::fs::write(
        store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    let service = PiPlatformService::new(store, root.join("samples"));
    (service, root)
}

fn collect_results(
    service: &PiPlatformService,
    runner: &mut NativeRunner,
    expected: usize,
) -> Vec<RuntimeStoreResult> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut completed = Vec::new();
    while Instant::now() < deadline && completed.len() < expected {
        for result in service.drain_platform_results(8) {
            let Some(message) = super::super::platform_native_persistence::finish_platform_result(
                service, runner, result,
            ) else {
                continue;
            };
            if let HostMessage::RuntimeResult { result } = &message {
                completed.push(result.clone());
            }
            runner.send_music_first(message).unwrap();
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        completed.len(),
        expected,
        "native persistence result timed out"
    );
    completed
}

fn apply_autosave_at(
    pending: &mut PendingPiPersistence,
    service: &PiPlatformService,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    now: Instant,
) -> Vec<HostMessage> {
    flush_due_native_persistence(pending, service, playback, runner, now)
}

#[test]
fn aux_revisions_coalesce_and_worker_persists_the_latest_default_and_backup() {
    let (service, root) = service_and_root("latest-revision");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = runner_with_aux_mapping(true, true);
    let mut pending = PendingPiPersistence::default();

    aux_turn(&mut runner, -1);
    let first_revision = runner.persistence_intent_at(Instant::now());
    assert!(first_revision.is_none());
    std::thread::sleep(Duration::from_millis(160));
    let first_poll = Instant::now();
    runner.mark_native_backup_issued_at(first_poll - Duration::from_secs(297));
    let first = runner.persistence_intent_at(first_poll).unwrap();
    let first_due = first_poll + Duration::from_secs(2);
    assert!(apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        first_poll,
    )
    .is_empty());
    apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        first_poll + Duration::from_millis(500),
    );
    assert!(pending
        .due_native(first_poll + Duration::from_millis(1_999))
        .is_none());
    assert_eq!(pending.snapshot_captures(), 0);
    assert!(service.native_default_write().is_none());
    assert!(!root.join("store/default.patch.json").exists());

    std::thread::sleep(Duration::from_millis(1_800));
    aux_turn(&mut runner, -1);
    let before_debounce = Instant::now();
    assert!(runner.persistence_intent_at(before_debounce).is_none());
    apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        before_debounce,
    );
    std::thread::sleep(Duration::from_millis(160));
    let latest_eligible_at = Instant::now();
    let latest = runner.persistence_intent_at(latest_eligible_at).unwrap();
    assert_ne!(latest.revision(), first.revision());
    let latest_due = latest_eligible_at + Duration::from_secs(2);
    apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        latest_eligible_at,
    );
    apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        first_due,
    );
    assert!(pending
        .due_native(latest_due - Duration::from_millis(1))
        .is_none());
    assert_eq!(pending.snapshot_captures(), 0);
    assert!(service.native_default_write().is_none());
    assert!(!root.join("store/default.patch.json").exists());

    let expected = runner
        .capture_config_snapshot()
        .into_local_patch_payload()
        .unwrap();
    assert!(apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        latest_due,
    )
    .is_empty());
    assert_eq!(
        service.native_default_write().unwrap().revision(),
        latest.revision()
    );
    assert_eq!(pending.snapshot_captures(), 1);
    let results = collect_results(&service, &mut runner, 2);
    assert!(results.iter().any(|result| matches!(
        result,
        RuntimeStoreResult::Identified { result, .. }
            if matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, is_auto: Some(true) })
    )));
    assert!(results.iter().any(|result| matches!(
        result,
        RuntimeStoreResult::Identified { result, .. }
            if matches!(result.as_ref(), RuntimeStoreResult::SaveBackupResult { ok: true })
    )));
    assert_eq!(
        super::super::platform_service_store::load_json(&root.join("store/default.patch.json"))
            .unwrap(),
        Some(expected.clone())
    );
    let backups = std::fs::read_dir(root.join("store/backups"))
        .unwrap()
        .map(|entry| {
            super::super::platform_service_store::load_json(&entry.unwrap().path())
                .unwrap()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(backups, vec![expected]);
    assert!(service.native_default_write().is_none());
    assert!(runner.persistence_intent_at(latest_due).is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn playing_edit_keeps_native_deadline_when_input_stops_transport() {
    let (service, root) = service_and_root("playing-stop");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = runner_with_aux_mapping(true, false);
    let mut pending = PendingPiPersistence::default();
    aux_turn(&mut runner, -1);
    std::thread::sleep(Duration::from_millis(160));
    let eligible_at = Instant::now();
    let intent = runner.persistence_intent_at(eligible_at).unwrap();
    let expected = runner
        .capture_config_snapshot()
        .into_local_patch_payload()
        .unwrap();
    assert!(apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        eligible_at,
    )
    .is_empty());

    input(&mut runner, json!({"type":"button_shift","pressed":true}));
    let stopped = input(&mut runner, json!({"type":"button_s","pressed":true}));
    input(&mut runner, json!({"type":"button_s","pressed":false}));
    input(&mut runner, json!({"type":"button_shift","pressed":false}));
    assert!(stopped.iter().any(|message| matches!(
        message,
        playback_runtime::RunnerMessage::RuntimeStatus { status }
            if status.transport == playback_runtime::RuntimeTransportState::Stopped
    )));
    let due_at = eligible_at + Duration::from_secs(2);
    assert!(apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        due_at - Duration::from_millis(1),
    )
    .is_empty());
    assert_eq!(pending.snapshot_captures(), 0);
    assert!(
        apply_autosave_at(&mut pending, &service, &mut playback, &mut runner, due_at,).is_empty()
    );
    assert_eq!(
        service.native_default_write().unwrap().revision(),
        intent.revision()
    );
    assert_eq!(pending.snapshot_captures(), 1);
    collect_results(&service, &mut runner, 1);
    assert_eq!(
        super::super::platform_service_store::load_json(&root.join("store/default.patch.json"))
            .unwrap(),
        Some(expected)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn older_default_completion_does_not_clear_a_newer_dirty_aux_revision() {
    let (service, root) = service_and_root("newer-dirty-revision");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = runner_with_aux_mapping(true, false);
    let mut pending = PendingPiPersistence::default();
    aux_turn(&mut runner, -1);
    std::thread::sleep(Duration::from_millis(160));
    let first_eligible = Instant::now();
    let first_intent = runner.persistence_intent_at(first_eligible).unwrap();
    apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        first_eligible,
    );
    let first_due = first_eligible + Duration::from_secs(2);
    let store_guard = service.store_lock.lock().unwrap();
    apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        first_due,
    );
    assert_eq!(
        service.native_default_write().unwrap().revision(),
        first_intent.revision()
    );

    aux_turn(&mut runner, -1);
    std::thread::sleep(Duration::from_millis(160));
    assert!(runner.persistence_intent_at(Instant::now()).is_none());
    let latest_payload = runner
        .capture_config_snapshot()
        .into_local_patch_payload()
        .unwrap();
    drop(store_guard);

    let first_completion = collect_results(&service, &mut runner, 1);
    assert!(matches!(
        &first_completion[0],
        RuntimeStoreResult::Identified { result, .. }
            if matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, .. })
    ));
    let latest_eligible = Instant::now();
    let latest_intent = runner.persistence_intent_at(latest_eligible).unwrap();
    assert_ne!(latest_intent.revision(), first_intent.revision());
    apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        latest_eligible,
    );
    let due_after_latest = latest_eligible + Duration::from_secs(2);
    apply_autosave_at(
        &mut pending,
        &service,
        &mut playback,
        &mut runner,
        due_after_latest,
    );
    assert_eq!(
        service.native_default_write().unwrap().revision(),
        latest_intent.revision()
    );
    collect_results(&service, &mut runner, 1);
    assert_eq!(
        super::super::platform_service_store::load_json(&root.join("store/default.patch.json"))
            .unwrap(),
        Some(latest_payload)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn manual_default_save_cancels_pending_coalescing_and_stays_immediate() {
    let (service, root) = service_and_root("manual-cancel");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = runner_with_aux_mapping(true, false);
    let mut pending = PendingPiPersistence::default();
    aux_turn(&mut runner, -1);
    std::thread::sleep(Duration::from_millis(160));
    let intent = runner.persistence_intent_at(Instant::now()).unwrap();
    pending.observe_native(
        Some(intent),
        Instant::now(),
        service.store_write_generation(),
        false,
    );
    assert!(pending.is_pending());

    confirm_action(&mut runner, "default.save");
    let response = take_manual_save(&mut pending, &service, &mut playback, &mut runner);
    assert!(response.is_none());
    assert!(!pending.is_pending());
    assert!(service.native_default_write().is_some());
    let result = collect_results(&service, &mut runner, 1);
    assert!(matches!(
        &result[0],
        RuntimeStoreResult::Identified { result, .. }
            if matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, is_auto: None })
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stopped_manual_default_save_is_an_immediate_native_intent() {
    let (service, root) = service_and_root("stopped-manual");
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    let mut runner = runner_with_aux_mapping_in_state(true, false, false);
    let mut pending = PendingPiPersistence::default();
    aux_turn(&mut runner, -1);
    std::thread::sleep(Duration::from_millis(160));
    let intent = runner.persistence_intent_at(Instant::now()).unwrap();
    pending.observe_native(
        Some(intent),
        Instant::now(),
        service.store_write_generation(),
        false,
    );
    assert!(pending.is_pending());

    confirm_action(&mut runner, "default.save");
    let response = take_manual_save(&mut pending, &service, &mut playback, &mut runner);
    assert!(response.is_none());
    assert!(!pending.is_pending());
    assert!(service.native_default_write().is_some());
    let result = collect_results(&service, &mut runner, 1);
    assert!(matches!(
        &result[0],
        RuntimeStoreResult::Identified { result, .. }
            if matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, is_auto: None })
    ));
    let _ = std::fs::remove_dir_all(root);
}
