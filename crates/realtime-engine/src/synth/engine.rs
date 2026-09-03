use super::dsp_config::{DspRuntimeConfig, WorkerLoadWarningState};
use super::fx::{
    fx_bus_state_from_params, fx_bus_state_matches_params, master_fx_state_from_params,
    master_fx_state_matches_params, process_master_fx_slot, FxBusState, MasterFxState,
};
use super::fx_params::{compile_fx_bus_params, FxBusParams};
use super::runtime_state::*;
use super::synth_voice_pool::SynthVoicePool;
use super::types::*;
use render_voice::{refresh_synth_voice_render_cache, SynthVoiceRenderConfig};
use serde_json::Value;
use std::collections::BTreeMap;

#[cfg(test)]
mod admission_tests;
mod bus_chain_owner;
#[cfg(test)]
mod bus_chain_owner_tests;
mod control;
#[cfg(test)]
mod control_tests;
mod dynamic_control;
mod inline_source_executor;
#[cfg(test)]
mod lifecycle_tests;
mod note_control;
#[cfg(test)]
mod output_stereo_bus_tests;
mod prepared_control_apply;
mod prepared_control_prepare;
mod render;
#[cfg(test)]
mod render_block_tests;
mod render_momentary_fx;
mod render_plan;
mod render_profile;
mod render_routing;
mod render_samples;
mod render_synth;
#[cfg(test)]
mod render_synth_block_tests;
#[cfg(test)]
mod render_tests;
mod render_voice;
mod retired_state;
#[cfg(test)]
mod sample_buffer_view_tests;
#[cfg(test)]
mod sample_filter_block_tests;
mod sample_voice_pool;
#[cfg(test)]
mod source_lane_prefix_tests;
mod source_lane_renderer;
mod source_worker;
#[cfg(test)]
mod source_worker_failure_tests;
mod source_worker_health;
#[cfg(test)]
mod source_worker_identity_tests;
mod source_worker_lease;
mod source_worker_lifecycle;
mod source_worker_load;
#[cfg(test)]
mod source_worker_load_integration_tests;
#[cfg(any(test, feature = "test-support"))]
mod source_worker_observer;
mod source_worker_owner;
#[cfg(test)]
mod source_worker_parity_tests;
mod source_worker_placement;
#[cfg(test)]
mod source_worker_placement_tests;
mod source_worker_protocol;
mod source_worker_retirement;
#[cfg(test)]
mod source_worker_retirement_tests;
#[cfg(test)]
mod source_worker_start_hook_tests;
#[cfg(test)]
#[path = "engine/source_worker_test_fixtures.rs"]
mod source_worker_test_fixtures;
#[cfg(test)]
mod source_worker_tests;
#[cfg(all(test, feature = "source-worker-benchmark-timing"))]
#[path = "engine/source_worker_timing_integration_tests.rs"]
mod source_worker_timing_integration_tests;
mod source_worker_transfer;
mod support;
#[cfg(test)]
mod test_support;
mod voice_budget;

pub use prepared_control_prepare::{
    prepare_audio_config, prepare_fx_bus_slot, prepare_global_fx_slot,
    prepare_instrument_slot_config, prepare_instruments_config, prepare_momentary_fx_start,
    PreparedAudioConfig, PreparedFxBusSlot, PreparedGlobalFxSlot, PreparedInstrumentSlot,
    PreparedInstrumentsConfig, PreparedMomentaryFxStart,
};
pub use retired_state::RetiredAudioState;
use retired_state::{store_retired_preview, PREVIEW_AUDITION_SLOTS};
pub use source_worker::SourceWorkerRuntime;
pub use source_worker_health::{SourceWorkerHealth, SourceWorkerHealthSnapshot};
pub use source_worker_lifecycle::SourceWorkerLifecycle;
pub use source_worker_lifecycle::SOURCE_WORKER_THREAD_NAMES;
pub use source_worker_load::{
    SourceWorkerLoadSnapshot, SOURCE_WORKER_MAX_COST_UNITS, SOURCE_WORKER_SAMPLE_COST_UNITS,
    SOURCE_WORKER_SYNTH_COST_UNITS,
};
#[cfg(any(test, feature = "test-support"))]
pub use source_worker_observer::{
    install_source_worker_shutdown_probe_for_test, SourceWorkerOwnerIdentity,
    SourceWorkerShutdownProbeGuard,
};
pub use source_worker_protocol::{
    SourceWorkerMode, SourceWorkerRetirementError, SourceWorkerSetupError, SourceWorkerShutdown,
    SourceWorkerStartHook, SOURCE_WORKER_MODE_INLINE, SOURCE_WORKER_MODE_PERSISTENT,
};
#[cfg(any(test, feature = "test-support"))]
pub use source_worker_retirement::SourceWorkerHoldControl;
pub use source_worker_retirement::SourceWorkerRetirement;

