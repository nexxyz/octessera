#![recursion_limit = "256"]

mod delay_timing;
mod native_help;
mod native_menu;
mod native_runner;
mod preset_name_policy;
mod protocol;
mod runtime;
mod timing_probe;
mod timing_units;

pub use native_runner::{NativeRunner, NativeRunnerConfig};
pub use platform_core::MusicalEvent;
pub use preset_name_policy::{clean_preset_name, fresh_preset_name, is_valid_preset_name};
pub use protocol::{
    HostMessage, MidiPort, RunnerMessage, RuntimeAdapterError, RuntimeAudioCommand,
    RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeErrorMetadata,
    RuntimeMomentaryFxTarget, RuntimeOperation, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeRecovery, RuntimeStatus, RuntimeStatusState, RuntimeStoreResult, RuntimeTransportState,
    RuntimeUiPulse, SampleEntry, SyncSource,
};
pub use runtime::{
    CoreRunner, HostAdapter, PlaybackRuntime, RuntimeConfig, RuntimeDispatchInput, RuntimeIngest,
};
pub use timing_probe::{
    parse_timing_probe_durations, parse_timing_probe_scenarios, print_timing_probe_summary,
    run_timing_probe, TimingProbeOptions, TimingProbeReport, TimingProbeScenario,
};

#[cfg(test)]
mod tests;
