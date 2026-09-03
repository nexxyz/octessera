use realtime_engine::synth::{SourceWorkerHealth, SourceWorkerRetirement, SourceWorkerRuntime};

#[derive(Clone, Copy)]
pub(super) enum EngineSourceMode {
    Inline,
    Persistent,
}

pub(super) struct PersistentSourceWorker {
    pub(super) runtime: SourceWorkerRuntime,
}

pub(super) struct EngineSourceWorkerState {
    pub(super) mode: EngineSourceMode,
    pub(super) worker: Option<PersistentSourceWorker>,
}

impl EngineSourceWorkerState {
    pub(super) fn inline() -> Self {
        Self {
            mode: EngineSourceMode::Inline,
            worker: None,
        }
    }

    pub(super) fn persistent(runtime: SourceWorkerRuntime) -> Self {
        Self {
            mode: EngineSourceMode::Persistent,
            worker: Some(PersistentSourceWorker::new(runtime)),
        }
    }

    pub(super) fn health(&self) -> SourceWorkerHealth {
        match (&self.mode, &self.worker) {
            (EngineSourceMode::Inline, _) => SourceWorkerHealth::Disabled,
            (EngineSourceMode::Persistent, Some(worker)) => worker.runtime.health_snapshot().status,
            (EngineSourceMode::Persistent, None) => SourceWorkerHealth::CompletionFailed,
        }
    }

    pub(super) fn retire(&mut self) -> Option<SourceWorkerRetirement> {
        self.mode = EngineSourceMode::Inline;
        self.worker.take().map(PersistentSourceWorker::retire)
    }
}

impl PersistentSourceWorker {
    pub(super) fn new(runtime: SourceWorkerRuntime) -> Self {
        Self { runtime }
    }

    pub(super) fn retire(self) -> SourceWorkerRetirement {
        self.runtime.retire()
    }
}

pub struct EngineSourceWorkerShutdownOwner {
    completion_rx: crossbeam_channel::Receiver<realtime_engine::synth::SourceWorkerShutdown>,
    reaper: Option<std::thread::JoinHandle<()>>,
    #[cfg(test)]
    lifecycle_probe: crate::source_worker_reaper::ReaperLifecycleProbe,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineSourceWorkerShutdownError {
    ReaperCompletionUnavailable,
    ReaperThreadPanicked,
}

impl EngineSourceWorkerShutdownOwner {
    pub(super) fn new(
        completion_rx: crossbeam_channel::Receiver<realtime_engine::synth::SourceWorkerShutdown>,
        reaper: std::thread::JoinHandle<()>,
        #[cfg(test)] lifecycle_probe: crate::source_worker_reaper::ReaperLifecycleProbe,
    ) -> Self {
        Self {
            completion_rx,
            reaper: Some(reaper),
            #[cfg(test)]
            lifecycle_probe,
        }
    }

    #[cfg(test)]
    pub(crate) fn lifecycle_probe_for_test(
        &self,
    ) -> crate::source_worker_reaper::ReaperLifecycleProbe {
        self.lifecycle_probe.clone()
    }

    pub fn shutdown(self) -> realtime_engine::synth::SourceWorkerShutdown {
        self.try_shutdown()
            .expect("persistent source reaper completion")
    }

    pub fn try_shutdown(
        mut self,
    ) -> Result<realtime_engine::synth::SourceWorkerShutdown, EngineSourceWorkerShutdownError> {
        let completion = match self.completion_rx.recv() {
            Ok(completion) => completion,
            Err(_) => {
                let reaper_panicked = self
                    .reaper
                    .take()
                    .is_some_and(|reaper| reaper.join().is_err());
                return Err(if reaper_panicked {
                    EngineSourceWorkerShutdownError::ReaperThreadPanicked
                } else {
                    EngineSourceWorkerShutdownError::ReaperCompletionUnavailable
                });
            }
        };
        if let Some(reaper) = self.reaper.take() {
            if reaper.join().is_err() {
                return Err(EngineSourceWorkerShutdownError::ReaperThreadPanicked);
            }
        }
        Ok(completion)
    }
}
