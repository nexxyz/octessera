use super::*;
use serde_json::json;
use std::path::PathBuf;

#[test]
fn gate_allows_only_graphical_hdmi_snapshots() {
    for mode in [
        "live-grid",
        "plain-grid",
        "active-behavior",
        "cycle-behaviors",
    ] {
        assert!(hdmi_snapshot_allows_capture(
            &json!({ "hdmi": { "mode": mode } })
        ));
    }
    for snapshot in [
        json!({}),
        json!({ "hdmi": {} }),
        json!({ "hdmi": { "mode": null } }),
        json!({ "hdmi": { "mode": "terminal" } }),
        json!({ "hdmi": { "mode": "terminal" } }),
        json!({ "hdmi": { "mode": "unknown" } }),
        json!({ "hdmi": [] }),
    ] {
        assert!(!hdmi_snapshot_allows_capture(&snapshot));
    }
}

#[test]
fn bluetooth_lets_a_board_without_usb_host_capture_keyboards() {
    let control = KeyboardCaptureControl::new(false);
    control.observe_hdmi_mode(playback_runtime::NativeHdmiMode::LiveGrid);
    assert!(!control.is_enabled());
    assert_eq!(control.buses(), KeyboardBuses::default());

    control.observe_bluetooth(true);
    assert!(control.is_enabled());
    assert_eq!(
        control.buses(),
        KeyboardBuses {
            usb: false,
            bluetooth: true
        }
    );

    control.observe_hdmi_mode(playback_runtime::NativeHdmiMode::Terminal);
    assert!(
        !control.is_enabled(),
        "terminal mode hands keyboards to the console"
    );
}

#[test]
fn usb_host_role_turns_usb_keyboard_capture_on_and_off_live() {
    let control = KeyboardCaptureControl::new(false);
    control.observe_hdmi_mode(playback_runtime::NativeHdmiMode::LiveGrid);
    assert!(!control.is_enabled());

    control.observe_usb_host(true);
    assert!(control.is_enabled());
    assert!(control.buses().usb);

    control.observe_usb_host(false);
    assert!(!control.is_enabled());
    assert_eq!(control.buses(), KeyboardBuses::default());
}

#[test]
fn control_transitions_wake_and_update_gate() {
    let control = KeyboardCaptureControl::new(true);
    assert!(!control.is_enabled());
    control.observe_snapshot(&json!({ "hdmi": { "mode": "live-grid" } }));
    assert!(control.is_enabled());
    control.observe_snapshot(&json!({ "hdmi": { "mode": "terminal" } }));
    assert!(!control.is_enabled());
}

#[test]
fn typed_hdmi_modes_keep_keyboard_capture_gate_in_lockstep() {
    let control = KeyboardCaptureControl::new(true);
    for mode in [
        playback_runtime::NativeHdmiMode::LiveGrid,
        playback_runtime::NativeHdmiMode::PlainGrid,
        playback_runtime::NativeHdmiMode::ActiveBehavior,
        playback_runtime::NativeHdmiMode::CycleBehaviors,
    ] {
        control.observe_hdmi_mode(mode);
        assert!(control.is_enabled());
    }
    control.observe_hdmi_mode(playback_runtime::NativeHdmiMode::Terminal);
    assert!(!control.is_enabled());
    let disabled = KeyboardCaptureControl::new(false);
    disabled.observe_hdmi_mode(playback_runtime::NativeHdmiMode::LiveGrid);
    assert!(!disabled.is_enabled());
}

#[test]
fn release_held_inputs_emits_ordered_releases_once_and_is_idempotent() {
    let mut state = KeyboardState::default();
    for key in [
        KeyboardKey::Backspace,
        KeyboardKey::Space,
        KeyboardKey::LeftShift,
        KeyboardKey::LeftControl,
    ] {
        state.commit(state.prepare(key, 1).unwrap());
    }
    let (input_tx, input_rx) = std::sync::mpsc::channel();
    assert!(release_held_inputs(&mut state, &input_tx));
    assert!(release_held_inputs(&mut state, &input_tx));
    let messages = input_rx
        .try_iter()
        .filter_map(|message| match message {
            HostMessage::DeviceInput { input, .. } => Some(input),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        vec![
            json!({ "type": "button_a", "pressed": false }),
            json!({ "type": "button_s", "pressed": false }),
            json!({ "type": "button_shift", "pressed": false }),
            json!({ "type": "button_fn", "pressed": false }),
        ]
    );
}

#[test]
fn event_nodes_are_selected_by_lowest_numeric_suffix() {
    let paths = [
        PathBuf::from("/dev/input/event12"),
        PathBuf::from("/dev/input/event3"),
        PathBuf::from("/dev/input/event1"),
        PathBuf::from("/dev/input/event4-extra"),
        PathBuf::from("/dev/input/mouse0"),
    ];
    assert_eq!(
        sorted_event_nodes(paths),
        vec![
            PathBuf::from("/dev/input/event1"),
            PathBuf::from("/dev/input/event3"),
            PathBuf::from("/dev/input/event12"),
        ]
    );
}

#[test]
fn owning_guard_shutdown_joins_without_physical_evdev() {
    let (_tx, rx) = std::sync::mpsc::channel();
    let capture = KeyboardCapture::spawn(_tx, true);
    drop(rx);
    capture.shutdown().unwrap();
}
