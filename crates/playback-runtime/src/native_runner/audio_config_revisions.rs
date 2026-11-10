use crate::runtime::RuntimeConfig;

/// The full audio configuration's revision and what the engine and host last received of it.
#[derive(Clone, Debug, Default)]
pub(super) struct NativeAudioConfigRevisions {
    pub(super) revision: u64,
    /// The audio revision the last JSON snapshot carried the audio settings of.
    pub(super) last_snapshot: Option<u64>,
    /// The audio revision last queued to the engine as a full configuration.
    pub(super) queued: Option<u64>,
    pub(super) last_published_runtime_config: Option<RuntimeConfig>,
}

impl NativeAudioConfigRevisions {
    pub(super) fn changed_since_snapshot(&self) -> bool {
        self.last_snapshot != Some(self.revision)
    }

    pub(super) fn mark_snapshot(&mut self) {
        self.last_snapshot = Some(self.revision);
    }

    /// The revision not yet queued to the engine, now marked queued.
    pub(super) fn take_unqueued(&mut self) -> Option<u64> {
        let unqueued = self.queued != Some(self.revision);
        self.queued = Some(self.revision);
        unqueued.then_some(self.revision)
    }

    pub(super) fn commit(&mut self, revision: u64) {
        self.revision = revision;
    }
}
