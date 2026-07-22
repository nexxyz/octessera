mod audio_config;
mod engine;
mod fx;
mod fx_params;
mod runtime_state;
#[cfg(test)]
mod tests;
mod types;

pub use audio_config::{
    normalize_audio_config, normalize_fx_slot, normalize_instrument_slot_config,
    parse_voice_stealing_mode, validate_fx_type, validate_momentary_fx_type,
    validate_sample_bank_param_path, validate_synth_param_path, NormalizedAudioConfig,
    NormalizedInstrumentSlot, NormalizedSampleConfig,
};
pub use engine::{
    prepare_audio_config, prepare_fx_bus_slot, prepare_global_fx_slot,
    prepare_instrument_slot_config, prepare_instruments_config, prepare_momentary_fx_start,
    PreparedAudioConfig, PreparedFxBusSlot, PreparedGlobalFxSlot, PreparedInstrumentSlot,
    PreparedInstrumentsConfig, PreparedMomentaryFxStart, SynthEngine,
};
pub use types::{
    default_synth_config, AudioLoadStatus, EnvConfig, FilterConfig, FilterType, FxBusConfig,
    FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MasterFxConfig, MixerConfig, MomentaryFxTarget, OscConfig, RenderProfileSnapshot,
    SampleBankConfig, SampleBuffer, SampleSlotConfig, SynthConfig, SynthProfileSnapshot,
    VoiceStealingMode, BUS_FX_WARNING_SLOT_COUNT, BUS_SLOTS_PER_BUS, DEFAULT_AUDIO_BLOCK_FRAMES,
    DEFAULT_AUDIO_SAMPLE_RATE, DEFAULT_PAN_POSITIONS, DEFAULT_SYNTH_SLOT_WORKERS,
    GLOBAL_FX_SLOT_COUNT, INSTRUMENT_SLOT_COUNT, MAX_SAMPLE_VOICES, MAX_SAMPLE_VOICES_PER_SLOT,
    MAX_SYNTH_VOICES, MAX_SYNTH_VOICES_PER_SLOT, RENDER_PROFILE_STAGE_COUNT,
    SAMPLE_SLOTS_PER_INSTRUMENT, VOICES_PER_SLOT,
};

#[cfg(test)]
mod test_allocator {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static ENABLED: Cell<bool> = const { Cell::new(false) };
        static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    }

    struct CountingAllocator;

    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = System.alloc(layout);
            count_allocation();
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            System.dealloc(pointer, layout);
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let pointer = System.realloc(pointer, layout, new_size);
            count_allocation();
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

    pub(crate) fn count<F, R>(operation: F) -> (R, usize)
    where
        F: FnOnce() -> R,
    {
        ALLOCATIONS.with(|allocations| allocations.set(0));
        ENABLED.with(|enabled| enabled.set(true));
        let result = operation();
        ENABLED.with(|enabled| enabled.set(false));
        let allocations = ALLOCATIONS.with(Cell::get);
        (result, allocations)
    }
}
