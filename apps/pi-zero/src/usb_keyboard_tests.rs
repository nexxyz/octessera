use super::*;
use serde_json::json;
use std::path::PathBuf;

fn input_of(input: KeyboardInput) -> Value {
    match input {
        KeyboardInput::EncoderTurn { id, delta } => json!({
            "type": "encoder_turn",
            "delta": delta,
            "id": encoder_input_id(id)
        }),
        KeyboardInput::EncoderPress { id } => {
            json!({ "type": "encoder_press", "id": encoder_input_id(id) })
        }
        KeyboardInput::ButtonA(pressed) => json!({ "type": "button_a", "pressed": pressed }),
        KeyboardInput::ButtonS(pressed) => json!({ "type": "button_s", "pressed": pressed }),
        KeyboardInput::ButtonShift(pressed) => {
            json!({ "type": "button_shift", "pressed": pressed })
        }
        KeyboardInput::ButtonFn(pressed) => json!({ "type": "button_fn", "pressed": pressed }),
    }
}

fn encoder_input_id(id: &str) -> &str {
    match id {
        "encoder_main" => "main",
        "encoder_aux_1" => "aux1",
        "encoder_aux_2" => "aux2",
        "encoder_aux_3" => "aux3",
        _ => panic!("unexpected encoder id"),
    }
}

#[test]
fn usb_host_keyboard_mapping_matches_native_input_contract() {
    let expected = [
        (
            KeyboardKey::Left,
            1,
            json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
        ),
        (
            KeyboardKey::Up,
            1,
            json!({ "type": "encoder_turn", "delta": -1, "id": "main" }),
        ),
        (
            KeyboardKey::Right,
            1,
            json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        ),
        (
            KeyboardKey::Down,
            1,
            json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        ),
        (
            KeyboardKey::Q,
            1,
            json!({ "type": "encoder_turn", "delta": -1, "id": "aux1" }),
        ),
        (
            KeyboardKey::W,
            1,
            json!({ "type": "encoder_press", "id": "aux1" }),
        ),
        (
            KeyboardKey::E,
            1,
            json!({ "type": "encoder_turn", "delta": 1, "id": "aux1" }),
        ),
        (
            KeyboardKey::A,
            1,
            json!({ "type": "encoder_turn", "delta": -1, "id": "aux2" }),
        ),
        (
            KeyboardKey::S,
            1,
            json!({ "type": "encoder_press", "id": "aux2" }),
        ),
        (
            KeyboardKey::D,
            1,
            json!({ "type": "encoder_turn", "delta": 1, "id": "aux2" }),
        ),
        (
            KeyboardKey::Z,
            1,
            json!({ "type": "encoder_turn", "delta": -1, "id": "aux3" }),
        ),
        (
            KeyboardKey::X,
            1,
            json!({ "type": "encoder_press", "id": "aux3" }),
        ),
        (
            KeyboardKey::C,
            1,
            json!({ "type": "encoder_turn", "delta": 1, "id": "aux3" }),
        ),
        (
            KeyboardKey::Enter,
            1,
            json!({ "type": "encoder_press", "id": "main" }),
        ),
        (
            KeyboardKey::Backspace,
            1,
            json!({ "type": "button_a", "pressed": true }),
        ),
        (
            KeyboardKey::Backspace,
            0,
            json!({ "type": "button_a", "pressed": false }),
        ),
        (
            KeyboardKey::Escape,
            1,
            json!({ "type": "button_a", "pressed": true }),
        ),
        (
            KeyboardKey::Space,
            1,
            json!({ "type": "button_s", "pressed": true }),
        ),
        (
            KeyboardKey::Space,
            0,
            json!({ "type": "button_s", "pressed": false }),
        ),
        (
            KeyboardKey::LeftShift,
            1,
            json!({ "type": "button_shift", "pressed": true }),
        ),
        (
            KeyboardKey::LeftShift,
            0,
            json!({ "type": "button_shift", "pressed": false }),
        ),
        (
            KeyboardKey::LeftControl,
            1,
            json!({ "type": "button_fn", "pressed": true }),
        ),
        (
            KeyboardKey::LeftControl,
            0,
            json!({ "type": "button_fn", "pressed": false }),
        ),
    ];
    for (key, value, expected) in expected {
        assert_eq!(map_key_event(key, value).map(input_of), Some(expected));
    }
}

