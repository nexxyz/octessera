use super::support::FakeHost;
use crate::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig,
    RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation, RuntimeStoreResult,
};
use serde_json::{json, Value};

fn runtime() -> (PlaybackRuntime, NativeRunner, FakeHost) {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let mut host = FakeHost::default();
    runtime
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut host,
        )
        .unwrap();
    (runtime, runner, host)
}

fn host_result(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut FakeHost,
    result: RuntimeStoreResult,
    id: &str,
) {
    runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: result.with_identity(id.into(), Some(1)),
            },
            runner,
            host,
        )
        .unwrap();
}

fn autosave_failure() -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Storage,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::StoreSaveDefault,
            Some("No space left on device".into()),
        ),
    }
}

fn input(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut FakeHost,
    input: Value,
) {
    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input,
                request_snapshot: Some(false),
            },
            runner,
            host,
        )
        .unwrap();
}

fn shows_error(runtime: &PlaybackRuntime) -> bool {
    runtime
        .last_snapshot()
        .unwrap()
        .get("runtimeError")
        .is_some()
}

#[test]
fn a_retried_autosave_failure_latches_once_and_menu_input_works_until_a_save_succeeds() {
    let (mut runtime, mut runner, mut host) = runtime();
    host_result(
        &mut runtime,
        &mut runner,
        &mut host,
        autosave_failure(),
        "auto-1",
    );
    assert_eq!(runtime.latched_errors().len(), 1);
    assert!(shows_error(&runtime));
    input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(runtime.latched_errors().is_empty());

    host_result(
        &mut runtime,
        &mut runner,
        &mut host,
        autosave_failure(),
        "auto-2",
    );
    host_result(
        &mut runtime,
        &mut runner,
        &mut host,
        autosave_failure(),
        "auto-3",
    );
    assert!(runtime.latched_errors().is_empty());
    assert!(!shows_error(&runtime));
    let cursor = runner.test_menu_cursor();
    input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    assert_ne!(runner.test_menu_cursor(), cursor);

    host_result(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeStoreResult::SaveDefaultResult {
            ok: true,
            is_auto: Some(true),
        },
        "auto-4",
    );
    host_result(
        &mut runtime,
        &mut runner,
        &mut host,
        autosave_failure(),
        "auto-5",
    );
    assert_eq!(runtime.latched_errors().len(), 1);
    assert!(shows_error(&runtime));
}

#[test]
fn a_latched_error_over_the_restore_question_takes_main_first_and_then_shows_the_question() {
    let (mut runtime, mut runner, mut host) = runtime();
    let confirm = RuntimeStoreResult::UserDataTransferStatus {
        status: crate::RuntimeUserDataTransferStatus {
            phase: crate::RuntimeUserDataTransferPhase::ConfirmRestore,
            url: Some("http://192.168.42.1:8081".into()),
            code: Some("Ab2Cd3Ef4H".into()),
            expires_in_seconds: Some(900),
        },
    };
    host_result(&mut runtime, &mut runner, &mut host, confirm, "transfer");
    host_result(
        &mut runtime,
        &mut runner,
        &mut host,
        autosave_failure(),
        "auto-1",
    );
    assert!(shows_error(&runtime));

    input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(runtime.latched_errors().is_empty());
    let snapshot = runtime.last_snapshot().unwrap();
    assert!(snapshot.get("runtimeError").is_none());
    assert_eq!(snapshot["display"]["title"], "Restore backup?");
}

#[test]
fn a_patch_save_run_by_hand_latches_its_failure_again() {
    let (mut runtime, mut runner, mut host) = runtime();
    host_result(
        &mut runtime,
        &mut runner,
        &mut host,
        autosave_failure(),
        "auto-1",
    );
    input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    assert!(runtime.latched_errors().is_empty());

    runtime
        .dispatch_runner_messages(
            vec![crate::RunnerMessage::PlatformEffects {
                effects: vec![crate::RuntimePlatformEffect::StoreSaveDefault {
                    payload: json!({}),
                    mode: None,
                }],
            }],
            &mut runner,
            &mut host,
        )
        .unwrap();
    host_result(
        &mut runtime,
        &mut runner,
        &mut host,
        autosave_failure(),
        "manual",
    );
    assert_eq!(runtime.latched_errors().len(), 1);
}
