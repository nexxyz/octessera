use crate::event::EngineEvent;
use crate::latest_controls::{LatestCandidate, LatestControls, LatestCursor};
use crate::momentary_transport;
use crate::queue_types::{is_latest_event, is_musical_event, QueueEventClass};
pub use crate::queue_types::{QueueKind, QueueSendError};
use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

#[path = "queue_sender.rs"]
mod sender;

pub const MUSICAL_QUEUE_CAPACITY: usize = 512;
pub const STRUCTURAL_QUEUE_CAPACITY: usize = 64;
const PREVIEW_QUEUE_CAPACITY: usize = 1;
const NO_EMERGENCY_SEQUENCE: u64 = u64::MAX;

struct SequencedEvent {
    sequence: u64,
    event: EngineEvent,
}

pub struct EngineEventSender {
    musical_tx: Sender<SequencedEvent>,
    structural_tx: Sender<SequencedEvent>,
    preview_tx: Sender<EngineEvent>,
    preview_drop_rx: Receiver<EngineEvent>,
    latest: Arc<LatestControls>,
    receiver_alive: Arc<AtomicBool>,
    next_sequence: Arc<AtomicU64>,
    emergency_sequence: Arc<AtomicU64>,
}

pub struct EngineEventReceiver {
    musical_rx: Receiver<SequencedEvent>,
    structural_rx: Receiver<SequencedEvent>,
    preview_rx: Receiver<EngineEvent>,
    latest: Arc<LatestControls>,
    latest_cursor: LatestCursor,
    receiver_alive: Arc<AtomicBool>,
    next_musical: Option<SequencedEvent>,
    next_structural: Option<SequencedEvent>,
    musical_disconnected: bool,
    structural_disconnected: bool,
    preview_disconnected: bool,
    emergency_sequence: Arc<AtomicU64>,
    pending_emergency: Option<u64>,
    panic_fence: Option<u64>,
    #[cfg(test)]
    panic_consume_hook: Option<fn(&AtomicU64)>,
}

pub fn event_queue() -> (EngineEventSender, EngineEventReceiver) {
    let (musical_tx, musical_rx) = bounded(MUSICAL_QUEUE_CAPACITY);
    let (structural_tx, structural_rx) = bounded(STRUCTURAL_QUEUE_CAPACITY);
    let (preview_tx, preview_rx) = bounded(PREVIEW_QUEUE_CAPACITY);
    let receiver_alive = Arc::new(AtomicBool::new(true));
    let latest = Arc::new(LatestControls::new());
    let next_sequence = Arc::new(AtomicU64::new(0));
    let emergency_sequence = Arc::new(AtomicU64::new(NO_EMERGENCY_SEQUENCE));
    (
        EngineEventSender {
            musical_tx,
            structural_tx,
            preview_drop_rx: preview_rx.clone(),
            preview_tx,
            latest: latest.clone(),
            receiver_alive: receiver_alive.clone(),
            next_sequence: next_sequence.clone(),
            emergency_sequence: emergency_sequence.clone(),
        },
        EngineEventReceiver {
            musical_rx,
            structural_rx,
            preview_rx,
            latest,
            latest_cursor: LatestCursor::new(),
            receiver_alive,
            next_musical: None,
            next_structural: None,
            musical_disconnected: false,
            structural_disconnected: false,
            preview_disconnected: false,
            emergency_sequence,
            pending_emergency: None,
            panic_fence: None,
            #[cfg(test)]
            panic_consume_hook: None,
        },
    )
}

impl Drop for EngineEventReceiver {
    fn drop(&mut self) {
        self.receiver_alive.store(false, Ordering::Release);
    }
}

impl EngineEventReceiver {
    pub(crate) fn take_latest_candidate(&mut self) -> Option<LatestCandidate> {
        self.latest.candidate(&mut self.latest_cursor)
    }

    pub(crate) fn mark_latest_applied(&mut self, candidate: LatestCandidate) {
        self.latest.mark_applied(&mut self.latest_cursor, candidate);
    }

    pub(crate) fn latest_epoch_cancelled(&self, epoch: u64) -> bool {
        self.latest.is_momentary_epoch_cancelled(epoch)
    }

