use crate::audio_replay::{is_replay_event, ReplayCache};
use rodio_engine_source::{EngineEvent, EngineEventSender, QueueKind, QueueSendError};
use std::sync::{Arc, Mutex, MutexGuard};

/// The DAC stream's engine event queue, if the stream is open, plus the state
/// replayed into a newly opened engine. One lock keeps replay-then-attach atomic
/// with respect to concurrent sends.
#[derive(Clone)]
pub(crate) struct AudioEngineOwner {
    state: Arc<Mutex<EngineOwnerState>>,
}

struct EngineOwnerState {
    tx: Option<EngineEventSender>,
    replay: ReplayCache,
}

#[derive(Debug)]
pub(crate) enum AudioSendError {
    Queue(QueueSendError),
    Lock,
}

impl std::fmt::Display for AudioSendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queue(error) => error.fmt(formatter),
            Self::Lock => formatter.write_str("audio engine owner lock failed"),
        }
    }
}

impl AudioEngineOwner {
    pub(crate) fn new(replay: ReplayCache) -> Self {
        Self {
            state: Arc::new(Mutex::new(EngineOwnerState { tx: None, replay })),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, EngineOwnerState>, AudioSendError> {
        self.state.lock().map_err(|_| AudioSendError::Lock)
    }

    pub(crate) fn send(&self, event: EngineEvent) -> Result<(), AudioSendError> {
        let mut state = self.lock()?;
        if is_replay_event(&event) {
            state.replay.remember(&event);
        }
        let Some(tx) = state.tx.as_ref() else {
            return Ok(());
        };
        let (result, disconnected) =
            send_result(tx.send(event), || tx.send(EngineEvent::AllNotesOff));
        if disconnected {
            state.tx = None;
        }
        result
    }

    /// Replays the remembered state into a freshly opened engine, then makes it
    /// the owner.
    pub(crate) fn attach(&self, tx: EngineEventSender) -> Result<(), String> {
        let mut state = self.lock().map_err(|error| error.to_string())?;
        for event in state.replay.events() {
            tx.send(event).map_err(|error| error.to_string())?;
        }
        state.tx = Some(tx);
        Ok(())
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub(crate) fn detach(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.tx = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn attached(tx: EngineEventSender) -> Self {
        Self {
            state: Arc::new(Mutex::new(EngineOwnerState {
                tx: Some(tx),
                replay: ReplayCache::default(),
            })),
        }
    }

    #[cfg(test)]
    pub(crate) fn is_attached(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.tx.is_some())
            .unwrap_or(false)
    }

    #[cfg(test)]
    pub(crate) fn replay_events(&self) -> Vec<EngineEvent> {
        crate::audio_replay::collect_replay_events(&self.state.lock().unwrap().replay)
    }
}

/// Maps a send result to the caller's error and whether the engine is gone.
/// A full musical queue gets emergency silence; a full latest-value queue is
/// only reported, since the next update supersedes it.
fn send_result(
    result: Result<(), QueueSendError>,
    send_emergency: impl FnOnce() -> Result<(), QueueSendError>,
) -> (Result<(), AudioSendError>, bool) {
    match result {
        Ok(()) => (Ok(()), false),
        Err(
            error @ QueueSendError::Full {
                queue: QueueKind::Latest,
            },
        ) => (Err(AudioSendError::Queue(error)), false),
        Err(error @ QueueSendError::Full { .. }) => {
            let disconnected = matches!(send_emergency(), Err(QueueSendError::Disconnected { .. }));
            (Err(AudioSendError::Queue(error)), disconnected)
        }
        Err(error @ QueueSendError::Disconnected { .. }) => {
            (Err(AudioSendError::Queue(error)), true)
        }
    }
}

#[cfg(test)]
#[path = "audio_engine_owner_tests.rs"]
mod tests;
