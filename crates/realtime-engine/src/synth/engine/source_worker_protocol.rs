pub const SOURCE_WORKER_MODE_INLINE: u8 = 0;
pub const SOURCE_WORKER_MODE_PERSISTENT: u8 = 2;

pub type SourceWorkerStartHook = fn(usize) -> Result<(), ()>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SourceWorkerMode {
    Inline = SOURCE_WORKER_MODE_INLINE,
    Persistent = SOURCE_WORKER_MODE_PERSISTENT,
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
}
