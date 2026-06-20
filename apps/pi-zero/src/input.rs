use platform_core::AUX_ENCODER_COUNT;
use playback_runtime::HostMessage;
use serde_json::json;

pub enum MidiMessage {
    Realtime { bytes: Vec<u8> },
}

pub fn encoder_input_id(index: usize) -> &'static str {
    match index {
        0 => "main",
        1 => "aux1",
        2 => "aux2",
        3 => "aux3",
        _ => "main",
    }
}

pub fn encoder_index(id: &str) -> usize {
    let index = id
        .strip_prefix("encoder_aux_")
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|index| (1..=AUX_ENCODER_COUNT).contains(index))
        .unwrap_or(if id == "encoder_main" { 0 } else { usize::MAX });
    if index == usize::MAX {
        0
    } else {
        index
    }
}

pub fn encoder_turn_message(id: &str, delta: i8) -> HostMessage {
    let index = encoder_index(id);
    HostMessage::DeviceInput {
        input: json!({
            "type": "encoder_turn",
            "delta": if delta < 0 { -1 } else { 1 },
            "id": encoder_input_id(index)
        }),
    }
}

pub fn encoder_press_message(id: &str) -> HostMessage {
    let index = encoder_index(id);
    HostMessage::DeviceInput {
        input: json!({
            "type": "encoder_press",
            "id": encoder_input_id(index)
        }),
    }
}

pub fn grid_message(x: usize, y: usize, pressed: bool) -> HostMessage {
    HostMessage::DeviceInput {
        input: json!({
            "type": if pressed { "grid_press" } else { "grid_release" },
            "x": x,
            "y": y
        }),
    }
}

pub fn neokey_message(key: u8, pressed: bool) -> Option<HostMessage> {
    let input_type = match key {
        0 => "button_a",
        1 => "button_s",
        2 => "button_shift",
        3 => "button_fn",
        _ => return None,
    };
    Some(HostMessage::DeviceInput {
        input: json!({ "type": input_type, "pressed": pressed }),
    })
}

pub fn midi_realtime_message(bytes: &[u8]) -> Option<MidiMessage> {
    if bytes
        .iter()
        .any(|byte| matches!(*byte, 0xF8 | 0xFA | 0xFB | 0xFC))
    {
        return Some(MidiMessage::Realtime {
            bytes: bytes.to_vec(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neokey_maps_to_native_button_inputs() {
        let expected = ["button_a", "button_s", "button_shift", "button_fn"];
        for (key, expected_type) in expected.into_iter().enumerate() {
            let HostMessage::DeviceInput { input } = neokey_message(key as u8, true).unwrap()
            else {
                panic!("expected device input");
            };
            assert_eq!(input["type"], expected_type);
            assert_eq!(input["pressed"], true);
        }
    }

    #[test]
    fn encoders_map_to_main_and_three_aux_inputs() {
        let ids = [
            ("encoder_main", "main"),
            ("encoder_aux_1", "aux1"),
            ("encoder_aux_2", "aux2"),
            ("encoder_aux_3", "aux3"),
        ];
        for (hardware_id, input_id) in ids {
            let HostMessage::DeviceInput { input } = encoder_turn_message(hardware_id, 1) else {
                panic!("expected device input");
            };
            assert_eq!(input["type"], "encoder_turn");
            assert_eq!(input["id"], input_id);
            let HostMessage::DeviceInput { input } = encoder_press_message(hardware_id) else {
                panic!("expected device input");
            };
            assert_eq!(input["type"], "encoder_press");
            assert_eq!(input["id"], input_id);
        }
    }

    #[test]
    fn non_realtime_midi_is_ignored() {
        assert!(midi_realtime_message(&[0x90, 60, 100]).is_none());
        assert!(midi_realtime_message(&[0xF8]).is_some());
    }
}
