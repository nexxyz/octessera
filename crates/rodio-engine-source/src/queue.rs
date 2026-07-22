use crate::EngineEvent;
use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const ORDERED_QUEUE_CAPACITY: usize = 512;
pub const COALESCED_QUEUE_CAPACITY: usize = 128;

pub struct EngineEventSender {
    ordered_tx: Sender<EngineEvent>,
    coalesced_tx: Sender<EngineEvent>,
    coalesced_drop_rx: Receiver<EngineEvent>,
    receiver_alive: Arc<AtomicBool>,
}

pub struct EngineEventReceiver {
    ordered_rx: Receiver<EngineEvent>,
    coalesced_rx: Receiver<EngineEvent>,
    receiver_alive: Arc<AtomicBool>,
}

pub fn event_queue() -> (EngineEventSender, EngineEventReceiver) {
    let (ordered_tx, ordered_rx) = bounded(ORDERED_QUEUE_CAPACITY);
    let (coalesced_tx, coalesced_rx) = bounded(COALESCED_QUEUE_CAPACITY);
    let receiver_alive = Arc::new(AtomicBool::new(true));
    (
        EngineEventSender {
            ordered_tx,
            coalesced_tx,
            coalesced_drop_rx: coalesced_rx.clone(),
            receiver_alive: receiver_alive.clone(),
        },
        EngineEventReceiver {
            ordered_rx,
            coalesced_rx,
            receiver_alive,
        },
    )
}

impl Clone for EngineEventSender {
    fn clone(&self) -> Self {
        Self {
            ordered_tx: self.ordered_tx.clone(),
            coalesced_tx: self.coalesced_tx.clone(),
            coalesced_drop_rx: self.coalesced_drop_rx.clone(),
            receiver_alive: self.receiver_alive.clone(),
        }
    }
}

impl EngineEventSender {
    pub fn send(&self, event: EngineEvent) -> Result<(), QueueSendError> {
        if is_ordered_event(&event) {
            return self
                .ordered_tx
                .try_send(event)
                .map_err(|error| match error {
                    TrySendError::Full(_) => QueueSendError::full(QueueKind::Ordered),
                    TrySendError::Disconnected(_) => {
                        QueueSendError::disconnected(QueueKind::Ordered)
                    }
                });
        }
        if !self.receiver_alive.load(Ordering::Acquire) {
            return Err(QueueSendError::disconnected(QueueKind::Coalesced));
        }
        let mut event = event;
        loop {
            match self.coalesced_tx.try_send(event) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Full(next)) => {
                    event = next;
                    let _ = self.coalesced_drop_rx.try_recv();
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err(QueueSendError::disconnected(QueueKind::Coalesced))
                }
            }
        }
    }
}

impl Drop for EngineEventReceiver {
    fn drop(&mut self) {
        self.receiver_alive.store(false, Ordering::Release);
    }
}

impl EngineEventReceiver {
    pub fn try_recv_ordered(&self) -> Result<EngineEvent, crossbeam_channel::TryRecvError> {
        self.ordered_rx.try_recv()
    }

    pub fn try_recv_coalesced(&self) -> Result<EngineEvent, crossbeam_channel::TryRecvError> {
        self.coalesced_rx.try_recv()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueSendError {
    Full { queue: QueueKind },
    Disconnected { queue: QueueKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueKind {
    Ordered,
    Coalesced,
}

impl QueueSendError {
    fn full(queue: QueueKind) -> Self {
        Self::Full { queue }
    }

    fn disconnected(queue: QueueKind) -> Self {
        Self::Disconnected { queue }
    }

    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full { .. })
    }

    pub fn is_ordered_full(&self) -> bool {
        matches!(
            self,
            Self::Full {
                queue: QueueKind::Ordered
            }
        )
    }
}

impl Display for QueueSendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full {
                queue: QueueKind::Ordered,
            } => f.write_str("ordered audio event queue is full"),
            Self::Full {
                queue: QueueKind::Coalesced,
            } => f.write_str("coalesced audio event queue is full"),
            Self::Disconnected { queue } => write!(f, "{queue:?} audio event queue disconnected"),
        }
    }
}

impl std::error::Error for QueueSendError {}

