pub const ROUTING_TREE_WORKER_THREAD_NAMES: [&str; 2] = ["oct-dsp-tree-0", "oct-dsp-tree-1"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceWorkerRenderDisposition {
    Fresh,
    NewlyMissed,
    Recovering,
    RecoveredReady,
    Fatal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkStamp {
    pub runtime_generation: u64,
    pub render_plan_generation: u64,
    pub quantum_sequence: u64,
    pub frames: usize,
    pub base_sample_clock: u64,
}

#[cfg(feature = "routing-tree-executor")]
pub(super) use super::source_worker_owner::RoutingTreeWork;

pub type SourceWorkerStartHook = fn(usize) -> Result<(), ()>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceWorkerMode {
    Inline,
    #[cfg(feature = "routing-tree-executor")]
    RoutingTree,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceWorkerShutdown {
    pub joined_workers: usize,
    pub retirement_error: Option<SourceWorkerRetirementError>,
    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    pub destroyed_owner_count: usize,
    #[cfg(all(any(test, feature = "test-support"), feature = "routing-tree-executor"))]
    pub destroyed_owner_identities:
        [Option<super::source_worker_lifecycle::SourceWorkerOwnerIdentity>; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceWorkerRetirementError {
    CloseStateUnavailable,
    CloseStateMismatch,
    GenerationMismatch { expected: u64, actual: u64 },
    RuntimeStillOpen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(feature = "routing-tree-executor")]
pub enum SourceWorkerSetupError {
    WorkerSchedulingUnavailable {
        parity: usize,
    },
    InvalidBlockFrames {
        requested: usize,
        min: usize,
        max: usize,
    },
    PrewarmFailed,
    InlineSourceExecutorUnavailable,
    PartitionsUnavailable,
    WorkerChannelsUnavailable,
    WorkerThreadUnavailable,
    RetirementReaperUnavailable,
    #[cfg(feature = "routing-tree-executor")]
    RoutingTreeAdmissionUnavailable,
}
