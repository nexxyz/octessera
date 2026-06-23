mod engine;
mod fx;
mod fx_params;
mod runtime_state;
#[cfg(test)]
mod tests;
mod types;

pub use engine::SynthEngine;
pub use types::{
    default_synth_config, AudioLoadStatus, EnvConfig, FilterConfig, FilterType, FxBusConfig,
    FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MasterFxConfig, MixerConfig, MomentaryFxTarget, OscConfig, SampleBankConfig, SampleBuffer,
    SampleSlotConfig, SynthConfig, SynthProfileSnapshot, VoiceStealingMode, BUS_SLOTS_PER_BUS,
    DEFAULT_AUDIO_BLOCK_FRAMES, DEFAULT_AUDIO_SAMPLE_RATE, DEFAULT_PAN_POSITIONS,
    INSTRUMENT_SLOT_COUNT, MAX_ACTIVE_BUS_FX_SLOTS, MAX_ACTIVE_GLOBAL_FX_SLOTS, MAX_SAMPLE_VOICES,
    MAX_SAMPLE_VOICES_PER_SLOT, MAX_SYNTH_VOICES, MAX_SYNTH_VOICES_PER_SLOT,
    SAMPLE_SLOTS_PER_INSTRUMENT, VOICES_PER_SLOT,
};
