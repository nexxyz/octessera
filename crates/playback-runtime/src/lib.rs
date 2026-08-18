#![recursion_limit = "256"]

mod deferred_default_save;
mod delay_timing;
mod native_help;
mod native_menu;
mod native_runner;
pub mod oled_frame;
mod preset_name_policy;
mod protocol;
mod runtime;
mod timing_probe;
mod timing_units;
pub mod user_data_bundle;

pub use deferred_default_save::{DeferredDefaultSave, DeferredDefaultSaveEntry};
pub use native_runner::{AudioOutputSet, NativeRunner, NativeRunnerConfig};
pub use platform_core::MusicalEvent;
pub use preset_name_policy::{clean_preset_name, fresh_preset_name, is_valid_preset_name};
pub use protocol::{
    HostMessage, MidiPort, RunnerMessage, RuntimeAdapterError, RuntimeAudioCommand,
    RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeErrorMetadata,
    RuntimeMomentaryFxTarget, RuntimeOperation, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeRecovery, RuntimeSetupPortalDisposition, RuntimeSetupPortalErrorCode,
    RuntimeSetupPortalPhase, RuntimeSetupPortalStatus, RuntimeSetupPortalTransfer, RuntimeStatus,
    RuntimeStatusState, RuntimeStoreResult, RuntimeSystemInfo, RuntimeSystemInfoError,
    RuntimeTransportState, SampleEntry, SyncSource, SETUP_PORTAL_SUFFIX_MAX_CHARS,
};
pub use runtime::{
    CoreRunner, HostAdapter, PlaybackRuntime, RuntimeConfig, RuntimeDispatchInput, RuntimeIngest,
    RuntimeOledCacheFault, RuntimePresentationMetrics,
};
pub use timing_probe::{
    parse_timing_probe_durations, parse_timing_probe_scenarios, print_timing_probe_summary,
    run_timing_probe, TimingProbeOptions, TimingProbeReport, TimingProbeScenario,
};
pub use user_data_bundle::{
    apply_user_preference_delta, decode_user_data_bundle, encode_user_data_bundle,
    is_safe_user_data_name, manifest_for_user_data_bundle, media_reference_from_bytes,
    migrate_user_data_bundle, new_user_data_bundle, preference_delta_from_config,
    validate_user_data_bundle, UserDataBundle, UserDataBundleMetadata, UserDataManifestEntry,
    UserDataManifestEntryKind, UserDataMediaKind, UserDataMediaReference, UserDataMusicalState,
    UserDataPreset, UserPreferenceDelta, USER_DATA_BUNDLE_KIND, USER_DATA_BUNDLE_SCHEMA_VERSION,
    USER_DATA_MAX_BUNDLE_BYTES, USER_DATA_MAX_ITEM_BYTES, USER_DATA_MAX_MANIFEST_ENTRIES,
    USER_DATA_MAX_MEDIA_BYTES, USER_DATA_MAX_MEDIA_REFERENCES, USER_DATA_MAX_METADATA_CHARS,
    USER_DATA_MAX_PRESETS, USER_DATA_MAX_PRESET_NAME_CHARS, USER_DATA_MAX_TOTAL_MEDIA_BYTES,
};

#[cfg(test)]
mod tests;