    pub(crate) fn cancel_latest_epoch(&mut self, epoch: u64) {
        self.latest.cancel_epoch(epoch);
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(crate) fn has_pending(&mut self) -> bool {
        self.fill_heads();
        self.fill_pending_emergency();
        self.next_source().is_some()
            || self.latest.has_pending(&self.latest_cursor)
            || !self.preview_rx.is_empty()
    }

    pub(crate) fn has_pending_structural(&mut self) -> bool {
        self.fill_heads();
        self.next_structural.is_some()
    }

    pub(crate) fn try_take_emergency(&mut self) -> Option<EngineEvent> {
        self.fill_heads();
        self.fill_pending_emergency();
        if matches!(self.next_source(), Some((QueueSource::Emergency, _))) {
            let event = self.take_next().map(|(_, event)| event);
            #[cfg(test)]
            if event.is_some() {
                if let Some(hook) = self.panic_consume_hook {
                    hook(self.emergency_sequence.as_ref());
                }
            }
            return event;
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn set_panic_consume_hook(&mut self, hook: fn(&AtomicU64)) {
        self.panic_consume_hook = Some(hook);
    }

    pub(crate) fn try_take_musical(&mut self) -> Option<EngineEvent> {
        self.fill_heads();
        self.fill_pending_emergency();
        if self.pending_emergency.is_some() {
            return None;
        }
        self.discard_fenced_musical();
        self.next_musical.take().map(|event| event.event)
    }

    pub(crate) fn try_take_next_allowed(
        &mut self,
        allow_emergency: bool,
        allow_structural_owner: bool,
        allow_structural_barrier: bool,
    ) -> Result<QueueDequeueResult, crossbeam_channel::TryRecvError> {
        self.fill_heads();
        self.fill_pending_emergency();
        if self.next_source().is_none() {
            if self.queues_disconnected() {
                return Err(crossbeam_channel::TryRecvError::Disconnected);
            }
            return Err(crossbeam_channel::TryRecvError::Empty);
        }
        let Some(class) = self.next_class() else {
            return Err(crossbeam_channel::TryRecvError::Empty);
        };
        let allowed = match class {
            QueueEventClass::Emergency => allow_emergency,
            QueueEventClass::StructuralRetiring | QueueEventClass::StructuralNonRetiring => {
                allow_structural_owner
            }
            QueueEventClass::Musical => true,
            QueueEventClass::StructuralBarrier => allow_structural_barrier,
        };
        if !allowed {
            return Ok(QueueDequeueResult { class, event: None });
        }
        match self.take_next() {
            Some((class, event)) => Ok(QueueDequeueResult {
                class,
                event: Some(event),
            }),
            None => Err(crossbeam_channel::TryRecvError::Empty),
        }
    }

    pub(crate) fn try_recv_classified(
        &mut self,
    ) -> Result<(QueueEventClass, EngineEvent), crossbeam_channel::TryRecvError> {
        self.fill_heads();
        self.fill_pending_emergency();
        let Some((_, _)) = self.next_source() else {
            if self.queues_disconnected() {
                return Err(crossbeam_channel::TryRecvError::Disconnected);
            }
            return Err(crossbeam_channel::TryRecvError::Empty);
        };
        match self.take_next() {
            Some(next) => Ok(next),
            None => Err(crossbeam_channel::TryRecvError::Empty),
        }
    }

    pub fn try_recv(&mut self) -> Result<EngineEvent, crossbeam_channel::TryRecvError> {
        self.try_recv_classified().map(|(_, event)| event)
    }

    pub fn try_recv_ordered(&mut self) -> Result<EngineEvent, crossbeam_channel::TryRecvError> {
        self.try_recv()
    }

    pub fn try_recv_musical(&mut self) -> Result<EngineEvent, crossbeam_channel::TryRecvError> {
        self.fill_heads();
        self.fill_pending_emergency();
        if self.pending_emergency.is_some() {
            return self.try_recv();
        }
        self.discard_fenced_musical();
        let Some(event) = self.next_musical.take() else {
            return if self.musical_disconnected {
                Err(crossbeam_channel::TryRecvError::Disconnected)
            } else {
                Err(crossbeam_channel::TryRecvError::Empty)
            };
        };
        Ok(event.event)
    }

    pub fn try_recv_structural(&mut self) -> Result<EngineEvent, crossbeam_channel::TryRecvError> {
        self.fill_heads();
        let Some(event) = self.next_structural.take() else {
            return if self.structural_disconnected {
                Err(crossbeam_channel::TryRecvError::Disconnected)
            } else {
                Err(crossbeam_channel::TryRecvError::Empty)
            };
        };
        Ok(event.event)
    }

    pub(crate) fn try_take_preview(&mut self) -> Option<EngineEvent> {
        match self.preview_rx.try_recv() {
            Ok(event) => Some(event),
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                self.preview_disconnected = true;
                None
            }
            Err(crossbeam_channel::TryRecvError::Empty) => None,
        }
    }

    fn fill_heads(&mut self) {
        self.discard_fenced_musical();
        if self.next_musical.is_none() && !self.musical_disconnected {
            match self.musical_rx.try_recv() {
                Ok(event) => self.next_musical = Some(event),
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    self.musical_disconnected = true
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
            }
        }
        self.discard_fenced_musical();
        if self.next_structural.is_none() && !self.structural_disconnected {
            match self.structural_rx.try_recv() {
                Ok(event) => self.next_structural = Some(event),
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    self.structural_disconnected = true
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
            }
        }
    }

    fn fill_pending_emergency(&mut self) {
        if self.pending_emergency.is_none() {
            let sequence = self
                .emergency_sequence
                .swap(NO_EMERGENCY_SEQUENCE, Ordering::AcqRel);
            if sequence != NO_EMERGENCY_SEQUENCE {
                self.pending_emergency = Some(sequence);
            }
        }
    }

    fn discard_fenced_musical(&mut self) {
        let Some(fence) = self.panic_fence else {
            return;
        };
        while self
            .next_musical
            .as_ref()
            .is_some_and(|event| event.sequence <= fence)
        {
            self.next_musical = None;
            if self.musical_disconnected {
                break;
            }
            match self.musical_rx.try_recv() {
                Ok(event) => self.next_musical = Some(event),
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    self.musical_disconnected = true;
                    break;
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
            }
        }
    }

    fn next_source(&self) -> Option<(QueueSource, u64)> {
        if let Some(sequence) = self.pending_emergency {
            return Some((QueueSource::Emergency, sequence));
        }
        let musical = self
            .next_musical
            .as_ref()
            .map(|event| (QueueSource::Musical, event.sequence));
        let structural = self
            .next_structural
            .as_ref()
            .map(|event| (QueueSource::Structural, event.sequence));
        [musical, structural]
            .into_iter()
            .flatten()
            .min_by_key(|(_, sequence)| *sequence)
    }

    fn take_next(&mut self) -> Option<(QueueEventClass, EngineEvent)> {
        let (source, sequence) = self.next_source()?;
        match source {
            QueueSource::Emergency => {
                self.pending_emergency = None;
                self.panic_fence = Some(
                    self.panic_fence
                        .map_or(sequence, |fence| fence.max(sequence)),
                );
                Some((QueueEventClass::Emergency, EngineEvent::AllNotesOff))
            }
            QueueSource::Musical => {
                let event = self.next_musical.take()?;
                debug_assert_eq!(event.sequence, sequence);
                let class = QueueEventClass::from_event(&event.event);
                Some((class, event.event))
            }
            QueueSource::Structural => {
                let event = self.next_structural.take()?;
                debug_assert_eq!(event.sequence, sequence);
                let class = QueueEventClass::from_event(&event.event);
                Some((class, event.event))
            }
        }
    }

    fn next_class(&self) -> Option<QueueEventClass> {
        match self.next_source()?.0 {
            QueueSource::Emergency => Some(QueueEventClass::Emergency),
            QueueSource::Musical => self
                .next_musical
                .as_ref()
                .map(|event| QueueEventClass::from_event(&event.event)),
            QueueSource::Structural => self
                .next_structural
                .as_ref()
                .map(|event| QueueEventClass::from_event(&event.event)),
        }
    }

    fn queues_disconnected(&self) -> bool {
        self.next_musical.is_none()
            && self.next_structural.is_none()
            && self.musical_disconnected
            && self.structural_disconnected
    }
}

#[derive(Clone, Copy)]
enum QueueSource {
    Emergency,
    Musical,
    Structural,
}

pub(crate) struct QueueDequeueResult {
    pub(crate) class: QueueEventClass,
    pub(crate) event: Option<EngineEvent>,
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
