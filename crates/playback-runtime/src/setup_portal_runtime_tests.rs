use super::support::FakeHost;
use crate::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage, RuntimeConfig,
    RuntimeSetupPortalErrorCode, RuntimeSetupPortalPhase, RuntimeSetupPortalStatus,
    RuntimeStoreResult,
};
use serde_json::{json, Value};

fn send_input(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut FakeHost,
    input: Value,
) -> Vec<RunnerMessage> {
    runtime
        .dispatch_host_message(
            HostMessage::DeviceInput {
                input,
                request_snapshot: Some(true),
            },
            runner,
            host,
        )
        .unwrap()
        .messages
}

fn navigate_to_configure_wifi(
    runtime: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut FakeHost,
) {
    runner.skip_startup_splash();
    for _ in 0..16 {
        if runner.test_current_menu_label().as_deref() == Some("System") {
            break;
        }
        send_input(
            runtime,
            runner,
            host,
            json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
        );
    }
    assert_eq!(runner.test_current_menu_label().as_deref(), Some("System"));
    send_input(
        runtime,
        runner,
        host,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    for _ in 0..32 {
        if runner.test_current_menu_label().as_deref() == Some("Configure WiFi") {
            break;
        }
        send_input(
            runtime,
            runner,
            host,
            json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
        );
    }
    assert_eq!(
        runner.test_current_menu_label().as_deref(),
        Some("Configure WiFi")
    );
}

fn identity_layers(result: &RuntimeStoreResult) -> usize {
    match result {
        RuntimeStoreResult::Identified { result, .. } => 1 + identity_layers(result),
        _ => 0,
    }
}

#[test]
fn identified_setup_portal_failure_uses_the_native_failed_modal() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let mut host = FakeHost {
        setup_portal_result: Some(RuntimeStoreResult::SetupPortalStatus {
            status: RuntimeSetupPortalStatus {
                phase: RuntimeSetupPortalPhase::Failed,
                disposition: None,
                portal_suffix: None,
                reboot_required: false,
                error_code: Some(RuntimeSetupPortalErrorCode::OperationFailed),
            },
        }),
        ..FakeHost::default()
    };
    let initial = runner.messages_with_snapshot().unwrap();
    runtime
        .dispatch_runner_messages(initial, &mut runner, &mut host)
        .unwrap();

    navigate_to_configure_wifi(&mut runtime, &mut runner, &mut host);
    send_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "encoder_press", "id": "main" }),
    );
    send_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "encoder_turn", "id": "main", "delta": 1 }),
    );
    send_input(
        &mut runtime,
        &mut runner,
        &mut host,
        json!({ "type": "encoder_press", "id": "main" }),
    );

    let result = host
        .setup_portal_results_sent
        .first()
        .expect("identified setup result");
    assert_eq!(identity_layers(result), 1);
    assert!(host
        .effects
        .iter()
        .any(|effect| matches!(effect, crate::RuntimePlatformEffect::SetupPortalOpen)));
    let snapshot = runtime.last_snapshot().expect("presented snapshot");
    assert_eq!(snapshot["display"]["title"], "Wi-Fi Setup");
    assert_eq!(snapshot["display"]["lines"][0], "Setup failed");
    assert!(snapshot.get("runtimeError").is_none());
    assert!(runtime.latched_errors().is_empty());
}
