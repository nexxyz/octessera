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
}

impl EngineSourceWorkerShutdownOwner {
    pub(super) fn new(
        completion_rx: crossbeam_channel::Receiver<realtime_engine::synth::SourceWorkerShutdown>,
        reaper: std::thread::JoinHandle<()>,
    ) -> Self {
        Self {
            completion_rx,
            reaper: Some(reaper),
        }
    }

    pub fn shutdown(mut self) -> realtime_engine::synth::SourceWorkerShutdown {
        let completion = self
            .completion_rx
            .recv()
            .expect("persistent source reaper completion");
        if let Some(reaper) = self.reaper.take() {
            let _ = reaper.join();
        }
        completion
    }
}
