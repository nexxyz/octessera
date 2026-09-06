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
        let retirement = self.retire_workers();
        let Some(backlog) = self.retired_backlog.take() else {
            return;
        };
        let Some(shutdown_tx) = self.shutdown_tx.take() else {
            source_worker_reaper::abort_failed_shutdown_handoff(TrySendError::Disconnected(
                SourceShutdownEnvelope {
                    backlog,
                    retirement,
                },
            ));
        };
        let envelope = SourceShutdownEnvelope {
            backlog,
            retirement,
        };
        match shutdown_tx.try_send(envelope) {
            Ok(()) => {}
            Err(error) => source_worker_reaper::abort_failed_shutdown_handoff(error),
        }
    }

    pub(super) fn retire_workers(
        &mut self,
    ) -> Option<realtime_engine::synth::SourceWorkerRetirement> {
        self.worker_state.retire()
    }
}
