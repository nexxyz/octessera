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
