use super::*;
use serde_json::json;

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
        KeyboardInput::Grid { x, y, pressed } => json!({
            "type": if pressed { "grid_press" } else { "grid_release" },
            "x": x,
            "y": y
        }),
    }
}

const fn aux(encoder: u8, slot: u8) -> KeyboardKey {
    KeyboardKey::Aux { encoder, slot }
}

const fn grid(column: u8, row: u8) -> KeyboardKey {
    KeyboardKey::Grid { column, row }
}

fn press(state: &mut KeyboardState, key: KeyboardKey, value: i32) -> Option<KeyboardInput> {
    let prepared = state.prepare(key, value)?;
    state.commit(prepared);
    prepared.input
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
            aux(1, 0),
            1,
            json!({ "type": "encoder_turn", "delta": -1, "id": "aux1" }),
        ),
        (
            aux(1, 1),
            1,
            json!({ "type": "encoder_press", "id": "aux1" }),
        ),
        (
            aux(1, 2),
            1,
            json!({ "type": "encoder_turn", "delta": 1, "id": "aux1" }),
        ),
        (
            aux(2, 0),
            1,
            json!({ "type": "encoder_turn", "delta": -1, "id": "aux2" }),
        ),
        (
            aux(2, 1),
            1,
            json!({ "type": "encoder_press", "id": "aux2" }),
        ),
        (
            aux(2, 2),
            1,
            json!({ "type": "encoder_turn", "delta": 1, "id": "aux2" }),
        ),
        (
            aux(3, 0),
            1,
            json!({ "type": "encoder_turn", "delta": -1, "id": "aux3" }),
        ),
        (
            aux(3, 1),
            1,
            json!({ "type": "encoder_press", "id": "aux3" }),
        ),
        (
            aux(3, 2),
            1,
            json!({ "type": "encoder_turn", "delta": 1, "id": "aux3" }),
        ),
        (
            KeyboardKey::KeypadEnter,
            1,
            json!({ "type": "encoder_press", "id": "main" }),
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
        (aux(1, 0), "encoder_aux_1", -1),
        (aux(1, 2), "encoder_aux_1", 1),
        (aux(2, 0), "encoder_aux_2", -1),
        (aux(2, 2), "encoder_aux_2", 1),
        (aux(3, 0), "encoder_aux_3", -1),
        (aux(3, 2), "encoder_aux_3", 1),
    ] {
        assert_eq!(
            map_key_event(key, 2),
            Some(KeyboardInput::EncoderTurn { id, delta })
        );
    }
    for key in [
        KeyboardKey::Enter,
        KeyboardKey::KeypadEnter,
        aux(1, 1),
        aux(2, 1),
        aux(3, 1),
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
fn evdev_codes_bind_controls_and_grid_by_key_position() {
    use ::evdev::KeyCode;
    let expected = [
        (KeyCode::KEY_1, aux(1, 0)),
        (KeyCode::KEY_2, aux(1, 1)),
        (KeyCode::KEY_3, aux(1, 2)),
        (KeyCode::KEY_Q, aux(2, 0)),
        (KeyCode::KEY_W, aux(2, 1)),
        (KeyCode::KEY_E, aux(2, 2)),
        (KeyCode::KEY_A, aux(3, 0)),
        (KeyCode::KEY_S, aux(3, 1)),
        (KeyCode::KEY_D, aux(3, 2)),
        (KeyCode::KEY_C, grid(0, 0)),
        (KeyCode::KEY_SLASH, grid(7, 0)),
        (KeyCode::KEY_F, grid(0, 1)),
        (KeyCode::KEY_SEMICOLON, grid(6, 1)),
        (KeyCode::KEY_APOSTROPHE, grid(7, 1)),
        (KeyCode::KEY_R, grid(0, 2)),
        (KeyCode::KEY_Y, grid(2, 2)),
        (KeyCode::KEY_LEFTBRACE, grid(7, 2)),
        (KeyCode::KEY_4, grid(0, 3)),
        (KeyCode::KEY_0, grid(6, 3)),
        (KeyCode::KEY_MINUS, grid(7, 3)),
        (KeyCode::KEY_ENTER, KeyboardKey::Enter),
        (KeyCode::KEY_KPENTER, KeyboardKey::KeypadEnter),
        (KeyCode::KEY_LEFTALT, KeyboardKey::LeftAlt),
    ];
    for (code, key) in expected {
        assert_eq!(super::evdev::keyboard_key(code.code()), Some(key));
    }
    for code in [
        KeyCode::KEY_Z,
        KeyCode::KEY_X,
        KeyCode::KEY_102ND,
        KeyCode::KEY_RIGHTALT,
    ] {
        assert_eq!(super::evdev::keyboard_key(code.code()), None);
    }
    for key in super::worker::required_keys().chain([KeyboardKey::KeypadEnter]) {
        let code = super::evdev::evdev_key(key).code();
        assert_eq!(super::evdev::keyboard_key(code), Some(key));
    }
}

#[test]
fn grid_keys_press_bottom_half_plain_and_top_half_with_left_alt() {
    let mut state = KeyboardState::default();
    assert_eq!(
        press(&mut state, grid(0, 0), 1),
        Some(KeyboardInput::Grid {
            x: 0,
            y: 0,
            pressed: true
        })
    );
    assert_eq!(press(&mut state, KeyboardKey::LeftAlt, 1), None);
    assert_eq!(
        press(&mut state, grid(7, 3), 1),
        Some(KeyboardInput::Grid {
            x: 7,
            y: 7,
            pressed: true
        })
    );
    assert_eq!(press(&mut state, KeyboardKey::LeftAlt, 0), None);
    assert_eq!(
        press(&mut state, grid(3, 2), 1),
        Some(KeyboardInput::Grid {
            x: 3,
            y: 2,
            pressed: true
        })
    );
}

#[test]
fn grid_release_goes_to_the_pressed_tile_after_alt_changes() {
    let mut state = KeyboardState::default();
    press(&mut state, grid(1, 1), 1);
    press(&mut state, KeyboardKey::LeftAlt, 1);
    assert_eq!(
        press(&mut state, grid(1, 1), 0),
        Some(KeyboardInput::Grid {
            x: 1,
            y: 1,
            pressed: false
        })
    );
    press(&mut state, grid(2, 0), 1);
    press(&mut state, KeyboardKey::LeftAlt, 0);
    assert_eq!(
        press(&mut state, grid(2, 0), 0),
        Some(KeyboardInput::Grid {
            x: 2,
            y: 4,
            pressed: false
        })
    );
    assert_eq!(state.prepare(grid(2, 0), 0), None);
}

#[test]
fn grid_key_repeat_is_ignored() {
    let mut state = KeyboardState::default();
    press(&mut state, grid(4, 3), 1);
    assert_eq!(state.prepare(grid(4, 3), 2), None);
    assert_eq!(state.prepare(grid(4, 3), 1), None);
}

#[test]
fn keyboard_grid_press_sends_the_neotrellis_grid_message() {
    let device_input = |message| match message {
        HostMessage::DeviceInput { input, .. } => input,
        _ => panic!("grid input is device input"),
    };
    let mut state = KeyboardState::default();
    press(&mut state, KeyboardKey::LeftAlt, 1);
    let pressed = press(&mut state, grid(5, 2), 1).unwrap();
    assert_eq!(
        device_input(host_message(pressed)),
        device_input(crate::input::grid_message(5, 6, true))
    );
    let released = press(&mut state, grid(5, 2), 0).unwrap();
    assert_eq!(
        device_input(host_message(released)),
        device_input(crate::input::grid_message(5, 6, false))
    );
}

#[test]
fn aux_clicks_emit_once_until_release_and_clear_on_cleanup() {
    for (key, id) in [
        (aux(1, 1), "encoder_aux_1"),
        (aux(2, 1), "encoder_aux_2"),
        (aux(3, 1), "encoder_aux_3"),
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
fn held_release_order_is_stable_and_clears_once() {
    let mut state = KeyboardState::default();
    for key in [
        KeyboardKey::Backspace,
        grid(6, 1),
        KeyboardKey::Space,
        aux(1, 1),
        aux(2, 1),
        aux(3, 1),
        KeyboardKey::LeftShift,
        KeyboardKey::LeftAlt,
        grid(0, 3),
        KeyboardKey::LeftControl,
    ] {
        let prepared = state.prepare(key, 1).unwrap();
        state.commit(prepared);
    }
    assert_eq!(
        state.held_releases(),
        vec![
            KeyboardInput::Grid {
                x: 6,
                y: 1,
                pressed: false
            },
            KeyboardInput::Grid {
                x: 0,
                y: 7,
                pressed: false
            },
            KeyboardInput::ButtonA(false),
            KeyboardInput::ButtonS(false),
            KeyboardInput::ButtonShift(false),
            KeyboardInput::ButtonFn(false),
        ]
    );
    state.clear();
    assert!(state.held_releases().is_empty());
    assert_eq!(
        state.prepare(grid(0, 3), 1).unwrap().input,
        Some(KeyboardInput::Grid {
            x: 0,
            y: 3,
            pressed: true
        })
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
