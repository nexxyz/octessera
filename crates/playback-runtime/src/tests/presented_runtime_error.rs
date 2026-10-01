use super::support::FakeHost;
use crate::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage, RuntimeConfig,
    RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation, RuntimeStoreResult,
};
use serde_json::{json, Value};

fn initialized_runtime() -> (PlaybackRuntime, NativeRunner, FakeHost) {
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

fn dispatch_input(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut FakeHost,
    input: Value,
    request_snapshot: bool,
) -> Vec<RunnerMessage> {
    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input,
                request_snapshot: Some(request_snapshot),
            },
            runner,
            host,
        )
        .unwrap()
        .messages
}

fn latch_error(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut FakeHost,
    operation: RuntimeOperation,
    message: &str,
) {
    latch_error_facts(
        runtime,
        runner,
        host,
        RuntimeErrorDomain::Runtime,
        RuntimeErrorCode::OperationFailed,
        operation,
        message,
    );
}

fn latch_error_facts(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut FakeHost,
    domain: RuntimeErrorDomain,
    code: RuntimeErrorCode,
    operation: RuntimeOperation,
    message: &str,
) {
    runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::RuntimeFailure {
                    error: RuntimeErrorFacts::new(domain, code, operation, Some(message.into())),
                },
            },
            runner,
            host,
        )
        .unwrap();
}

fn assert_no_control_output(messages: &[RunnerMessage]) {
    assert!(messages.iter().all(|message| !matches!(
        message,
        RunnerMessage::PlatformEffects { .. }
            | RunnerMessage::MusicalEvents { .. }
            | RunnerMessage::MidiEvents { .. }
            | RunnerMessage::AudioCommands { .. }
            | RunnerMessage::PresentedRuntimeErrorDismissed
    )));
}

#[test]
fn latched_error_gates_hidden_input_dismissal_and_next_navigation() {
    let (mut runtime, mut runner, mut host) = initialized_runtime();
    latch_error(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeOperation::StoreLoadDefault,
        "load failed",
    );
    let menu_path = runner.test_current_menu_path();
    let effects = host.effects.len();
    let hidden = dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
        false,
    );
    assert_no_control_output(&hidden);
    assert_eq!(runner.test_current_menu_path(), menu_path);
    assert_eq!(host.effects.len(), effects);
    assert!(runtime
        .last_snapshot()
        .unwrap()
        .get("runtimeError")
        .is_some());

    let dismissed = dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "button_a", "pressed": true }),
        false,
    );
    assert_no_control_output(&dismissed);
    assert!(runtime.latched_errors().is_empty());
    assert!(runtime
        .last_snapshot()
        .unwrap()
        .get("runtimeError")
        .is_none());
    assert_eq!(runner.test_current_menu_path(), menu_path);

    let cursor = runner.test_menu_cursor();
    dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
        true,
    );
    assert_ne!(runner.test_menu_cursor(), cursor);
    assert!(runtime
        .last_snapshot()
        .unwrap()
        .get("runtimeError")
        .is_none());
}

#[test]
fn wake_is_consumed_before_presented_error_dismissal() {
    let (mut runtime, mut runner, mut host) = initialized_runtime();
    latch_error(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeOperation::RuntimeDispatch,
        "dispatch failed",
    );
    runner.test_set_oled_off();

    let woke = dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "button_a", "pressed": true }),
        false,
    );
    assert_no_control_output(&woke);
    assert_eq!(runtime.latched_errors().len(), 1);
    assert!(runtime
        .last_snapshot()
        .unwrap()
        .get("runtimeError")
        .is_some());

    dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "button_a", "pressed": true }),
        false,
    );
    assert!(runtime.latched_errors().is_empty());
}

#[test]
fn held_fn_release_reconciles_physical_modifier_state_while_error_is_visible() {
    let (mut runtime, mut runner, mut host) = initialized_runtime();
    dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "button_fn", "pressed": true }),
        true,
    );
    assert_eq!(runtime.last_snapshot().unwrap()["settings"]["fnHeld"], true);
    latch_error(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeOperation::Store,
        "store failed",
    );
    runner.test_set_oled_off();

    let released = dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "button_fn", "pressed": false }),
        false,
    );
    assert_no_control_output(&released);
    assert_eq!(
        runtime.last_snapshot().unwrap()["settings"]["fnHeld"],
        false
    );
    assert_eq!(runtime.latched_errors().len(), 1);
    assert_eq!(runtime.last_snapshot().unwrap()["display"]["off"], false);

    let dismissed = dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "button_a", "pressed": true }),
        false,
    );
    assert_no_control_output(&dismissed);
    assert!(runtime.latched_errors().is_empty());

    let cursor = runner.test_menu_cursor();
    dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
        true,
    );
    assert_ne!(runner.test_menu_cursor(), cursor);
    assert!(runtime
        .last_snapshot()
        .unwrap()
        .get("runtimeError")
        .is_none());
}

fn last_snapshot(messages: &[RunnerMessage]) -> &Value {
    messages
        .iter()
        .rev()
        .find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot),
            _ => None,
        })
        .expect("snapshot message")
}

#[test]
fn concise_midi_error_remains_visible_after_dismissing_a_generic_top_error() {
    let (mut runtime, mut runner, mut host) = initialized_runtime();
    latch_error_facts(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeErrorDomain::Midi,
        RuntimeErrorCode::OperationFailed,
        RuntimeOperation::MidiListInputs,
        "MIDI unavailable",
    );
    latch_error(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeOperation::StoreLoadDefault,
        "load failed",
    );
    let menu_path = runner.test_current_menu_path();
    let cursor = runner.test_menu_cursor();

    let dismissed = dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "encoder_press", "id": "main" }),
        false,
    );
    let snapshot = last_snapshot(&dismissed);
    assert_eq!(runtime.latched_errors().len(), 1);
    assert_eq!(snapshot["runtimeError"]["operation"], "midi_list_inputs");
    assert_eq!(snapshot["display"]["title"], "MIDI INPUTS");
    assert_eq!(snapshot["display"]["lines"][0], "MIDI unavailable");

    let gated = dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
        false,
    );
    assert_no_control_output(&gated);
    assert_eq!(runtime.latched_errors().len(), 1);
    assert_eq!(runner.test_current_menu_path(), menu_path);
    assert_eq!(runner.test_menu_cursor(), cursor);

    dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "button_a", "pressed": true }),
        false,
    );
    assert!(runtime.latched_errors().is_empty());
    assert!(runtime
        .last_snapshot()
        .unwrap()
        .get("runtimeError")
        .is_none());
    assert_ne!(
        runtime.last_snapshot().unwrap()["display"]["title"],
        "MIDI INPUTS"
    );
}

#[test]
fn two_latched_errors_dismiss_one_top_error_at_a_time() {
    let (mut runtime, mut runner, mut host) = initialized_runtime();
    latch_error(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeOperation::Store,
        "first",
    );
    latch_error(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeOperation::AudioCommand,
        "second",
    );
    assert_eq!(runtime.latched_errors().len(), 2);

    dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "encoder_press", "id": "main" }),
        false,
    );
    assert_eq!(runtime.latched_errors().len(), 1);
    assert_eq!(
        runtime.latched_errors()[0].operation,
        RuntimeOperation::Store
    );
    assert_eq!(
        runtime.last_snapshot().unwrap()["runtimeError"]["message"],
        "first"
    );

    dispatch_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "button_a", "pressed": true }),
        false,
    );
    assert!(runtime.latched_errors().is_empty());
    assert!(runtime
        .last_snapshot()
        .unwrap()
        .get("runtimeError")
        .is_none());
}
