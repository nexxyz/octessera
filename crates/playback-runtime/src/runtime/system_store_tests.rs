use crate::tests::support::FakeHost;
use crate::{
    CoreRunner, HostMessage, MusicalEvent, NativeRunner, NativeRunnerConfig, PlaybackRuntime,
    RunnerMessage, RuntimeDispatchInput, RuntimeErrorCode, RuntimeErrorDomain, RuntimeOperation,
    RuntimePlatformEffect, RuntimePlatformRequest, RuntimeRecovery, RuntimeStoreResult,
    RuntimeTransportState,
};
use platform_core::NoteBehavior;
use serde_json::{json, Value};

fn runtime_with_runner() -> (PlaybackRuntime, NativeRunner, FakeHost) {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        note_behaviors: vec![NoteBehavior::Hold; 16],
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.skip_startup_splash();
    let mut runtime = PlaybackRuntime::new(crate::RuntimeConfig::default());
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

fn system_document() -> Value {
    let config: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let mut system = crate::split_system_patch_documents(&config).unwrap().system;
    system["runtimeConfig"]["displayBrightness"] = json!(29);
    system
}

fn register_request(
    runner: &mut NativeRunner,
    effect: RuntimePlatformEffect,
    request_id: &str,
    revision: Option<u64>,
) -> RuntimePlatformRequest {
    let request = RuntimePlatformRequest::new(effect, request_id.into(), revision);
    runner.register_platform_request(&request);
    request
}

fn identified(result: RuntimeStoreResult, request: &RuntimePlatformRequest) -> HostMessage {
    HostMessage::RuntimeResult {
        result: result.with_identity(request.request_id.clone(), request.revision),
    }
}

fn dispatch_result(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut FakeHost,
    result: RuntimeStoreResult,
    request: &RuntimePlatformRequest,
) {
    runtime
        .dispatch_host_message(identified(result, request), runner, host)
        .unwrap();
}

#[test]
fn invalid_system_load_latches_typed_error_after_native_rejection_without_disturbing_playback() {
    let (mut runtime, mut runner, mut host) = runtime_with_runner();
    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({ "type": "button_s", "pressed": true }),
                request_snapshot: None,
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({ "type": "grid_press", "x": 2, "y": 3 }),
                request_snapshot: None,
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    let before = runtime.last_status().unwrap().clone();
    assert_eq!(before.transport, RuntimeTransportState::Playing);
    let musical_events_before = host.musical_events.clone();

    let request = register_request(
        &mut runner,
        RuntimePlatformEffect::StoreLoadSystem,
        "load-missing-system",
        Some(18),
    );
    let output = runtime
        .dispatch_host_message(
            identified(
                RuntimeStoreResult::LoadSystemResult { payload: None },
                &request,
            ),
            &mut runner,
            &mut host,
        )
        .unwrap();

    let error = runtime.last_status().unwrap().error.as_ref().unwrap();
    assert_eq!(error.domain, RuntimeErrorDomain::Storage);
    assert_eq!(error.code, RuntimeErrorCode::OperationFailed);
    assert_eq!(error.operation, RuntimeOperation::StoreLoadSystem);
    assert_eq!(error.request_id.as_deref(), Some("load-missing-system"));
    assert_eq!(error.revision, Some(18));
    assert_eq!(error.recovery, RuntimeRecovery::RetainLastGood);
    assert_eq!(runtime.last_status().unwrap().transport, before.transport);
    assert_eq!(
        runtime.last_status().unwrap().current_ppqn_pulse,
        before.current_ppqn_pulse
    );
    assert_eq!(host.musical_events, musical_events_before);
    assert!(host
        .musical_events
        .iter()
        .any(|event| matches!(event, MusicalEvent::NoteOn { .. })));
    assert_eq!(
        runtime.last_snapshot().unwrap()["runtimeError"]["operation"],
        "store_load_system"
    );
    assert!(output.messages.iter().any(|message| matches!(message,
        RunnerMessage::Snapshot { snapshot }
            if snapshot["runtimeError"]["requestId"] == "load-missing-system"
                && snapshot["display"]["lines"].as_array().is_some_and(|lines|
                    lines.iter().any(|line| line.as_str().is_some_and(|line| line.contains("store load"))))
    )));

    runtime
        .dispatch(
            RuntimeDispatchInput::HostMessage(HostMessage::TransportPulseStep {
                pulses: 0,
                source: crate::SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            }),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_eq!(
        runtime
            .last_status()
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .request_id
            .as_deref(),
        Some("load-missing-system")
    );
}

#[test]
fn stale_and_wrong_revision_system_results_are_ignored() {
    let (mut runtime, mut runner, mut host) = runtime_with_runner();
    let first = register_request(
        &mut runner,
        RuntimePlatformEffect::StoreLoadSystem,
        "system-old",
        Some(2),
    );
    dispatch_result(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeStoreResult::LoadSystemResult { payload: None },
        &first,
    );

    let current = register_request(
        &mut runner,
        RuntimePlatformEffect::StoreLoadSystem,
        "system-current",
        Some(3),
    );
    let before = runtime.last_snapshot().unwrap().clone();
    let wrong_revision = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadSystem,
        current.request_id.clone(),
        Some(4),
    );
    for (request, message) in [(&first, "stale"), (&wrong_revision, "wrong revision")] {
        dispatch_result(
            &mut runtime,
            &mut runner,
            &mut host,
            RuntimeStoreResult::LoadSystemResult {
                payload: Some(system_document()),
            },
            request,
        );
        dispatch_result(
            &mut runtime,
            &mut runner,
            &mut host,
            RuntimeStoreResult::RuntimeFailure {
                error: crate::RuntimeErrorFacts::new(
                    RuntimeErrorDomain::Storage,
                    RuntimeErrorCode::OperationFailed,
                    RuntimeOperation::StoreLoadSystem,
                    Some(message.into()),
                ),
            },
            request,
        );
    }
    assert_eq!(runtime.last_snapshot().unwrap(), &before);
    assert_eq!(
        runtime
            .last_status()
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .request_id
            .as_deref(),
        Some("system-old")
    );

    dispatch_result(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeStoreResult::LoadSystemResult {
            payload: Some(system_document()),
        },
        &current,
    );
    dispatch_result(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeStoreResult::LoadSystemResult { payload: None },
        &wrong_revision,
    );
    assert_eq!(
        runtime.last_snapshot().unwrap()["settings"]["displayBrightness"],
        29
    );
    assert_eq!(
        runtime
            .last_status()
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .request_id
            .as_deref(),
        Some("system-old")
    );
}

#[test]
fn valid_system_load_applies_once_and_duplicate_cannot_clear_newer_failure() {
    let (mut runtime, mut runner, mut host) = runtime_with_runner();
    let current = register_request(
        &mut runner,
        RuntimePlatformEffect::StoreLoadSystem,
        "system-current",
        Some(3),
    );
    dispatch_result(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeStoreResult::LoadSystemResult {
            payload: Some(system_document()),
        },
        &current,
    );
    assert!(runtime.last_status().unwrap().error.is_none());
    assert_eq!(
        runtime.last_snapshot().unwrap()["settings"]["displayBrightness"],
        29
    );
    let applied = runtime.last_snapshot().unwrap().clone();
    let mut duplicate = system_document();
    duplicate["runtimeConfig"]["displayBrightness"] = json!(55);
    dispatch_result(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeStoreResult::LoadSystemResult {
            payload: Some(duplicate),
        },
        &current,
    );
    assert_eq!(runtime.last_snapshot().unwrap(), &applied);
    let newer = register_request(
        &mut runner,
        RuntimePlatformEffect::StoreLoadSystem,
        "system-newer-failure",
        Some(5),
    );
    dispatch_result(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeStoreResult::LoadSystemResult { payload: None },
        &newer,
    );
    let after_failure = runtime.last_snapshot().unwrap().clone();
    dispatch_result(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeStoreResult::LoadSystemResult {
            payload: Some(system_document()),
        },
        &current,
    );
    dispatch_result(
        &mut runtime,
        &mut runner,
        &mut host,
        RuntimeStoreResult::LoadSystemResult { payload: None },
        &newer,
    );
    assert_eq!(runtime.last_snapshot().unwrap(), &after_failure);
    assert_ne!(runtime.last_snapshot().unwrap(), &applied);
    assert_eq!(
        runtime
            .last_status()
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .request_id
            .as_deref(),
        Some("system-newer-failure")
    );
}

#[test]
fn identified_system_runtime_failure_preserves_typed_error_facts() {
    let (mut runtime, mut runner, mut host) = runtime_with_runner();
    let request = register_request(
        &mut runner,
        RuntimePlatformEffect::StoreSaveSystem {
            payload: system_document(),
        },
        "system-save-adapter-failure",
        Some(7),
    );
    let mut facts = crate::RuntimeErrorFacts::new(
        RuntimeErrorDomain::Storage,
        RuntimeErrorCode::InvalidPayload,
        RuntimeOperation::StoreSaveSystem,
        Some("invalid system bytes".into()),
    );
    facts.request_id = Some("host-provided-id".into());
    facts.revision = Some(99);
    let output = runtime
        .dispatch_host_message(
            identified(
                RuntimeStoreResult::RuntimeFailure { error: facts },
                &request,
            ),
            &mut runner,
            &mut host,
        )
        .unwrap();

    let error = runtime.last_status().unwrap().error.as_ref().unwrap();
    assert_eq!(error.domain, RuntimeErrorDomain::Storage);
    assert_eq!(error.code, RuntimeErrorCode::InvalidPayload);
    assert_eq!(error.operation, RuntimeOperation::StoreSaveSystem);
    assert_eq!(
        error.request_id.as_deref(),
        Some("system-save-adapter-failure")
    );
    assert_eq!(error.revision, Some(7));
    assert!(output.messages.iter().any(|message| matches!(message,
        RunnerMessage::Snapshot { snapshot }
            if snapshot["runtimeError"]["code"] == "invalid_payload"
                && snapshot["runtimeError"]["requestId"] == "system-save-adapter-failure"
    )));
}

#[test]
fn system_save_results_leave_a_pending_default_restart_request_available() {
    for ok in [true, false] {
        let (mut runtime, mut runner, mut host) = runtime_with_runner();
        let default_request = register_request(
            &mut runner,
            RuntimePlatformEffect::StoreSaveDefault {
                payload: json!({ "revision": 41 }),
                mode: Some("restart-setting".into()),
            },
            "pending-default-restart",
            Some(41),
        );
        let system_request = register_request(
            &mut runner,
            RuntimePlatformEffect::StoreSaveSystem {
                payload: system_document(),
            },
            "system-save-result",
            Some(42),
        );
        runtime
            .dispatch_host_message(
                identified(RuntimeStoreResult::SaveSystemResult { ok }, &system_request),
                &mut runner,
                &mut host,
            )
            .unwrap();
        if !ok {
            let error = runtime.last_status().unwrap().error.as_ref().unwrap();
            assert_eq!(error.operation, RuntimeOperation::StoreSaveSystem);
            assert_eq!(error.request_id.as_deref(), Some("system-save-result"));
        }
        runtime
            .dispatch_host_message(
                identified(
                    RuntimeStoreResult::SaveDefaultResult {
                        ok: true,
                        is_auto: None,
                    },
                    &default_request,
                ),
                &mut runner,
                &mut host,
            )
            .unwrap();
        if ok {
            assert!(runtime.last_status().unwrap().error.is_none());
        } else {
            assert_eq!(
                runtime
                    .last_status()
                    .unwrap()
                    .error
                    .as_ref()
                    .unwrap()
                    .operation,
                RuntimeOperation::StoreSaveSystem
            );
        }
    }
}
