use realtime_engine::synth::SourceWorkerHealth;
#[cfg(feature = "routing-tree-executor")]
use realtime_engine::synth::SourceWorkerRetirement;
#[cfg(feature = "routing-tree-executor")]
use realtime_engine::synth::SourceWorkerRuntime;

#[derive(Clone, Copy)]
pub(super) enum EngineSourceMode {
    Inline,
    #[cfg(feature = "routing-tree-executor")]
    RoutingTree,
}

#[cfg(feature = "routing-tree-executor")]
pub(super) struct RoutingTreeSourceWorker {
    pub(super) runtime: SourceWorkerRuntime,
}

pub(super) struct EngineSourceWorkerState {
    pub(super) mode: EngineSourceMode,
    #[cfg(feature = "routing-tree-executor")]
    pub(super) worker: Option<RoutingTreeSourceWorker>,
}

impl EngineSourceWorkerState {
    pub(super) fn inline() -> Self {
        Self {
            mode: EngineSourceMode::Inline,
            #[cfg(feature = "routing-tree-executor")]
            worker: None,
        }
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn routing_tree(runtime: SourceWorkerRuntime) -> Self {
        Self {
            mode: EngineSourceMode::RoutingTree,
            worker: Some(RoutingTreeSourceWorker::new(runtime)),
        }
    }

    pub(super) fn health(&self) -> SourceWorkerHealth {
        match self.mode {
            EngineSourceMode::Inline => SourceWorkerHealth::Disabled,
            #[cfg(feature = "routing-tree-executor")]
            EngineSourceMode::RoutingTree => self
                .worker
                .as_ref()
                .map_or(SourceWorkerHealth::CompletionFailed, |worker| {
                    worker.runtime.health_snapshot().status
                }),
        }
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn is_routing_tree(&self) -> bool {
        match self.mode {
            EngineSourceMode::RoutingTree => true,
            EngineSourceMode::Inline => false,
        }
    }

    pub(super) fn lookahead_frames(&self) -> usize {
        #[cfg(not(feature = "routing-tree-executor"))]
        return 0;
        #[cfg(feature = "routing-tree-executor")]
        self.worker
            .as_ref()
            .map(|worker| worker.runtime.lookahead_frames())
            .unwrap_or(0)
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn retire(&mut self) -> Option<SourceWorkerRetirement> {
        self.mode = EngineSourceMode::Inline;
        self.worker.take().map(RoutingTreeSourceWorker::retire)
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn take_runtime(&mut self) -> Option<SourceWorkerRuntime> {
        self.mode = EngineSourceMode::Inline;
        self.worker.take().map(|worker| worker.runtime)
    }
}

#[cfg(feature = "routing-tree-executor")]
impl RoutingTreeSourceWorker {
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
    #[cfg(all(test, feature = "routing-tree-executor"))]
    lifecycle_probe: crate::source_worker_reaper::ReaperLifecycleProbe,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineSourceWorkerShutdownError {
    ReaperCompletionUnavailable,
    ReaperThreadPanicked,
}

impl EngineSourceWorkerShutdownOwner {
    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn new(
        completion_rx: crossbeam_channel::Receiver<realtime_engine::synth::SourceWorkerShutdown>,
        reaper: std::thread::JoinHandle<()>,
        #[cfg(all(test, feature = "routing-tree-executor"))]
        lifecycle_probe: crate::source_worker_reaper::ReaperLifecycleProbe,
    ) -> Self {
        Self {
            completion_rx,
            reaper: Some(reaper),
            #[cfg(all(test, feature = "routing-tree-executor"))]
            lifecycle_probe,
        }
    }

    #[cfg(all(test, feature = "routing-tree-executor"))]
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
