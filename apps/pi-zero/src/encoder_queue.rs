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
    deltas: [i16; 4],
}

impl PendingEncoderTurns {
    pub fn enqueue(&mut self, id: &str, delta: i8) {
        let index = ENCODER_IDS
            .iter()
            .position(|candidate| *candidate == id)
            .unwrap_or(0);
        self.deltas[index] = (self.deltas[index] + i16::from(delta)).clamp(-127, 127);
    }

    pub fn take_messages(&mut self) -> Vec<HostMessage> {
        let mut messages = Vec::new();
        for (index, delta) in self.deltas.iter_mut().enumerate() {
            if *delta == 0 {
                continue;
            }
            messages.push(encoder_turn_message(ENCODER_IDS[index], *delta as i8));
            *delta = 0;
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
        let HostMessage::DeviceInput { input } = &messages[0] else {
            panic!("expected device input");
        };
        assert_eq!(input["id"], "main");
        assert_eq!(input["delta"], 2);
        let HostMessage::DeviceInput { input } = &messages[1] else {
            panic!("expected device input");
        };
        assert_eq!(input["id"], "aux1");
        assert_eq!(input["delta"], -3);
        assert!(pending.take_messages().is_empty());
    }
}
