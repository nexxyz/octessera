use super::*;
use crate::RuntimePlatformRequest;

const SAVE_OPTIONS: [&str; 3] = ["Cancel", "Save this one", "Save System"];
const APP_RESTART_OPTIONS: [&str; 2] = ["Continue", "Restart now"];

fn acknowledge_save(runner: &mut NativeRunner, request_id: &str) {
    let payload = runner
        .restart_settings
        .pending_write_payload()
        .expect("System Apply payload");
    runner.register_platform_request(&RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreSaveSystem {
            payload: payload.as_ref().clone(),
        },
        request_id.into(),
        None,
    ));
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveSystemResult { ok: true }
                .with_identity(request_id.into(), None),
        })
        .unwrap();
}

fn perf_mode_save_choice() -> NativeRunner {
    let mut runner = optimization_capable_runner();
    runner.menu.focus_item_key("sound.optimizeFor");
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    runner
}

fn perf_mode_restart_choice() -> NativeRunner {
    let mut runner = perf_mode_save_choice();
    runner.display.confirm_dialog.as_mut().unwrap().cursor = 1;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    acknowledge_save(&mut runner, "perf-mode-save");
    runner
}

fn assert_dialog_snapshot(
    runner: &mut NativeRunner,
    body: &[&str],
    options: &[&str],
    cursor: usize,
) {
    runner.display.confirm_dialog.as_mut().unwrap().cursor = cursor;
    let snapshot = runner.snapshot().unwrap();
    let mut expected = body
        .iter()
        .map(|line| (*line).to_string())
        .collect::<Vec<_>>();
    expected.extend(
        options
            .iter()
            .enumerate()
            .map(|(index, option)| format!("{} {option}", if index == cursor { ">" } else { " " })),
    );
    let lines = snapshot["display"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(lines, expected);
    assert_eq!(snapshot["selectedRow"], json!(body.len() + cursor));
    assert!(
        lines.iter().all(|line| line.chars().count() <= 20),
        "{body:?}"
    );
}

#[test]
fn perf_mode_save_snapshot_renders_all_options_at_each_cursor() {
    let mut runner = perf_mode_save_choice();
    for cursor in 0..SAVE_OPTIONS.len() {
        assert_dialog_snapshot(&mut runner, &["Restart required."], &SAVE_OPTIONS, cursor);
    }
}

#[test]
fn perf_mode_restart_snapshot_offers_app_restart_at_each_cursor() {
    let mut runner = perf_mode_restart_choice();
    for cursor in 0..APP_RESTART_OPTIONS.len() {
        assert_dialog_snapshot(
            &mut runner,
            &["System saved.", "Restart the app", "to apply?"],
            &APP_RESTART_OPTIONS,
            cursor,
        );
    }
}
