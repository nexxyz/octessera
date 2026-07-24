use crate::EngineEvent;
use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

pub const ORDERED_QUEUE_CAPACITY: usize = 512;
pub const COALESCED_QUEUE_CAPACITY: usize = 128;
const NO_EMERGENCY_SEQUENCE: u64 = u64::MAX;

struct SequencedEvent {
    sequence: u64,
    event: EngineEvent,
}

pub struct EngineEventSender {
    ordered_tx: Sender<SequencedEvent>,
    coalesced_tx: Sender<SequencedEvent>,
    coalesced_drop_rx: Receiver<SequencedEvent>,
    receiver_alive: Arc<AtomicBool>,
    next_sequence: Arc<AtomicU64>,
    emergency_sequence: Arc<AtomicU64>,
}

pub struct EngineEventReceiver {
    ordered_rx: Receiver<SequencedEvent>,
    coalesced_rx: Receiver<SequencedEvent>,
    receiver_alive: Arc<AtomicBool>,
    next_ordered: Option<SequencedEvent>,
    next_coalesced: Option<SequencedEvent>,
    ordered_disconnected: bool,
    coalesced_disconnected: bool,
    emergency_sequence: Arc<AtomicU64>,
    pending_emergency: Option<u64>,
}

pub fn event_queue() -> (EngineEventSender, EngineEventReceiver) {
    let (ordered_tx, ordered_rx) = bounded(ORDERED_QUEUE_CAPACITY);
    let (coalesced_tx, coalesced_rx) = bounded(COALESCED_QUEUE_CAPACITY);
    let receiver_alive = Arc::new(AtomicBool::new(true));
    let next_sequence = Arc::new(AtomicU64::new(0));
    let emergency_sequence = Arc::new(AtomicU64::new(NO_EMERGENCY_SEQUENCE));
    (
        EngineEventSender {
            ordered_tx,
            coalesced_tx,
            coalesced_drop_rx: coalesced_rx.clone(),
            receiver_alive: receiver_alive.clone(),
            next_sequence: next_sequence.clone(),
            emergency_sequence: emergency_sequence.clone(),
        },
        EngineEventReceiver {
            ordered_rx,
            coalesced_rx,
            receiver_alive,
            next_ordered: None,
            next_coalesced: None,
            ordered_disconnected: false,
            coalesced_disconnected: false,
            emergency_sequence,
            pending_emergency: None,
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
            next_sequence: self.next_sequence.clone(),
            emergency_sequence: self.emergency_sequence.clone(),
        }
    }
}

