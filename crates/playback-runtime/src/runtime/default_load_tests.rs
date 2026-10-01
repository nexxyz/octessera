use super::restore_handoff_tests::{documents, runtime_runner};
use crate::{
    CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage,
    RuntimeConfig, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimePlatformEffect, RuntimeStoreResult,
};
use serde_json::json;

#[test]
fn ordinary_default_load_does_not_ack_restore() {
    let (mut runtime, mut runner, mut host) = runtime_runner();
    let (_, mut patch) = documents();
    patch["runtimeConfig"]["transport"]["bpm"] = json!(127.0);
    patch["runtimeConfig"]["bpm"] = json!(127.0);
    let request = runtime.next_platform_request(RuntimePlatformEffect::StoreLoadDefault);
    runner.register_platform_request(&request);
    runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadDefaultResult {
                    payload: Some(patch),
                }
                .with_identity(request.request_id, request.revision),
            },
            &mut runner,
            &mut host,
        )
        .unwrap();

    assert_eq!(runtime.config().bpm, 127.0);
    assert_eq!(host.acknowledge_count, 0);
}

#[test]
fn ordinary_invalid_default_load_returns_typed_failure_without_clearing_store_error() {
    let (mut runtime, mut runner, mut host) = runtime_runner();
    runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::RuntimeFailure {
                    error: RuntimeErrorFacts::new(
                        RuntimeErrorDomain::Storage,
                        RuntimeErrorCode::OperationFailed,
                        RuntimeOperation::StoreListPresets,
                        Some("previous list failure".into()),
                    )
                    .with_identity(Some("previous-store-error".into()), Some(8)),
                },
            },
            &mut runner,
            &mut host,
        )
        .unwrap();
    let before = runner.capture_config_snapshot().into_payload();
    let mut patch = documents().1;
    patch["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] =
        json!("C:/invalid/local.wav");
    let request = runtime.next_platform_request(RuntimePlatformEffect::StoreLoadDefault);
    runner.register_platform_request(&request);

    let output = runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadDefaultResult {
                    payload: Some(patch),
                }
                .with_identity(request.request_id, request.revision),
            },
            &mut runner,
            &mut host,
        )
        .unwrap();

    let error = runtime.last_status().unwrap().error.as_ref().unwrap();
    assert_eq!(error.domain, RuntimeErrorDomain::Storage);
    assert_eq!(error.operation, RuntimeOperation::StoreLoadDefault);
    assert_eq!(error.recovery, crate::RuntimeRecovery::RetainLastGood);
    assert_eq!(runner.capture_config_snapshot().into_payload(), before);
    assert_eq!(host.acknowledge_count, 0);
    assert_eq!(host.inner.silence_calls, 0);
    assert!(output.messages.iter().any(|message| matches!(
        message,
        RunnerMessage::Snapshot { snapshot }
            if snapshot["runtimeError"]["operation"] == "store_load_default"
    )));

    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input: json!({ "type": "encoder_press", "id": "main" }),
                request_snapshot: None,
            },
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
            .operation,
        RuntimeOperation::StoreListPresets
    );
}

#[test]
fn native_runner_send_and_runtime_handoff_share_local_patch_rejection() {
    let (_, mut runner, _) = runtime_runner();
    let before = runner.capture_config_snapshot().into_payload();
    let mut patch = documents().1;
    patch["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] =
        json!("C:/invalid/local.wav");

    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(patch),
            }
            .with_identity("direct-native-default-load".into(), Some(29)),
        })
        .unwrap();

    assert_eq!(runner.capture_config_snapshot().into_payload(), before);
    assert!(messages.iter().any(|message| matches!(
        message,
        RunnerMessage::Snapshot { snapshot }
            if snapshot["display"]["title"] == "RUNTIME ERROR"
    )));
    assert!(!messages.iter().any(|message| matches!(
        message,
        RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(
                effect,
                RuntimePlatformEffect::MidiPanic
                    | RuntimePlatformEffect::StoreSaveDefault { .. }
            ))
    )));

    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut fresh_runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut host = crate::tests::support::FakeHost::default();
    let request = runtime.next_platform_request(RuntimePlatformEffect::StoreLoadDefault);
    fresh_runner.register_platform_request(&request);
    let mut invalid_patch = documents().1;
    invalid_patch["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] =
        json!("C:/invalid/local.wav");
    let ingest = runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadDefaultResult {
                    payload: Some(invalid_patch),
                }
                .with_identity(request.request_id, request.revision),
            },
            &mut fresh_runner,
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
            .operation,
        RuntimeOperation::StoreLoadDefault
    );
    assert!(ingest.messages.iter().any(|message| matches!(
        message,
        RunnerMessage::Snapshot { snapshot }
            if snapshot["runtimeError"]["operation"] == "store_load_default"
    )));
}
