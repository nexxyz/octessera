use super::*;

impl SynthEngine {
    pub fn new(sample_rate: u32) -> Self {
        let default = default_synth_config();
        let default_render = SynthVoiceRenderConfig::from_config(default);
        Self {
            sample_rate,
            sample_clock: 0,
            slot_kind: [InstrumentKind::Synth; INSTRUMENT_SLOT_COUNT],
            instruments: [default; INSTRUMENT_SLOT_COUNT],
            drum_voices: [DrumConfig::default().voices; INSTRUMENT_SLOT_COUNT],
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
            fx_activity_hold_frames: (sample_rate.saturating_mul(150) / 1000).max(1),
            render_profile: RenderProfileState::default(),
            block_slot_scratch: BlockSlotScratch::new(),
            #[cfg(test)]
            routing_tree_scratch: RoutingTreeBlockScratch::new(),
            #[cfg(feature = "routing-tree-benchmark")]
            routing_tree_assignment: None,
            #[cfg(feature = "routing-tree-benchmark")]
            routing_tree_notes_started: false,
            #[cfg(feature = "routing-tree-benchmark")]
            routing_tree_profile: SynthProfileSnapshot::default(),
            #[cfg(feature = "routing-tree-benchmark")]
            routing_tree_source_event_sample_clock: None,
            #[cfg(feature = "routing-tree-benchmark")]
            routing_tree_rejection: false,
            dsp_config: DspRuntimeConfig::default(),
            worker_utilization_ppm: None,
            worker_load_warning: WorkerLoadWarningState::default(),
            #[cfg(any(test, feature = "test-support"))]
            persistent_bus_limit: None,
        }
    }
}
