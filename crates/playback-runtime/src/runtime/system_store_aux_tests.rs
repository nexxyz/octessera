use crate::tests::support::FakeHost;
use crate::{
    CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeErrorDomain,
    RuntimeOperation, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeRecovery,
    RuntimeStoreResult,
};
use platform_core::NoteBehavior;
use serde_json::{json, Value};

#[test]
fn system_aux_bindings_load_reaches_outer_status_as_typed_native_failure() {
    let full: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let documents = crate::split_system_patch_documents(&full).unwrap();
    let mut system_with_aux = documents.system;
    system_with_aux["runtimeConfig"]["shiftAuxBindings"] =
        json!({ "aux2": { "turnKey": "displayBrightness" } });

    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "keys".into(),
        note_behaviors: vec![NoteBehavior::Hold; 16],
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.apply_config_payload(full).unwrap();
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
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadSystem,
        "system-aux-load".into(),
        Some(23),
    );
    runner.register_platform_request(&request);

    let output = runtime
        .dispatch_host_message(
            HostMessage::RuntimeResult {
                result: RuntimeStoreResult::LoadSystemResult {
                    payload: Some(system_with_aux),
                }
                .with_identity(request.request_id.clone(), request.revision),
            },
            &mut runner,
            &mut host,
        )
        .unwrap();

    let error = runtime.last_status().unwrap().error.as_ref().unwrap();
    assert_eq!(error.domain, RuntimeErrorDomain::Storage);
    assert_eq!(error.operation, RuntimeOperation::StoreLoadSystem);
    assert_eq!(error.request_id.as_deref(), Some("system-aux-load"));
    assert_eq!(error.revision, Some(23));
    assert_eq!(error.recovery, RuntimeRecovery::RetainLastGood);
    assert!(output.messages.iter().any(|message| matches!(message,
        crate::RunnerMessage::Snapshot { snapshot }
            if snapshot["runtimeError"]["operation"] == "store_load_system"
                && snapshot["runtimeError"]["requestId"] == "system-aux-load"
    )));
}
