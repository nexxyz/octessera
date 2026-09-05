pub const SOURCE_WORKER_MODE_INLINE: u8 = 0;
pub const SOURCE_WORKER_MODE_PERSISTENT: u8 = 2;
#[cfg(feature = "routing-tree-benchmark")]
pub const SOURCE_WORKER_MODE_ROUTING_TREE_PERSISTENT: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceWorkerRenderDisposition {
    Fresh,
    NewlyMissed,
    Recovering,
    RecoveredReady,
    Fatal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerPhase {
    Sources,
    Buses,
    #[cfg(feature = "routing-tree-benchmark")]
    RoutingTree,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkStamp {
    pub runtime_generation: u64,
    pub render_plan_generation: u64,
    pub quantum_sequence: u64,
    pub frames: usize,
    pub base_sample_clock: u64,
}

pub(super) use super::source_worker_owner::WorkerCommand;

pub type SourceWorkerStartHook = fn(usize) -> Result<(), ()>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SourceWorkerMode {
    Inline = SOURCE_WORKER_MODE_INLINE,
    Persistent = SOURCE_WORKER_MODE_PERSISTENT,
    #[cfg(feature = "routing-tree-benchmark")]
    RoutingTreePersistent = SOURCE_WORKER_MODE_ROUTING_TREE_PERSISTENT,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceWorkerShutdown {
    pub joined_workers: usize,
    pub retirement_error: Option<SourceWorkerRetirementError>,
    #[cfg(any(test, feature = "test-support"))]
    pub destroyed_owner_count: usize,
    #[cfg(any(test, feature = "test-support"))]
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
    UnsupportedPersistentBusCount {
        requested: usize,
        max: usize,
    },
    #[cfg(feature = "routing-tree-benchmark")]
    RoutingTreeAdmissionUnavailable,
}
