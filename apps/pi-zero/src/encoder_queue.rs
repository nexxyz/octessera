use crate::input::encoder_turn_message;
use playback_runtime::HostMessage;

const ENCODER_IDS: [&str; 4] = [
    "encoder_main",
    "encoder_aux_1",
    "encoder_aux_2",
    "encoder_aux_3",
];

#[derive(Default)]
pub struct PendingEncoderTurns {
    turns: Vec<(usize, i16)>,
}

impl PendingEncoderTurns {
    pub fn enqueue(&mut self, id: &str, delta: i8) {
        let index = ENCODER_IDS
            .iter()
            .position(|candidate| *candidate == id)
            .unwrap_or(0);
        let delta = i16::from(delta);
        if let Some((last_index, last_delta)) = self.turns.last_mut() {
            if *last_index == index && last_delta.signum() == delta.signum() {
                *last_delta = (*last_delta + delta).clamp(-127, 127);
                return;
            }
        }
        self.turns.push((index, delta.clamp(-127, 127)));
    }

    pub fn take_messages(&mut self) -> Vec<HostMessage> {
        let turns = std::mem::take(&mut self.turns);
        let mut messages = Vec::with_capacity(turns.len());
        for (index, delta) in turns {
            if delta == 0 {
                continue;
            }
            messages.push(encoder_turn_message(ENCODER_IDS[index], delta as i8));
        }
        messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesces_turns_per_encoder() {
        let mut pending = PendingEncoderTurns::default();
        pending.enqueue("encoder_main", 1);
        pending.enqueue("encoder_main", 1);
        pending.enqueue("encoder_aux_1", -3);

        let messages = pending.take_messages();

        assert_eq!(messages.len(), 2);
        let HostMessage::DeviceInput { input, .. } = &messages[0] else {
            panic!("expected device input");
        };
        assert_eq!(input["id"], "main");
        assert_eq!(input["delta"], 2);
        let HostMessage::DeviceInput { input, .. } = &messages[1] else {
            panic!("expected device input");
        };
        assert_eq!(input["id"], "aux1");
        assert_eq!(input["delta"], -3);
        assert!(pending.take_messages().is_empty());
    }

    #[test]
    fn preserves_direction_reversals_for_main_and_aux_encoders() {
        let mut pending = PendingEncoderTurns::default();
        pending.enqueue("encoder_main", 1);
        pending.enqueue("encoder_main", -1);
        pending.enqueue("encoder_aux_2", -1);
        pending.enqueue("encoder_aux_2", 1);

        let messages = pending.take_messages();

        assert_eq!(messages.len(), 4);
        assert_turn(&messages[0], "main", 1);
        assert_turn(&messages[1], "main", -1);
        assert_turn(&messages[2], "aux2", -1);
        assert_turn(&messages[3], "aux2", 1);
    }

    fn assert_turn(message: &HostMessage, id: &str, delta: i8) {
        let HostMessage::DeviceInput { input, .. } = message else {
            panic!("expected device input");
        };
        assert_eq!(input["id"], id);
        assert_eq!(input["delta"], delta);
    }
}