#[test]
fn arrows_repeat_and_edge_controls_suppress_repeat() {
    for (key, id, delta) in [
        (KeyboardKey::Left, "encoder_main", -1),
        (KeyboardKey::Right, "encoder_main", 1),
        (KeyboardKey::Q, "encoder_aux_1", -1),
        (KeyboardKey::E, "encoder_aux_1", 1),
        (KeyboardKey::A, "encoder_aux_2", -1),
        (KeyboardKey::D, "encoder_aux_2", 1),
        (KeyboardKey::Z, "encoder_aux_3", -1),
        (KeyboardKey::C, "encoder_aux_3", 1),
    ] {
        assert_eq!(
            map_key_event(key, 2),
            Some(KeyboardInput::EncoderTurn { id, delta })
        );
    }
    for key in [
        KeyboardKey::Enter,
        KeyboardKey::W,
        KeyboardKey::S,
        KeyboardKey::X,
        KeyboardKey::Backspace,
        KeyboardKey::Space,
        KeyboardKey::LeftShift,
        KeyboardKey::LeftControl,
    ] {
        assert_eq!(map_key_event(key, 2), None);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn evdev_binds_aux3_left_to_physical_bottom_left_key_z() {
    assert_eq!(
        super::evdev::evdev_key(KeyboardKey::Z),
        evdev::KeyCode::KEY_Z
    );
    assert_eq!(
        super::evdev::keyboard_key(evdev::KeyCode::KEY_Z.code()),
        Some(KeyboardKey::Z)
    );
}

#[test]
fn aux_clicks_emit_once_until_release_and_clear_on_cleanup() {
    for (key, id) in [
        (KeyboardKey::W, "encoder_aux_1"),
        (KeyboardKey::S, "encoder_aux_2"),
        (KeyboardKey::X, "encoder_aux_3"),
    ] {
        let mut state = KeyboardState::default();
        let pressed = state.prepare(key, 1).unwrap();
        assert_eq!(pressed.input, Some(KeyboardInput::EncoderPress { id }));
        state.commit(pressed);
        assert!(state.prepare(key, 2).is_none());
        let released = state.prepare(key, 0).unwrap();
        assert_eq!(released.input, None);
        state.commit(released);
        assert_eq!(state.prepare(key, 0), None);
        assert_eq!(
            state.prepare(key, 1).unwrap().input,
            Some(KeyboardInput::EncoderPress { id })
        );

        let (input_tx, input_rx) = std::sync::mpsc::channel();
        state.commit(state.prepare(key, 1).unwrap());
        assert!(release_held_inputs(&mut state, &input_tx));
        assert!(release_held_inputs(&mut state, &input_tx));
        assert!(input_rx.try_recv().is_err());
        assert_eq!(
            state.prepare(key, 1).unwrap().input,
            Some(KeyboardInput::EncoderPress { id })
        );
    }
}

#[test]
fn shift_space_keeps_shift_and_space_order_and_meaning() {
    let mut state = KeyboardState::default();
    let shift = state.prepare(KeyboardKey::LeftShift, 1).unwrap();
    assert_eq!(shift.input, Some(KeyboardInput::ButtonShift(true)));
    state.commit(shift);
    let space = state.prepare(KeyboardKey::Space, 1).unwrap();
    assert_eq!(space.input, Some(KeyboardInput::ButtonS(true)));
}

#[test]
fn gate_allows_only_graphical_hdmi_snapshots_and_honors_board_role() {
    for mode in [
        "live-grid",
        "plain-grid",
        "active-behavior",
        "cycle-behaviors",
    ] {
        assert!(capture_enabled_for_snapshot(
            true,
            &json!({ "hdmi": { "mode": mode } })
        ));
    }
    for snapshot in [
        json!({}),
        json!({ "hdmi": {} }),
        json!({ "hdmi": { "mode": null } }),
        json!({ "hdmi": { "mode": "none" } }),
        json!({ "hdmi": { "mode": "terminal" } }),
        json!({ "hdmi": { "mode": "unknown" } }),
        json!({ "hdmi": [] }),
    ] {
        assert!(!capture_enabled_for_snapshot(true, &snapshot));
    }
    assert!(!capture_enabled_for_snapshot(
        false,
        &json!({
            "hdmi": { "mode": "live-grid" }
        })
    ));
}

#[test]
fn control_transitions_wake_and_update_gate() {
    let control = KeyboardCaptureControl::new(true);
    assert!(!control.is_enabled());
    control.observe_snapshot(&json!({ "hdmi": { "mode": "live-grid" } }));
    assert!(control.is_enabled());
    control.observe_snapshot(&json!({ "hdmi": { "mode": "none" } }));
    assert!(!control.is_enabled());
}

#[test]
fn held_release_order_is_stable_and_clears_once() {
    let mut state = KeyboardState::default();
    for key in [
        KeyboardKey::Backspace,
        KeyboardKey::Space,
        KeyboardKey::W,
        KeyboardKey::S,
        KeyboardKey::X,
        KeyboardKey::LeftShift,
        KeyboardKey::LeftControl,
    ] {
        let prepared = state.prepare(key, 1).unwrap();
        state.commit(prepared);
    }
    assert_eq!(
        state
            .held_releases()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
        vec![
            KeyboardInput::ButtonA(false),
            KeyboardInput::ButtonS(false),
            KeyboardInput::ButtonShift(false),
            KeyboardInput::ButtonFn(false),
        ]
    );
    state.clear();
    assert!(state
        .held_releases()
        .into_iter()
        .all(|input| input.is_none()));
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
fn duplicate_modifier_keys_release_only_after_both_are_up() {
    let mut state = KeyboardState::default();
    state.commit(state.prepare(KeyboardKey::LeftShift, 1).unwrap());
    assert_eq!(
        state.prepare(KeyboardKey::RightShift, 1).unwrap().input,
        None
    );
    state.commit(state.prepare(KeyboardKey::RightShift, 1).unwrap());
    let release = state.prepare(KeyboardKey::LeftShift, 0).unwrap();
    assert_eq!(release.input, None);
    state.commit(release);
    assert_eq!(
        state.prepare(KeyboardKey::RightShift, 0).unwrap().input,
        Some(KeyboardInput::ButtonShift(false))
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
