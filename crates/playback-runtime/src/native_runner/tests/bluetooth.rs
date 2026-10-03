use super::*;
use crate::{
    RuntimeBluetoothDevice, RuntimeBluetoothDeviceAction, RuntimeBluetoothDeviceKind,
    RuntimeBluetoothPairing, RuntimeBluetoothStatus,
};

fn board_runner() -> NativeRunner {
    NativeRunner::new(NativeRunnerConfig {
        jack_audio_required: true,
        ..NativeRunnerConfig::default()
    })
    .unwrap()
}

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

fn press(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    input(runner, json!({ "type": "encoder_press", "id": "main" }))
}

fn back(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    input(runner, json!({ "type": "button_a", "pressed": true }))
}

fn effects(messages: &[RunnerMessage]) -> Vec<RuntimePlatformEffect> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => effects.clone(),
            _ => Vec::new(),
        })
        .collect()
}

fn device(address: &str, name: &str, paired: bool, connected: bool) -> RuntimeBluetoothDevice {
    RuntimeBluetoothDevice {
        address: address.into(),
        name: name.into(),
        kind: RuntimeBluetoothDeviceKind::Keyboard,
        paired,
        connected,
    }
}

fn send_status(runner: &mut NativeRunner, status: RuntimeBluetoothStatus) {
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::BluetoothStatus { status },
        })
        .unwrap();
}

fn turn_bluetooth_on(runner: &mut NativeRunner) {
    assert!(runner.menu.focus_item_key("bluetooth.enabled"));
    press(runner);
    input(
        runner,
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
    );
    press(runner);
}

#[test]
fn bluetooth_setting_reveals_device_pages_and_is_saved_with_the_system() {
    let mut runner = board_runner();
    assert!(!runner.menu.focus_item_key("bluetooth.pair:AA"));
    turn_bluetooth_on(&mut runner);

    assert!(runner.bluetooth.enabled);
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["bluetooth"],
        json!({ "enabled": true })
    );
    let system =
        super::super::system_persistence::SystemPersistenceState::system_document(&runner).unwrap();
    assert_eq!(
        system["runtimeConfig"]["bluetooth"],
        json!({ "enabled": true })
    );
}

#[test]
fn desktop_has_no_bluetooth_page() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    assert!(!runner.menu.focus_item_key("bluetooth.enabled"));
}

#[test]
fn pair_new_opens_and_scans_before_anything_is_found() {
    let mut runner = board_runner();
    turn_bluetooth_on(&mut runner);
    assert!(runner.menu.focus_item_key("bluetooth.pair"));

    let entered = press(&mut runner);
    assert_eq!(
        effects(&entered),
        vec![RuntimePlatformEffect::BluetoothScan { active: true }]
    );
    assert!(runner.in_bluetooth_pair_page());
    assert_eq!(
        effects(&back(&mut runner)),
        vec![RuntimePlatformEffect::BluetoothScan { active: false }]
    );
}

#[test]
fn pair_new_scans_while_open_and_pairs_a_found_keyboard() {
    let mut runner = board_runner();
    turn_bluetooth_on(&mut runner);
    send_status(
        &mut runner,
        RuntimeBluetoothStatus {
            powered: true,
            devices: vec![
                device("AA", "Keys", false, false),
                RuntimeBluetoothDevice {
                    kind: RuntimeBluetoothDeviceKind::Audio,
                    ..device("CC", "Speaker", false, false)
                },
            ],
            ..RuntimeBluetoothStatus::default()
        },
    );

    assert!(!runner.menu.focus_item_key("bluetooth.pair:CC"));
    assert!(runner.menu.focus_item_key("bluetooth.pair:AA"));
    runner.menu.back();
    let entered = press(&mut runner);
    assert_eq!(
        effects(&entered),
        vec![RuntimePlatformEffect::BluetoothScan { active: true }]
    );

    let paired = press(&mut runner);
    assert_eq!(
        effects(&paired),
        vec![RuntimePlatformEffect::BluetoothDevice {
            address: "AA".into(),
            action: RuntimeBluetoothDeviceAction::Pair,
        }]
    );

    let left = back(&mut runner);
    assert_eq!(
        effects(&left),
        vec![RuntimePlatformEffect::BluetoothScan { active: false }]
    );
}

#[test]
fn paired_device_rows_toggle_the_connection_and_forget() {
    let mut runner = board_runner();
    turn_bluetooth_on(&mut runner);
    send_status(
        &mut runner,
        RuntimeBluetoothStatus {
            powered: true,
            devices: vec![device("AA", "Keys", true, true)],
            ..RuntimeBluetoothStatus::default()
        },
    );

    assert!(runner.menu.focus_item_key("bluetooth.toggle:AA"));
    assert_eq!(
        effects(&press(&mut runner)),
        vec![RuntimePlatformEffect::BluetoothDevice {
            address: "AA".into(),
            action: RuntimeBluetoothDeviceAction::Disconnect,
        }]
    );
    assert!(runner.menu.focus_item_key("bluetooth.forget:AA"));
    assert_eq!(
        effects(&press(&mut runner)),
        vec![RuntimePlatformEffect::BluetoothDevice {
            address: "AA".into(),
            action: RuntimeBluetoothDeviceAction::Forget,
        }]
    );
}

#[test]
fn pairing_code_takes_over_the_display_until_cancelled() {
    let mut runner = board_runner();
    turn_bluetooth_on(&mut runner);
    send_status(
        &mut runner,
        RuntimeBluetoothStatus {
            powered: true,
            pairing: Some(RuntimeBluetoothPairing {
                address: "AA".into(),
                name: "Keys".into(),
                code: Some("123456".into()),
            }),
            ..RuntimeBluetoothStatus::default()
        },
    );
    let display = &runner.snapshot().unwrap()["display"];
    assert_eq!(display["title"], "Bluetooth");
    assert!(display["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line == "123456 + Enter"));

    assert_eq!(
        effects(&back(&mut runner)),
        vec![RuntimePlatformEffect::BluetoothDevice {
            address: "AA".into(),
            action: RuntimeBluetoothDeviceAction::CancelPairing,
        }]
    );
    assert_ne!(runner.snapshot().unwrap()["display"]["title"], "Bluetooth");
}

#[test]
fn status_message_is_shown_once_as_a_toast() {
    let mut runner = board_runner();
    runner.display.oled_splash_until = None;
    send_status(
        &mut runner,
        RuntimeBluetoothStatus {
            message: Some("Paired Keys".into()),
            ..RuntimeBluetoothStatus::default()
        },
    );
    assert_eq!(
        runner
            .display
            .toast
            .as_ref()
            .map(|toast| toast.message.as_str()),
        Some("Paired Keys")
    );
    assert_eq!(
        runner.bluetooth.status.as_ref().unwrap().message,
        None,
        "the message is consumed so a later rebuild does not repeat it"
    );
}
