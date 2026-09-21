use super::{source_worker_reaper, EngineSource, SourceShutdownEnvelope};
use crossbeam_channel::TrySendError;

impl Drop for EngineSource {
    fn drop(&mut self) {
        for producer in self.mirror_producers.iter().flatten() {
            producer.invalidate();
        }
        self.handoff_shutdown();
    }
}

impl EngineSource {
    pub(super) fn handoff_shutdown(&mut self) {
        if self.shutdown_tx.is_none() {
            return;
        }
        let mut source_state = self.source_owned_drop_state();
        let backlog = source_state
            .retired_backlog
            .take()
            .unwrap_or_else(crate::retired_audio_backlog::RetiredAudioBacklog::new);
        let Some(shutdown_tx) = self.shutdown_tx.take() else {
            source_worker_reaper::abort_failed_shutdown_handoff(TrySendError::Disconnected(
                SourceShutdownEnvelope {
                    backlog,
                    retirement: None,
                    source_state: Some(source_state),
                },
            ));
        };
        let envelope = SourceShutdownEnvelope {
            backlog,
            retirement: None,
            source_state: Some(source_state),
        };
        match shutdown_tx.try_send(envelope) {
            Ok(()) => {}
            Err(error) => source_worker_reaper::abort_failed_shutdown_handoff(error),
        }
    }

    #[cfg(test)]
    pub(super) fn retire_workers(
        &mut self,
    ) -> Option<realtime_engine::synth::SourceWorkerRetirement> {
        self.worker_state.retire()
    }

    fn source_owned_drop_state(&mut self) -> crate::SourceOwnedDropState {
        crate::SourceOwnedDropState {
            engine: self.engine.take(),
            control_rx: self.control_rx.take(),
            buf: self.buf.take(),
            left_buf: self.left_buf.take(),
            right_buf: self.right_buf.take(),
            load_tx: self.load_tx.take(),
            retired_tx: Some(self.retired_tx.take()),
            retired_backlog: self.retired_backlog.take(),
            emergency_retirement: self.emergency_retirement.take(),
            mirror_producers: std::mem::take(&mut self.mirror_producers),
            #[cfg(any(test, feature = "routing-tree-executor"))]
            worker_runtime: self.worker_state.take_runtime(),
            #[cfg(test)]
            drop_probe: self.source_owned_drop_probe.take(),
        }
    }
}