use bus_chain_owner::{BusChainFrameOutput, BusChainOwner};
pub use bus_chain_owner::{BUS_CHAIN_SLOT_COST_UNITS, BUS_CHAIN_WORKER_MAX_COST_UNITS};
use control::MAX_MOMENTARY_FX;
use inline_source_executor::InlineSourceExecutor;
use render_plan::RenderPlan;
use render_profile::RenderProfileState;
use render_routing::FxBusOutputSpreadState;
use sample_voice_pool::SampleVoicePool;
use support::{
    midi_note_to_hz, mono_frame, pan_gains, pan_gains_float, parse_instrument_kind,
    parse_momentary_fx_kind, parse_route, sample_slot_for_note, InstrumentKind, MomentaryFxKind,
    MomentaryFxRuntimeParams, MomentaryFxState, PreviewSampleVoice, SampleVoice,
    DRY_HISTORY_FRAMES,
};

#[cfg(test)]
pub(in crate::synth) const FREEZE_INJECT_MS: u32 = support::FREEZE_INJECT_MS;

pub struct SynthEngine {
    sample_rate: u32,
    sample_clock: u64,
    slot_kind: [InstrumentKind; INSTRUMENT_SLOT_COUNT],
    instruments: [SynthConfig; INSTRUMENT_SLOT_COUNT],
    synth_render_configs: [SynthVoiceRenderConfig; INSTRUMENT_SLOT_COUNT],
    synth_render_revisions: [u32; INSTRUMENT_SLOT_COUNT],
    sample_banks: Vec<SampleBankConfig>,
    mods: [InstrumentMod; INSTRUMENT_SLOT_COUNT],
    synth_voice_pool: SynthVoicePool,
    sample_voice_pool: SampleVoicePool,
    active_synth_slots: [bool; INSTRUMENT_SLOT_COUNT],
    active_sample_slots: [bool; INSTRUMENT_SLOT_COUNT],
    preview_sample_voices: [Option<PreviewSampleVoice>; PREVIEW_AUDITION_SLOTS],
    preview_sample_orders: [u64; PREVIEW_AUDITION_SLOTS],
    preview_sample_next_order: u64,
    pending_render_retired: RetiredAudioState,
    render_plan: RenderPlan,
    source_worker_load: Option<SourceWorkerLoadSnapshot>,
    slot_route: [usize; INSTRUMENT_SLOT_COUNT],
    slot_pan_pos: [usize; INSTRUMENT_SLOT_COUNT],
    slot_pan_gains: [(f32, f32); INSTRUMENT_SLOT_COUNT],
    slot_volume: [f32; INSTRUMENT_SLOT_COUNT],
    bus_pan_pos: Vec<usize>,
    bus_pan_gains_cache: Vec<(f32, f32)>,
    bus_volume: Vec<f32>,
    bus_mono_scratch: Vec<f32>,
    bus_mono_snapshot: Vec<f32>,
    bus_output_spread_state: Vec<FxBusOutputSpreadState>,
    bus_chains: Vec<BusChainOwner>,
    active_bus_activity_count: usize,
    routed_bus_slot_count: usize,
    master_slot_params: Vec<FxBusParams>,
    master_slot_state: Vec<MasterFxState>,
    master_active_slot_indices: Vec<usize>,
    master_activity_frames: u32,
    pan_positions: usize,
    master_volume: f32,
    voice_stealing_mode: VoiceStealingMode,
    smoothed_load_ratio: f32,
    voice_steal_since_status: bool,
    cumulative_voice_steals: u64,
    cumulative_voice_admission_drops: u64,
    momentary_fx: Vec<MomentaryFxState>,
    dry_history: Vec<f32>,
    dry_history_pos: usize,
    fx_activity_hold_frames: u32,
    render_profile: RenderProfileState,
    block_slot_scratch: BlockSlotScratch,
    dsp_config: DspRuntimeConfig,
    worker_utilization_ppm: Option<u32>,
    worker_load_warning: WorkerLoadWarningState,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct SlotFrameOutput {
    pub sample: f32,
    pub active: bool,
}

pub(super) const BLOCK_SLOT_SCRATCH_FRAMES: usize = 2048;

pub(super) struct BlockSlotScratch {
    inline_source_executor: Option<InlineSourceExecutor>,
    sample_slot_out: [Vec<f32>; INSTRUMENT_SLOT_COUNT],
    synth_slot_out: [Vec<f32>; INSTRUMENT_SLOT_COUNT],
    sample_active: [Vec<bool>; INSTRUMENT_SLOT_COUNT],
    synth_active: [Vec<bool>; INSTRUMENT_SLOT_COUNT],
}

impl BlockSlotScratch {
    fn new() -> Self {
        Self {
            inline_source_executor: Some(InlineSourceExecutor::new()),
            sample_slot_out: std::array::from_fn(|_| vec![0.0; BLOCK_SLOT_SCRATCH_FRAMES]),
            synth_slot_out: std::array::from_fn(|_| vec![0.0; BLOCK_SLOT_SCRATCH_FRAMES]),
            sample_active: std::array::from_fn(|_| vec![false; BLOCK_SLOT_SCRATCH_FRAMES]),
            synth_active: std::array::from_fn(|_| vec![false; BLOCK_SLOT_SCRATCH_FRAMES]),
        }
    }

