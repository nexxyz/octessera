use super::*;

impl Clone for EngineEventSender {
    fn clone(&self) -> Self {
        Self {
            musical_tx: self.musical_tx.clone(),
            structural_tx: self.structural_tx.clone(),
            preview_tx: self.preview_tx.clone(),
            preview_drop_rx: self.preview_drop_rx.clone(),
            latest: self.latest.clone(),
            receiver_alive: self.receiver_alive.clone(),
            next_sequence: self.next_sequence.clone(),
            emergency_sequence: self.emergency_sequence.clone(),
        }
    }
}

impl EngineEventSender {
    pub fn send(&self, event: EngineEvent) -> Result<(), QueueSendError> {
        match event {
            EngineEvent::PreparedMomentaryFxStart { config } => {
                momentary_transport::send_start(config, &self.latest, |event| {
                    self.send_structural(event)
                })
            }
            EngineEvent::MomentaryFxStop { epoch } => {
                momentary_transport::send_stop(epoch, &self.latest, |event| {
                    self.send_structural(event)
                })
            }
            event => self.send_general(event),
        }
    }

    fn send_general(&self, event: EngineEvent) -> Result<(), QueueSendError> {
        if matches!(event, EngineEvent::AllNotesOff) {
            let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
            return self.send_emergency(sequence);
        }
        if is_latest_event(&event) {
            if !self.receiver_alive.load(Ordering::Acquire) {
                return Err(QueueSendError::disconnected(QueueKind::Latest));
            }
            return self.latest.publish_event(event);
        }
        if matches!(event, EngineEvent::PreviewSample { .. }) {
            return self.send_preview(event);
        }
        let queue = if is_musical_event(&event) {
            QueueKind::Musical
        } else {
            QueueKind::Structural
        };
        self.send_queued(event, queue)
    }

    fn send_queued(&self, event: EngineEvent, queue: QueueKind) -> Result<(), QueueSendError> {
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        let send_result = match queue {
            QueueKind::Musical => self.musical_tx.try_send(SequencedEvent { sequence, event }),
            QueueKind::Structural => self
                .structural_tx
                .try_send(SequencedEvent { sequence, event }),
            _ => unreachable!("non-queued event lane"),
        };
        send_result.map_err(|error| match error {
            TrySendError::Full(_) => QueueSendError::full(queue),
            TrySendError::Disconnected(_) => QueueSendError::disconnected(queue),
        })
    }

    fn send_structural(&self, event: EngineEvent) -> Result<(), QueueSendError> {
        if !self.receiver_alive.load(Ordering::Acquire) {
            return Err(QueueSendError::disconnected(QueueKind::Structural));
        }
        self.send_queued(event, QueueKind::Structural)
    }

    fn send_preview(&self, mut event: EngineEvent) -> Result<(), QueueSendError> {
        if !self.receiver_alive.load(Ordering::Acquire) {
            return Err(QueueSendError::disconnected(QueueKind::Preview));
        }
        loop {
            match self.preview_tx.try_send(event) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Full(next)) => {
                    event = next;
                    let _ = self.preview_drop_rx.try_recv();
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err(QueueSendError::disconnected(QueueKind::Preview))
                }
            }
        }
    }

    fn send_emergency(&self, sequence: u64) -> Result<(), QueueSendError> {
        if !self.receiver_alive.load(Ordering::Acquire) {
            return Err(QueueSendError::disconnected(QueueKind::Emergency));
        }
        let mut current = self.emergency_sequence.load(Ordering::Acquire);
        while current == NO_EMERGENCY_SEQUENCE || sequence > current {
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
