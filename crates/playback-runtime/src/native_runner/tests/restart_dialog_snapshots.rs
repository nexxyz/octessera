use super::*;

const HOST_SAVE_LINES: [&str; 4] = [
    "Restart required.",
    "Before Host reboot:",
    "Unplug computer USB.",
    "Audio/MIDI/SD2 off.",
];
const HOST_RESTART_LINES: [&str; 5] = [
    "Saved. Reboot to",
    "apply?",
    "Unplug computer USB",
    "before Host reboot.",
    "Audio/MIDI/SD2 off.",
];
const SAVE_OPTIONS: [&str; 3] = ["Cancel", "Save this setting", "Save everything"];
const RESTART_OPTIONS: [&str; 2] = ["Continue", "Reboot now"];

fn host_runner(auto_save_default: bool) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        usb_data_role_available: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.auto_save_default = auto_save_default;
    runner.menu.rebuild(runner.menu_config());
    runner.menu.set_enum_value_for_key("usb.dataRole", "Host");
    assert!(runner.apply_runtime_menu_key_fast("usb.dataRole").unwrap());
    runner
}

fn acknowledge_save(runner: &mut NativeRunner, request_id: &str) {
    let revision = runner.restart_settings.pending_write_revision().unwrap();
    runner.register_default_write_request(request_id, Some(revision));
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::Identified {
                result: Box::new(RuntimeStoreResult::SaveDefaultResult {
                    ok: true,
                    is_auto: None,
                }),
                request_id: request_id.into(),
                revision: Some(revision),
            },
        })
        .unwrap();
}

fn manual_host_restart_choice() -> NativeRunner {
    let mut runner = host_runner(false);
    runner.display.confirm_dialog.as_mut().unwrap().cursor = 1;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    acknowledge_save(&mut runner, "host-manual-save");
    runner
}

fn auto_host_restart_choice() -> NativeRunner {
    let mut runner = host_runner(true);
    runner.messages_with_snapshot().unwrap();
    acknowledge_save(&mut runner, "host-auto-save");
    runner
}

fn non_host_save_choice() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.menu.focus_item_key("sound.audioOutputBufferFrames");
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

fn non_host_restart_choice() -> NativeRunner {
    let mut runner = non_host_save_choice();
    runner.display.confirm_dialog.as_mut().unwrap().cursor = 1;
    runner
        .send(HostMessage::DeviceInput {
            input: json!({ "type": "encoder_press", "id": "main" }),
            request_snapshot: None,
        })
        .unwrap();
    acknowledge_save(&mut runner, "non-host-save");
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
fn host_save_snapshot_renders_all_options_and_markers_at_each_cursor() {
    let mut runner = host_runner(false);
    for cursor in 0..SAVE_OPTIONS.len() {
        assert_dialog_snapshot(&mut runner, &HOST_SAVE_LINES, &SAVE_OPTIONS, cursor);
    }
}

#[test]
fn host_restart_snapshot_renders_all_options_and_markers_at_each_cursor() {
    let mut runner = manual_host_restart_choice();
    assert_eq!(runner.usb_data_role, UsbDataRole::Host);
    for cursor in 0..RESTART_OPTIONS.len() {
        assert_dialog_snapshot(&mut runner, &HOST_RESTART_LINES, &RESTART_OPTIONS, cursor);
    }
}

#[test]
fn auto_save_host_restart_snapshot_includes_post_save_warning() {
    let mut runner = auto_host_restart_choice();
    assert_eq!(runner.usb_data_role, UsbDataRole::Host);
    assert_dialog_snapshot(&mut runner, &HOST_RESTART_LINES, &RESTART_OPTIONS, 0);
}

#[test]
fn non_host_save_snapshot_keeps_existing_dialog_at_each_cursor() {
    let mut runner = non_host_save_choice();
    for cursor in 0..SAVE_OPTIONS.len() {
        assert_dialog_snapshot(&mut runner, &["Restart required."], &SAVE_OPTIONS, cursor);
    }
}

#[test]
fn non_host_restart_snapshot_keeps_existing_dialog_at_each_cursor() {
    let mut runner = non_host_restart_choice();
    for cursor in 0..RESTART_OPTIONS.len() {
        assert_dialog_snapshot(
            &mut runner,
            &["Saved. Reboot to", "apply?"],
            &RESTART_OPTIONS,
            cursor,
        );
    }
}