pub(crate) fn is_ordered_event(event: &EngineEvent) -> bool {
    matches!(
        event,
        EngineEvent::AllNotesOff
            | EngineEvent::NoteOn { .. }
            | EngineEvent::NoteOff { .. }
            | EngineEvent::Cc { .. }
            | EngineEvent::PreviewSample { .. }
            | EngineEvent::MomentaryFxStart { .. }
            | EngineEvent::PreparedMomentaryFxStart { .. }
            | EngineEvent::MomentaryFxUpdate { .. }
            | EngineEvent::MomentaryFxStop { .. }
            | EngineEvent::ProbeMark { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use realtime_engine::synth::MomentaryFxTarget;
    use std::collections::BTreeMap;

    #[test]
    fn coalesced_queue_is_bounded_and_keeps_latest_control() {
        let (sender, receiver) = event_queue();
        for value in 0..(COALESCED_QUEUE_CAPACITY + 17) {
            sender
                .send(EngineEvent::SetMasterVolume {
                    volume_pct: value as f32,
                })
                .unwrap();
        }
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv_coalesced() {
            events.push(event);
        }
        assert!(events.len() <= COALESCED_QUEUE_CAPACITY);
        assert!(matches!(
            events.last(),
            Some(EngineEvent::SetMasterVolume { volume_pct }) if *volume_pct == (COALESCED_QUEUE_CAPACITY + 16) as f32
        ));
    }

    #[test]
    fn ordered_queue_preserves_note_panic_and_momentary_fx_order() {
        let (sender, receiver) = event_queue();
        sender
            .send(EngineEvent::NoteOn {
                instrument_slot: 0,
                note: 60,
                velocity: 100,
                duration_ms: 100,
            })
            .unwrap();
        sender.send(EngineEvent::AllNotesOff).unwrap();
        sender
            .send(EngineEvent::MomentaryFxStop { id: "fx".into() })
            .unwrap();
        assert!(matches!(
            receiver.try_recv_ordered().unwrap(),
            EngineEvent::NoteOn { note: 60, .. }
        ));
        assert!(matches!(
            receiver.try_recv_ordered().unwrap(),
            EngineEvent::AllNotesOff
        ));
        assert!(matches!(
            receiver.try_recv_ordered().unwrap(),
            EngineEvent::MomentaryFxStop { id } if id == "fx"
        ));
        let _ = MomentaryFxTarget::Global;
        let _ = BTreeMap::<String, serde_json::Value>::new();
    }

    #[test]
    fn ordered_queue_full_is_typed_and_does_not_silently_drop_panic() {
        let (sender, receiver) = event_queue();
        for _ in 0..ORDERED_QUEUE_CAPACITY {
            sender
                .send(EngineEvent::NoteOn {
                    instrument_slot: 0,
                    note: 60,
                    velocity: 100,
                    duration_ms: 100,
                })
                .unwrap();
        }

        let error = sender.send(EngineEvent::AllNotesOff).unwrap_err();
        assert!(error.is_full());
        assert!(error.is_ordered_full());
        assert!(matches!(
            error,
            QueueSendError::Full {
                queue: QueueKind::Ordered
            }
        ));
        assert!(matches!(
            receiver.try_recv_ordered(),
            Ok(EngineEvent::NoteOn { .. })
        ));
    }

    #[test]
    fn ordered_queue_keeps_preview_and_momentary_events_fifo() {
        let (sender, receiver) = event_queue();
        sender
            .send(EngineEvent::NoteOn {
                instrument_slot: 0,
                note: 60,
                velocity: 100,
                duration_ms: 100,
            })
            .unwrap();
        sender
            .send(EngineEvent::NoteOff {
                instrument_slot: 0,
                note: 60,
            })
            .unwrap();
        sender
            .send(EngineEvent::PreviewSample {
                instrument_slot: 0,
                buffer: realtime_engine::synth::SampleBuffer {
                    samples: vec![0.0].into_boxed_slice().into(),
                    channels: 1,
                    sample_rate: 44_100,
                },
                velocity: 100,
            })
            .unwrap();
        sender
            .send(EngineEvent::MomentaryFxStart {
                id: "fx".into(),
                fx_type: "stutter".into(),
                params: BTreeMap::new(),
                target: MomentaryFxTarget::Global,
            })
            .unwrap();

        assert!(matches!(
            receiver.try_recv_ordered().unwrap(),
            EngineEvent::NoteOn { .. }
        ));
        assert!(matches!(
            receiver.try_recv_ordered().unwrap(),
            EngineEvent::NoteOff { .. }
        ));
        assert!(matches!(
            receiver.try_recv_ordered().unwrap(),
            EngineEvent::PreviewSample { .. }
        ));
        assert!(matches!(
            receiver.try_recv_ordered().unwrap(),
            EngineEvent::MomentaryFxStart { .. }
        ));
    }

    #[test]
    fn queue_disconnect_is_typed_for_coalesced_controls() {
        let (sender, receiver) = event_queue();
        drop(receiver);
        let error = sender
            .send(EngineEvent::SetMasterVolume { volume_pct: 80.0 })
            .unwrap_err();
        assert!(matches!(
            error,
            QueueSendError::Disconnected {
                queue: QueueKind::Coalesced
            }
        ));
    }
}