    fn prepare_output(&mut self, frames: usize) -> bool {
        if frames > BLOCK_SLOT_SCRATCH_FRAMES {
            return false;
        }
        for buffer in &mut self.sample_slot_out {
            buffer[..frames].fill(0.0);
        }
        for buffer in &mut self.synth_slot_out {
            buffer[..frames].fill(0.0);
        }
        for buffer in &mut self.sample_active {
            buffer[..frames].fill(false);
        }
        for buffer in &mut self.synth_active {
            buffer[..frames].fill(false);
        }
        true
    }
}

impl SynthEngine {
    fn voice_pools_home(&self) -> bool {
        self.synth_voice_pool.has_home() && self.sample_voice_pool.has_home()
    }

    pub fn new(sample_rate: u32) -> Self {
        let default = default_synth_config();
        let default_render = SynthVoiceRenderConfig::from_config(default);
        Self {
            sample_rate,
            sample_clock: 0,
            slot_kind: [InstrumentKind::Synth; INSTRUMENT_SLOT_COUNT],
            instruments: [default; INSTRUMENT_SLOT_COUNT],
            synth_render_configs: [default_render; INSTRUMENT_SLOT_COUNT],
            synth_render_revisions: [0; INSTRUMENT_SLOT_COUNT],
            sample_banks: vec![SampleBankConfig::default(); INSTRUMENT_SLOT_COUNT],
            mods: [InstrumentMod::new(); INSTRUMENT_SLOT_COUNT],
            synth_voice_pool: SynthVoicePool::new(),
            sample_voice_pool: SampleVoicePool::new(),
            active_synth_slots: [false; INSTRUMENT_SLOT_COUNT],
            active_sample_slots: [false; INSTRUMENT_SLOT_COUNT],
            preview_sample_voices: std::array::from_fn(|_| None),
            preview_sample_orders: [0; PREVIEW_AUDITION_SLOTS],
            preview_sample_next_order: 0,
            pending_render_retired: RetiredAudioState::default(),
            render_plan: RenderPlan::new(),
            source_worker_load: None,
            slot_route: [0; INSTRUMENT_SLOT_COUNT],
            slot_pan_pos: [DEFAULT_PAN_POSITIONS / 2; INSTRUMENT_SLOT_COUNT],
            slot_pan_gains: [pan_gains(DEFAULT_PAN_POSITIONS / 2, DEFAULT_PAN_POSITIONS);
                INSTRUMENT_SLOT_COUNT],
            slot_volume: [1.0; INSTRUMENT_SLOT_COUNT],
            bus_pan_pos: Vec::new(),
            bus_pan_gains_cache: Vec::new(),
            bus_volume: Vec::new(),
            bus_mono_scratch: Vec::new(),
            bus_mono_snapshot: Vec::new(),
            bus_output_spread_state: Vec::new(),
            bus_chains: Vec::new(),
            active_bus_activity_count: 0,
            routed_bus_slot_count: 0,
            master_slot_params: Vec::new(),
            master_slot_state: Vec::new(),
            master_active_slot_indices: Vec::new(),
            master_activity_frames: 0,
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 1.0,
            voice_stealing_mode: VoiceStealingMode::AutoBalanced,
            smoothed_load_ratio: 0.0,
            voice_steal_since_status: false,
            cumulative_voice_steals: 0,
            cumulative_voice_admission_drops: 0,
            momentary_fx: Vec::with_capacity(MAX_MOMENTARY_FX),
            dry_history: vec![0.0; DRY_HISTORY_FRAMES * 2],
            dry_history_pos: 0,
            fx_activity_hold_frames: (sample_rate.saturating_mul(150) / 1000).max(1),
            render_profile: RenderProfileState::default(),
            block_slot_scratch: BlockSlotScratch::new(),
            dsp_config: DspRuntimeConfig::default(),
            worker_utilization_ppm: None,
            worker_load_warning: WorkerLoadWarningState::default(),
        }
    }