impl EngineEventSender {
    pub fn send(&self, event: EngineEvent) -> Result<(), QueueSendError> {
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        if matches!(event, EngineEvent::AllNotesOff) {
            return self.send_emergency(sequence);
        }
        let event = SequencedEvent { sequence, event };
        if is_ordered_event(&event.event) {
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

    fn send_emergency(&self, sequence: u64) -> Result<(), QueueSendError> {
        if !self.receiver_alive.load(Ordering::Acquire) {
            return Err(QueueSendError::disconnected(QueueKind::Emergency));
        }
        let mut current = self.emergency_sequence.load(Ordering::Acquire);
        while sequence < current {
            match self.emergency_sequence.compare_exchange_weak(
                current,
                sequence,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(next) => current = next,
            }
        }
        Ok(())
    }
}

impl Drop for EngineEventReceiver {
    fn drop(&mut self) {
        self.receiver_alive.store(false, Ordering::Release);
    }
}

impl EngineEventReceiver {
    pub fn try_recv(&mut self) -> Result<EngineEvent, crossbeam_channel::TryRecvError> {
        self.fill_heads();
        if self.pending_emergency.is_none() {
            let sequence = self
                .emergency_sequence
                .swap(NO_EMERGENCY_SEQUENCE, Ordering::AcqRel);
            if sequence != NO_EMERGENCY_SEQUENCE {
                self.pending_emergency = Some(sequence);
            }
        }

        let Some((source, sequence)) = self.next_source() else {
            if self.ordered_is_disconnected() && self.coalesced_is_disconnected() {
                return Err(crossbeam_channel::TryRecvError::Disconnected);
            }
            return Err(crossbeam_channel::TryRecvError::Empty);
        };
        match source {
            QueueSource::Emergency => {
                self.pending_emergency = None;
                Ok(EngineEvent::AllNotesOff)
            }
            QueueSource::Ordered => {
                let event = self.next_ordered.take().expect("ordered queue head");
                debug_assert_eq!(event.sequence, sequence);
                Ok(event.event)
            }
            QueueSource::Coalesced => {
                let event = self.next_coalesced.take().expect("coalesced queue head");
                debug_assert_eq!(event.sequence, sequence);
                Ok(event.event)
            }
        }
    }

    pub fn try_recv_ordered(&mut self) -> Result<EngineEvent, crossbeam_channel::TryRecvError> {
        self.try_recv()
    }

    pub fn try_recv_coalesced(&mut self) -> Result<EngineEvent, crossbeam_channel::TryRecvError> {
        self.try_recv()
    }

    fn fill_heads(&mut self) {
        if self.next_ordered.is_none() && !self.ordered_disconnected {
            match self.ordered_rx.try_recv() {
                Ok(event) => self.next_ordered = Some(event),
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    self.ordered_disconnected = true
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
            }
        }
        if self.next_coalesced.is_none() && !self.coalesced_disconnected {
            match self.coalesced_rx.try_recv() {
                Ok(event) => self.next_coalesced = Some(event),
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    self.coalesced_disconnected = true
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
            }
        }
    }

    fn next_source(&self) -> Option<(QueueSource, u64)> {
        let emergency = self
            .pending_emergency
            .map(|sequence| (QueueSource::Emergency, sequence));
        let ordered = self
            .next_ordered
            .as_ref()
            .map(|event| (QueueSource::Ordered, event.sequence));
        let coalesced = self
            .next_coalesced
            .as_ref()
            .map(|event| (QueueSource::Coalesced, event.sequence));
        [emergency, ordered, coalesced]
            .into_iter()
            .flatten()
            .min_by_key(|(_, sequence)| *sequence)
    }

    fn ordered_is_disconnected(&self) -> bool {
        self.next_ordered.is_none() && self.ordered_disconnected
    }

    fn coalesced_is_disconnected(&self) -> bool {
        self.next_coalesced.is_none() && self.coalesced_disconnected
    }
}

#[derive(Clone, Copy)]
enum QueueSource {
    Emergency,
    Ordered,
    Coalesced,
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
    Emergency,
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
            Self::Full {
                queue: QueueKind::Emergency,
            } => f.write_str("emergency audio event queue is full"),
            Self::Disconnected { queue } => write!(f, "{queue:?} audio event queue disconnected"),
        }
    }
}

impl std::error::Error for QueueSendError {}

pub(crate) fn is_ordered_event(event: &EngineEvent) -> bool {
    matches!(
        event,
        EngineEvent::SetPreparedInstruments(_)
            | EngineEvent::SetPreparedAudioConfig(_)
            | EngineEvent::SetPreparedSampleBank { .. }
            | EngineEvent::SetPreparedInstrumentSlot { .. }
            | EngineEvent::SetPreparedFxBusSlot { .. }
            | EngineEvent::SetPreparedGlobalFxSlot { .. }
            | EngineEvent::NoteOn { .. }
            | EngineEvent::NoteOff { .. }
            | EngineEvent::Cc { .. }
            | EngineEvent::PreviewSample { .. }
            | EngineEvent::PreparedMomentaryFxStart { .. }
            | EngineEvent::MomentaryFxUpdate { .. }
            | EngineEvent::MomentaryFxStop { .. }
            | EngineEvent::ProbeMark { .. }
    )
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
