use crate::input::encoder_turn_message;
use octessera_hal::encoder_gpio::HardwareEvent;
use playback_runtime::HostMessage;
use std::sync::mpsc::Receiver;

pub(crate) const ENCODER_EVENT_BUDGET: usize = 16;

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

pub(crate) fn drain_encoder_events<E, D, O>(
    event_rx: &Receiver<HardwareEvent>,
    pending: &mut PendingEncoderTurns,
    mut dispatch: D,
    mut observe_event: O,
) -> Result<usize, E>
where
    D: FnMut(HostMessage) -> Result<(), E>,
    O: FnMut(HardwareEvent),
{
    let mut event_count = 0;
    for _ in 0..ENCODER_EVENT_BUDGET {
        let Ok(event) = event_rx.try_recv() else {
            break;
        };
        event_count += 1;
        observe_event(event);
        match event {
            HardwareEvent::EncoderTurn { id, delta } => pending.enqueue(id, delta),
            HardwareEvent::EncoderPress { id } => {
                flush_pending_encoder_turns(pending, &mut dispatch)?;
                dispatch(crate::input::encoder_press_message(id))?;
            }
            HardwareEvent::EncoderRelease { .. } => {}
        }
    }
    flush_pending_encoder_turns(pending, &mut dispatch)?;
    Ok(event_count)
}

pub(crate) fn flush_pending_encoder_turns<E, D>(
    pending: &mut PendingEncoderTurns,
    mut dispatch: D,
) -> Result<usize, E>
where
    D: FnMut(HostMessage) -> Result<(), E>,
{
    let mut message_count = 0;
    for message in pending.take_messages() {
        dispatch(message)?;
        message_count += 1;
    }
    Ok(message_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

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

    #[test]
    fn drops_zero_delta_turns() {
        let mut pending = PendingEncoderTurns::default();
        pending.enqueue("encoder_aux_1", 0);

        assert!(pending.take_messages().is_empty());
    }

    #[test]
    fn clamps_same_direction_burst_to_message_delta_range() {
        let mut pending = PendingEncoderTurns::default();
        for _ in 0..200 {
            pending.enqueue("encoder_main", 1);
        }

        let messages = pending.take_messages();

        assert_eq!(messages.len(), 1);
        assert_turn(&messages[0], "main", 127);

        let mut negative = PendingEncoderTurns::default();
        for _ in 0..200 {
            negative.enqueue("encoder_main", -1);
        }
        let messages = negative.take_messages();

        assert_eq!(messages.len(), 1);
        assert_turn(&messages[0], "main", -127);
    }

    #[test]
    fn mixed_encoders_keep_order_without_cross_encoder_coalescing() {
        let mut pending = PendingEncoderTurns::default();
        pending.enqueue("encoder_main", 1);
        pending.enqueue("encoder_main", 1);
        pending.enqueue("encoder_aux_1", -1);
        pending.enqueue("encoder_main", 1);

        let messages = pending.take_messages();

        assert_eq!(messages.len(), 3);
        assert_turn(&messages[0], "main", 2);
        assert_turn(&messages[1], "aux1", -1);
        assert_turn(&messages[2], "main", 1);
    }

    #[test]
    fn bounded_drain_preserves_remaining_events_for_fairness() {
        let (event_tx, event_rx) = mpsc::channel();
        for _ in 0..=ENCODER_EVENT_BUDGET {
            event_tx
                .send(HardwareEvent::EncoderTurn {
                    id: "encoder_main",
                    delta: 1,
                })
                .unwrap();
        }
        let mut pending = PendingEncoderTurns::default();
        let mut messages = Vec::new();

        let drained = drain_encoder_events(
            &event_rx,
            &mut pending,
            |message| {
                messages.push(message);
                Ok::<(), ()>(())
            },
            |_| {},
        )
        .unwrap();

        assert_eq!(drained, ENCODER_EVENT_BUDGET);
        assert_eq!(messages.len(), 1);
        assert_turn(&messages[0], "main", ENCODER_EVENT_BUDGET as i8);
        assert!(event_rx.try_recv().is_ok());
    }

    #[test]
    fn pending_turns_flush_before_encoder_press() {
        let (event_tx, event_rx) = mpsc::channel();
        event_tx
            .send(HardwareEvent::EncoderTurn {
                id: "encoder_aux_2",
                delta: 2,
            })
            .unwrap();
        event_tx
            .send(HardwareEvent::EncoderPress {
                id: "encoder_aux_2",
            })
            .unwrap();
        let mut pending = PendingEncoderTurns::default();
        let mut messages = Vec::new();

        drain_encoder_events(
            &event_rx,
            &mut pending,
            |message| {
                messages.push(message);
                Ok::<(), ()>(())
            },
            |_| {},
        )
        .unwrap();

        assert_eq!(messages.len(), 2);
        assert_turn(&messages[0], "aux2", 2);
        let HostMessage::DeviceInput { input, .. } = &messages[1] else {
            panic!("expected encoder press input");
        };
        assert_eq!(input["type"], "encoder_press");
        assert_eq!(input["id"], "aux2");
    }

    fn assert_turn(message: &HostMessage, id: &str, delta: i8) {
        let HostMessage::DeviceInput { input, .. } = message else {
            panic!("expected device input");
        };
        assert_eq!(input["id"], id);
        assert_eq!(input["delta"], delta);
    }
}