    pub(in crate::synth::engine) fn record_voice_steal(&mut self) {
        self.voice_steal_since_status = true;
        self.cumulative_voice_steals = self.cumulative_voice_steals.saturating_add(1);
    }

    pub(in crate::synth::engine) fn record_voice_admission_drop(&mut self) {
        self.cumulative_voice_admission_drops =
            self.cumulative_voice_admission_drops.saturating_add(1);
    }

    pub fn profile_snapshot(&self) -> SynthProfileSnapshot {
        if !self.voice_pools_home() {
            return SynthProfileSnapshot::default();
        }
        let active_synth_voices = self.synth_voice_pool.active_total().unwrap_or(0);
        let active_sample_voices = self.sample_voice_pool.active_total().unwrap_or(0);
        SynthProfileSnapshot {
            active_synth_voices,
            active_sample_voices,
            active_preview_sample_voices: self
                .preview_sample_voices
                .iter()
                .filter(|voice| voice.is_some())
                .count(),
            active_momentary_fx: self.momentary_fx.len(),
            active_bus_fx_slots: self
                .bus_chains
                .iter()
                .map(|chain| chain.active_slot_count)
                .sum(),
            active_global_fx_slots: self.master_active_slot_indices.len(),
            cumulative_voice_steals: self.cumulative_voice_steals,
            cumulative_voice_admission_drops: self.cumulative_voice_admission_drops,
        }
    }

    pub fn is_idle(&self) -> bool {
        !self.has_active_synth_voices()
            && !self.has_active_sample_voices()
            && self.preview_sample_voices.iter().all(Option::is_none)
            && self.momentary_fx.is_empty()
            && self.active_bus_activity_count == 0
            && self
                .bus_chains
                .iter()
                .all(|chain| chain.assigned_worker.is_none())
            && self.master_activity_frames == 0
    }

    fn has_active_synth_voices(&self) -> bool {
        self.synth_voice_pool
            .active_total()
            .is_some_and(|count| count > 0)
    }

    fn has_active_sample_voices(&self) -> bool {
        self.sample_voice_pool
            .active_total()
            .is_some_and(|count| count > 0)
    }

    pub fn take_pending_render_retired(&mut self) -> RetiredAudioState {
        std::mem::take(&mut self.pending_render_retired)
    }

    pub fn pending_render_retired_is_empty(&self) -> bool {
        self.pending_render_retired
            .preview_sample_voices
            .iter()
            .all(Option::is_none)
            && self.pending_render_retired.bus_chains.is_empty()
            && self
                .pending_render_retired
                .displaced_momentary_fx
                .iter()
                .all(Option::is_none)
            && self.pending_render_retired.sample_voices.is_empty()
    }

    pub fn dsp_config(&self) -> DspRuntimeConfig {
        self.dsp_config
    }

    pub(super) fn observe_worker_utilization(
        &mut self,
        utilization_ppm: u32,
        rendered_frames: usize,
    ) {
        self.worker_utilization_ppm = Some(utilization_ppm);
        self.worker_load_warning
            .observe(utilization_ppm, rendered_frames, self.sample_rate);
    }

    pub(in crate::synth::engine) fn retire_render_preview(&mut self, voice: PreviewSampleVoice) {
        store_retired_preview(
            &mut self.pending_render_retired.preview_sample_voices,
            voice,
        );
    }
}
