use crate::input::{encoder_index, neokey_message};
use playback_runtime::HostMessage;

// TEMPORARY NEOKEY HARDWARE HACK.
// Remove this file and the `temporary_neokey_hack` call sites when the NeoKey is replaced.
pub(crate) const ENABLED: bool = true;

#[derive(Default)]
pub(crate) struct TemporaryNeoKeyHack {
    shift_pressed: bool,
    fn_pressed: bool,
}

impl TemporaryNeoKeyHack {
    pub(crate) fn encoder_turn_messages(
        &mut self,
        id: &str,
        delta: i8,
    ) -> Option<Vec<HostMessage>> {
        if !ENABLED || delta == 0 || encoder_index(id) != 1 {
            return None;
        }
        Some(button_tap(1))
    }

    pub(crate) fn encoder_press_messages(&mut self, id: &str) -> Option<Vec<HostMessage>> {
        if !ENABLED {
            return None;
        }
        match encoder_index(id) {
            1 => Some(button_tap(0)),
            2 => set_modifier(2, &mut self.shift_pressed, true),
            3 => set_modifier(3, &mut self.fn_pressed, true),
            _ => None,
        }
    }

    pub(crate) fn encoder_release_messages(&mut self, id: &str) -> Option<Vec<HostMessage>> {
        if !ENABLED {
            return None;
        }
        match encoder_index(id) {
            2 => set_modifier(2, &mut self.shift_pressed, false),
            3 => set_modifier(3, &mut self.fn_pressed, false),
            _ => None,
        }
    }
}

pub(crate) fn allow_missing_neokey() -> bool {
    ENABLED
}

fn button_tap(key: u8) -> Vec<HostMessage> {
    [true, false]
        .into_iter()
        .filter_map(|pressed| neokey_message(key, pressed))
        .collect()
}

fn set_modifier(key: u8, current: &mut bool, pressed: bool) -> Option<Vec<HostMessage>> {
    if *current == pressed {
        return Some(Vec::new());
    }
    *current = pressed;
    Some(vec![
        neokey_message(key, pressed).expect("temporary NeoKey hack uses valid NeoKey indices")
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aux_encoder_hack_maps_to_neokey_buttons() {
        let mut hack = TemporaryNeoKeyHack::default();
        assert_button_pair(
            hack.encoder_turn_messages("encoder_aux_1", 1).unwrap(),
            "button_s",
        );
        assert_button_pair(
            hack.encoder_press_messages("encoder_aux_1").unwrap(),
            "button_a",
        );
        assert_modifier(
            hack.encoder_press_messages("encoder_aux_2").unwrap(),
            "button_shift",
            true,
        );
        assert_modifier(
            hack.encoder_release_messages("encoder_aux_2").unwrap(),
            "button_shift",
            false,
        );
        assert_modifier(
            hack.encoder_press_messages("encoder_aux_3").unwrap(),
            "button_fn",
            true,
        );
    }

    #[test]
    fn aux_encoder_modifiers_can_be_held_together_and_released_independently() {
        let mut hack = TemporaryNeoKeyHack::default();

        assert_modifier(
            hack.encoder_press_messages("encoder_aux_2").unwrap(),
            "button_shift",
            true,
        );
        assert_modifier(
            hack.encoder_press_messages("encoder_aux_3").unwrap(),
            "button_fn",
            true,
        );
        assert_modifier(
            hack.encoder_release_messages("encoder_aux_2").unwrap(),
            "button_shift",
            false,
        );
        assert_modifier(
            hack.encoder_release_messages("encoder_aux_3").unwrap(),
            "button_fn",
            false,
        );
    }

    #[test]
    fn repeated_modifier_edges_do_not_emit_duplicate_messages() {
        let mut hack = TemporaryNeoKeyHack::default();

        assert_modifier(
            hack.encoder_press_messages("encoder_aux_2").unwrap(),
            "button_shift",
            true,
        );
        assert!(hack
            .encoder_press_messages("encoder_aux_2")
            .unwrap()
            .is_empty());
        assert_modifier(
            hack.encoder_release_messages("encoder_aux_2").unwrap(),
            "button_shift",
            false,
        );
        assert!(hack
            .encoder_release_messages("encoder_aux_2")
            .unwrap()
            .is_empty());
    }

    fn assert_button_pair(messages: Vec<HostMessage>, expected_type: &str) {
        assert_eq!(messages.len(), 2);
        assert_device_input(&messages[0], expected_type, true);
        assert_device_input(&messages[1], expected_type, false);
    }

    fn assert_modifier(messages: Vec<HostMessage>, expected_type: &str, pressed: bool) {
        assert_eq!(messages.len(), 1);
        assert_device_input(&messages[0], expected_type, pressed);
    }

    fn assert_device_input(message: &HostMessage, expected_type: &str, pressed: bool) {
        let HostMessage::DeviceInput { input, .. } = message else {
            panic!("expected device input");
        };
        assert_eq!(input["type"], expected_type);
        assert_eq!(input["pressed"], pressed);
    }
}
