mod audio_config;
mod dsp_config;
mod engine;
mod fx;
mod fx_params;
mod runtime_state;
#[cfg(feature = "source-worker-benchmark-timing")]
mod source_worker_timing;
#[cfg(all(test, feature = "source-worker-benchmark-timing"))]
mod source_worker_timing_tests;
mod synth_voice_pool;
#[cfg(test)]
mod tests;
mod types;

pub use audio_config::{
    normalize_audio_config, normalize_fx_slot, normalize_instrument_slot_config,
    parse_voice_stealing_mode, validate_fx_type, validate_momentary_fx_type,
    validate_sample_bank_param_path, validate_synth_param_path, NormalizedAudioConfig,
    NormalizedInstrumentSlot, NormalizedSampleConfig,
};
pub use dsp_config::{BusIdleThreshold, DspRuntimeConfig, WorkerWarningThreshold};
#[cfg(any(test, feature = "test-support"))]
pub use engine::SourceWorkerHoldControl;
#[cfg(any(test, feature = "test-support"))]
pub use engine::{
    install_source_worker_shutdown_probe_for_test, SourceWorkerOwnerIdentity,
    SourceWorkerShutdownProbeGuard,
};
pub use engine::{
    prepare_audio_config, prepare_fx_bus_slot, prepare_global_fx_slot,
    prepare_instrument_slot_config, prepare_instruments_config, prepare_momentary_fx_start,
    PreparedAudioConfig, PreparedFxBusSlot, PreparedGlobalFxSlot, PreparedInstrumentSlot,
    PreparedInstrumentsConfig, PreparedMomentaryFxStart, RetiredAudioState, SourceWorkerHealth,
    SourceWorkerHealthSnapshot, SourceWorkerLifecycle, SourceWorkerLoadSnapshot, SourceWorkerMode,
    SourceWorkerRetirement, SourceWorkerRetirementError, SourceWorkerRuntime,
    SourceWorkerSetupError, SourceWorkerShutdown, SourceWorkerStartHook, SynthEngine,
    SOURCE_WORKER_MAX_COST_UNITS, SOURCE_WORKER_MODE_INLINE, SOURCE_WORKER_MODE_PERSISTENT,
    SOURCE_WORKER_SAMPLE_COST_UNITS, SOURCE_WORKER_SYNTH_COST_UNITS, SOURCE_WORKER_THREAD_NAMES,
};
#[cfg(feature = "source-worker-benchmark-timing")]
pub use source_worker_timing::{
    SourceWorkerCoordinatorTimingSnapshot, SourceWorkerCpuSampler, SourceWorkerTimingProbe,
    SourceWorkerTimingSnapshot, SourceWorkerWorkerTimingSnapshot,
};
pub use types::{
    default_synth_config, AudioLoadStatus, EnvConfig, FilterConfig, FilterType, FxBusConfig,
    FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MasterFxConfig, MixerConfig, MomentaryFxTarget, OscConfig, RenderProfileSnapshot,
    SampleBankConfig, SampleBuffer, SampleSlotConfig, SynthConfig, SynthProfileSnapshot,
    VoiceStealingMode, BUS_FX_WARNING_SLOT_COUNT, BUS_SLOTS_PER_BUS,
    DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES, DEFAULT_AUDIO_SAMPLE_RATE, DEFAULT_PAN_POSITIONS,
    GLOBAL_FX_SLOT_COUNT, INSTRUMENT_SLOT_COUNT, MAX_CONTROL_EVENTS_PER_CALLBACK,
    MAX_SAMPLE_VOICES, MAX_SAMPLE_VOICES_PER_SLOT, MAX_SYNTH_VOICES, MAX_SYNTH_VOICES_PER_SLOT,
    RENDER_PROFILE_STAGE_COUNT, SAMPLE_SLOTS_PER_INSTRUMENT, SAMPLE_VOICE_LANE_CAPACITY,
    SAMPLE_VOICE_RETIREMENT_CAPACITY, SYNTH_VOICE_LANE_CAPACITY,
};

#[cfg(test)]
mod test_allocator {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static ENABLED: Cell<bool> = const { Cell::new(false) };
        static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
        static DEALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    }

    struct CountingAllocator;

    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = System.alloc(layout);
            count_allocation();
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            ENABLED.with(|enabled| {
                if enabled.get() {
                    DEALLOCATIONS.with(|deallocations| deallocations.set(deallocations.get() + 1));
                }
            });
            System.dealloc(pointer, layout);
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let pointer = System.realloc(pointer, layout, new_size);
            count_allocation();
            count_deallocation();
            pointer
        }
    }

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    fn count_allocation() {
        ENABLED.with(|enabled| {
            if enabled.get() {
                ALLOCATIONS.with(|allocations| allocations.set(allocations.get() + 1));
            }
        });
    }

    fn count_deallocation() {
        ENABLED.with(|enabled| {
            if enabled.get() {
                DEALLOCATIONS.with(|deallocations| deallocations.set(deallocations.get() + 1));
            }
        });
    }

    pub(crate) fn count_allocations_and_deallocations<F, R>(operation: F) -> (R, usize, usize)
    where
        F: FnOnce() -> R,
    {
        ALLOCATIONS.with(|allocations| allocations.set(0));
        DEALLOCATIONS.with(|deallocations| deallocations.set(0));
        ENABLED.with(|enabled| enabled.set(true));
        let result = operation();
        ENABLED.with(|enabled| enabled.set(false));
        let allocations = ALLOCATIONS.with(Cell::get);
        let deallocations = DEALLOCATIONS.with(Cell::get);
        (result, allocations, deallocations)
    }
}

#[cfg(test)]
mod capability_tests {
    use super::{
        MAX_CONTROL_EVENTS_PER_CALLBACK, SAMPLE_VOICE_LANE_CAPACITY,
        SAMPLE_VOICE_RETIREMENT_CAPACITY, SYNTH_VOICE_LANE_CAPACITY,
    };

    const _: () = assert!(
        SAMPLE_VOICE_RETIREMENT_CAPACITY
            == SAMPLE_VOICE_LANE_CAPACITY + (2 * MAX_CONTROL_EVENTS_PER_CALLBACK)
    );

    #[test]
    fn physical_voice_lane_capacities_match_the_capability_contract() {
        assert_eq!(SYNTH_VOICE_LANE_CAPACITY, 64);
        assert_eq!(SAMPLE_VOICE_LANE_CAPACITY, 64);
        assert_eq!(
            SAMPLE_VOICE_RETIREMENT_CAPACITY,
            SAMPLE_VOICE_LANE_CAPACITY + (2 * MAX_CONTROL_EVENTS_PER_CALLBACK)
        );
    }
}
