pub const SOURCE_WORKER_MODE_INLINE: u8 = 0;
pub const SOURCE_WORKER_MODE_PERSISTENT: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SourceWorkerMode {
    Inline = SOURCE_WORKER_MODE_INLINE,
    Persistent = SOURCE_WORKER_MODE_PERSISTENT,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceWorkerShutdown {
    pub joined_workers: usize,
    #[cfg(test)]
    pub(crate) destroyed_owner_count: usize,
    #[cfg(test)]
    pub(crate) destroyed_owner_identities:
        [Option<super::source_worker_lifecycle::SourceWorkerOwnerIdentity>; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceWorkerSetupError {
    PrewarmFailed,
    InlineSourceExecutorUnavailable,
    PartitionsUnavailable,
    WorkerChannelsUnavailable,
}
