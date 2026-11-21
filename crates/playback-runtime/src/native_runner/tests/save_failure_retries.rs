use super::*;

fn send_save_result(runner: &mut NativeRunner, id: &str, result: RuntimeStoreResult) -> Value {
    let revision = Some(runner.config_revision);
    runner.register_default_write_request(id, revision);
    let messages = runner
        .send(HostMessage::RuntimeResult {
            result: result.with_identity(id.into(), revision),
        })
        .unwrap();
    runner.messages_with_snapshot().unwrap();
    snapshot_from(&messages)
}

fn failed_default_save() -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: crate::RuntimeErrorFacts::new(
            crate::RuntimeErrorDomain::Storage,
            crate::RuntimeErrorCode::OperationFailed,
            crate::RuntimeOperation::StoreSaveDefault,
            Some("No space left on device".into()),
        ),
    }
}

fn press_main(runner: &mut NativeRunner) {
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
}

#[test]
fn a_retried_autosave_failure_shows_once_until_a_save_works_or_the_player_saves() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();

    let first = send_save_result(&mut runner, "auto-1", failed_default_save());
    assert_eq!(first["display"]["title"], "CAN'T SAVE");
    press_main(&mut runner);
    assert!(runner.display.runtime_error_presentation.is_none());

    send_save_result(&mut runner, "auto-2", failed_default_save());
    assert!(runner.display.runtime_error_presentation.is_none());

    send_save_result(
        &mut runner,
        "auto-3",
        RuntimeStoreResult::SaveDefaultResult {
            ok: true,
            is_auto: Some(true),
        },
    );
    let after_success = send_save_result(&mut runner, "auto-4", failed_default_save());
    assert_eq!(after_success["display"]["title"], "CAN'T SAVE");
    press_main(&mut runner);

    super::in_place_menu_actions::confirm_key(&mut runner, "default.save");
    let manual = send_save_result(&mut runner, "manual", failed_default_save());
    assert_eq!(manual["display"]["title"], "CAN'T SAVE");
}
