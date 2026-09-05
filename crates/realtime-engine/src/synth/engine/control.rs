use super::super::fx_params::FxKind;
use super::bus_chain_owner::fx_kind_cost;
use super::render_plan::{prepared_instrument_topology, RenderPlan, RenderPlanInstrumentSlot};
use super::retired_state::{store_retired_momentary, RetiredAudioState};
use super::support::stutter_segment_len;
use super::*;

pub(super) const MAX_MOMENTARY_FX: usize = 2;

#[derive(Clone, Copy)]
pub(super) struct NormalizedInstrumentMixer {
    pub(super) route: usize,
    pub(super) pan_pos: usize,
    pub(super) volume: f32,
}

struct CompiledBusMixerState {
    pan_positions: Vec<usize>,
    pan_gains: Vec<(f32, f32)>,
    volumes: Vec<f32>,
    chains: Vec<BusChainOwner>,
}

impl SynthEngine {
    pub fn momentary_fx_start(
        &mut self,
        id: String,
        fx_type: String,
        params: BTreeMap<String, Value>,
        target: MomentaryFxTarget,
    ) {
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some() && target != MomentaryFxTarget::Global {
            self.reject_routing_tree_mutation_for_control();
            return;
        }
        let Some(kind) = parse_momentary_fx_kind(&fx_type) else {
            return;
        };
        if let Some(pos) = self.momentary_fx.iter().position(|fx| fx.id == id) {
            self.momentary_fx.remove(pos);
        }
        if self.momentary_fx.iter().any(|fx| fx.kind == kind) {
            return;
        }
        if self.momentary_fx.len() >= MAX_MOMENTARY_FX {
            return;
        }
        self.momentary_fx.push(MomentaryFxState::new(
            id,
            kind,
            &params,
            target,
            self.sample_rate,
        ));

        if kind == MomentaryFxKind::PitchShift {
            let fx = self.momentary_fx.last_mut().expect("inserted momentary FX");
            fx.pitch_shifter
                .prefill_from_ring(&self.dry_history, self.dry_history_pos);
        }
    }

    pub fn momentary_fx_stop(&mut self, id: &str) -> RetiredAudioState {
        let mut retired = RetiredAudioState::default();
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some() && !self.routing_tree_momentary_stop_allowed(id) {
            self.reject_routing_tree_mutation_for_control();
            return retired;
        }
        let Some(pos) = self.momentary_fx.iter().position(|fx| fx.id == id) else {
            return retired;
        };
        let should_remove = matches!(
            self.momentary_fx[pos].kind,
            MomentaryFxKind::Stutter | MomentaryFxKind::PitchShift
        );
        if should_remove {
            store_retired_momentary(
                &mut retired.displaced_momentary_fx,
                self.momentary_fx.remove(pos),
            );
        } else {
            let fx = &mut self.momentary_fx[pos];
            fx.releasing = true;
            fx.release_pos = 0;
            if fx.kind == MomentaryFxKind::Freeze {
                if let MomentaryFxRuntimeParams::Freeze { release_len, .. } = fx.runtime_params {
                    fx.release_len = release_len;
                }
            }
        }
        retired
    }

    pub fn momentary_fx_update(&mut self, id: &str, params: &BTreeMap<String, Value>) {
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some() && !self.routing_tree_momentary_update_allowed(id)
        {
            self.reject_routing_tree_mutation_for_control();
            return;
        }
        if let Some(fx) = self.momentary_fx.iter_mut().find(|fx| fx.id == id) {
            fx.runtime_params =
                MomentaryFxRuntimeParams::from_params(fx.kind, params, self.sample_rate);
            if fx.kind == MomentaryFxKind::Stutter {
                fx.stutter_segment_len = stutter_segment_len(self.sample_rate, params);
                fx.stutter_write = 0;
                fx.stutter_ready = false;
                fx.stutter_ramp_pos = 0;
            }
        }
    }

    pub fn set_voice_stealing_mode(&mut self, mode: VoiceStealingMode) {
        self.voice_stealing_mode = mode;
    }

    pub fn set_runtime_load_ratio(&mut self, ratio: f32) {
        let r = ratio.clamp(0.0, 2.0);
        self.smoothed_load_ratio = 0.9 * self.smoothed_load_ratio + 0.1 * r;
    }

    pub fn audio_load_status(&mut self) -> AudioLoadStatus {
        let status = AudioLoadStatus {
            ratio: self.smoothed_load_ratio,
            voice_steal: self.voice_steal_since_status,
            worker_utilization: self
                .worker_utilization_ppm
                .map(|utilization| utilization as f32 / 1_000_000.0),
            high_cpu_steady: self.worker_load_warning.high_cpu_steady(),
            missed_quantum_flash: false,
            block_ratio_p95: 0.0,
            block_ratio_max: 0.0,
            blocks: 0,
            control_events: 0,
            config_events: 0,
            rendered_quantums: 0,
            repeated_quantums: 0,
            dropped_quantums: 0,
            deadline_misses: 0,
            deadline_recoveries: 0,
        };
        self.voice_steal_since_status = false;
        status
    }

    pub fn set_instruments(&mut self, cfg: InstrumentsConfig) {
        if self.persistent_bus_limit.is_some_and(|limit| {
            cfg.mixer
                .as_ref()
                .is_some_and(|mixer| mixer.buses.len() > limit)
        }) {
            #[cfg(feature = "routing-tree-benchmark")]
            self.reject_routing_tree_mutation_for_control();
            return;
        }
        let mut next_render_plan = RenderPlan::from_config(&cfg);
        for (index, slot) in cfg
            .instruments
            .iter()
            .take(INSTRUMENT_SLOT_COUNT)
            .enumerate()
        {
            let topology = prepared_instrument_topology(slot);
            next_render_plan.instrument_slots[index] = RenderPlanInstrumentSlot {
                kind: topology.kind,
                occupied: topology.occupied,
                route: topology
                    .route
                    .unwrap_or(self.render_plan.instrument_slots[index].route),
            };
        }
        for index in cfg.instruments.len().min(INSTRUMENT_SLOT_COUNT)..INSTRUMENT_SLOT_COUNT {
            let current = self.render_plan.instrument_slots[index];
            next_render_plan.instrument_slots[index] = RenderPlanInstrumentSlot {
                kind: current.kind,
                occupied: false,
                route: current.route,
            };
        }
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some()
            && !self.routing_tree_render_plan_allowed(&next_render_plan)
        {
            self.reject_routing_tree_mutation_for_control();
            return;
        }
        self.pan_positions = cfg.pan_positions.max(1);
        self.master_volume = (cfg.master_volume / 100.0).clamp(0.0, 1.0);
        self.apply_instrument_slots_config(cfg.instruments);
        self.refresh_slot_pan_gains();
        let mut next_bus = self.compile_bus_mixer_state(cfg.mixer.as_ref());
        let (next_master_slot_params, next_master_slot_state) =
            self.compile_master_mixer_state(cfg.mixer.as_ref());
        let mut previous_bus_chains = std::mem::take(&mut self.bus_chains);
        for next in &mut next_bus.chains {
            if let Some(previous) = previous_bus_chains
                .iter_mut()
                .find(|previous| previous.logical_bus_id == next.logical_bus_id)
            {
                next.preserve_state_from(previous);
            }
        }
        self.pending_render_retired
            .bus_chains
            .append(&mut previous_bus_chains);
        self.bus_pan_pos = next_bus.pan_positions;
        self.bus_pan_gains_cache = next_bus.pan_gains;
        self.bus_volume = next_bus.volumes;
        self.bus_chains = next_bus.chains;
        self.refresh_routed_bus_slot_count();
        self.bus_output_spread_state
            .resize_with(self.bus_chains.len(), || {
                FxBusOutputSpreadState::new(self.sample_rate)
            });
        self.active_bus_activity_count = self
            .bus_chains
            .iter()
            .filter(|chain| chain.is_active())
            .count();
        self.master_slot_params = next_master_slot_params;
        self.master_slot_state = next_master_slot_state;
        self.refresh_master_active_slot_indices();
        self.master_activity_frames = 0;
        self.bus_mono_scratch.resize(self.bus_chains.len(), 0.0);
        drop(self.render_plan.install_complete(next_render_plan));
    }

    fn apply_instrument_slots_config(&mut self, instruments: Vec<InstrumentSlotConfig>) {
        for (idx, slot) in instruments.into_iter().enumerate() {
            if idx >= INSTRUMENT_SLOT_COUNT {
                break;
            }
            self.apply_instrument_slot_config(idx, slot);
        }
    }

    pub fn set_instrument_slot(&mut self, index: usize, slot: InstrumentSlotConfig) {
        if index >= INSTRUMENT_SLOT_COUNT {
            #[cfg(feature = "routing-tree-benchmark")]
            if self.routing_tree_assignment.is_some() {
                self.reject_routing_tree_mutation_for_control();
            }
            return;
        }
        let render_plan = prepared_instrument_topology(&slot);
        #[cfg(feature = "routing-tree-benchmark")]
        if self.routing_tree_assignment.is_some() {
            let mut next_render_plan = self.render_plan.clone();
            next_render_plan.install_instrument_slot(index, render_plan);
            if !self.routing_tree_render_plan_allowed(&next_render_plan) {
                self.reject_routing_tree_mutation_for_control();
                return;
            }
        }
        self.apply_instrument_slot_config(index, slot);
        self.refresh_slot_pan_gains();
        self.render_plan.install_instrument_slot(index, render_plan);
        self.refresh_routed_bus_slot_count();
    }

    fn apply_instrument_slot_config(&mut self, idx: usize, slot: InstrumentSlotConfig) {
        let InstrumentSlotConfig { kind, synth, mixer } = slot;
        let kind = parse_instrument_kind(&kind);
        let mixer = mixer.map(|mixer| NormalizedInstrumentMixer {
            route: parse_route(&mixer.route),
            pan_pos: mixer.pan_pos.min(self.pan_positions - 1),
            volume: (mixer.volume / 100.0).clamp(0.0, 1.0),
        });
        self.apply_normalized_instrument_slot(
            idx,
            kind,
            synth,
            SynthVoiceRenderConfig::from_config(synth),
            mixer,
        );
    }

    pub(super) fn apply_normalized_instrument_slot(
        &mut self,
        idx: usize,
        kind: InstrumentKind,
        synth: SynthConfig,
        render_config: SynthVoiceRenderConfig,
        mixer: Option<NormalizedInstrumentMixer>,
    ) {
        self.slot_kind[idx] = kind;
        if kind == InstrumentKind::Synth {
            self.instruments[idx] = synth;
            self.synth_render_configs[idx] = render_config;
            self.synth_render_revisions[idx] = self.synth_render_revisions[idx].wrapping_add(1);
        }
        if let Some(mixer) = mixer {
            self.apply_normalized_instrument_mixer(
                idx,
                Some(mixer.route),
                Some(mixer.pan_pos),
                Some(mixer.volume),
            );
        }
    }

    pub(super) fn apply_normalized_instrument_mixer(
        &mut self,
        idx: usize,
        route: Option<usize>,
        pan_pos: Option<usize>,
        volume: Option<f32>,
    ) {
        if let Some(route) = route {
            self.slot_route[idx] = route;
        }
        if let Some(pan_pos) = pan_pos {
            self.slot_pan_pos[idx] = pan_pos;
        }
        if let Some(volume) = volume {
            self.slot_volume[idx] = volume;
        }
    }

    pub(super) fn refresh_routed_bus_slot_count(&mut self) {
        let bus_count = self.bus_pan_pos.len();
        self.routed_bus_slot_count = self
            .slot_route
            .iter()
            .filter(|route| **route > 0 && **route <= bus_count)
            .count();
    }

    fn refresh_slot_pan_gains(&mut self) {
        for idx in 0..INSTRUMENT_SLOT_COUNT {
            self.slot_pan_gains[idx] = pan_gains(self.slot_pan_pos[idx], self.pan_positions);
        }
    }

    fn compile_bus_mixer_state(&self, mixer: Option<&MixerConfig>) -> CompiledBusMixerState {
        let mut next_bus_pan_pos = Vec::new();
        let mut next_bus_pan_gains = Vec::new();
        let mut next_bus_volumes = Vec::new();
        let Some(mixer) = mixer else {
            return CompiledBusMixerState {
                pan_positions: next_bus_pan_pos,
                pan_gains: next_bus_pan_gains,
                volumes: next_bus_volumes,
                chains: Vec::new(),
            };
        };
        next_bus_pan_pos.reserve_exact(mixer.buses.len());
        next_bus_pan_gains.reserve_exact(mixer.buses.len());
        next_bus_volumes.reserve_exact(mixer.buses.len());
        let mut next_bus_chains = Vec::with_capacity(mixer.buses.len());
        for (bus_idx, bus) in mixer.buses.iter().enumerate() {
            let pan_pos = bus.pan_pos.min(self.pan_positions - 1);
            next_bus_pan_pos.push(pan_pos);
            next_bus_pan_gains.push(pan_gains(pan_pos, self.pan_positions));
            next_bus_volumes.push((bus.volume_pct / 100.0).clamp(0.0, 1.0));
            let cfgs = compile_bus_slot_configs(bus);
            let params: [FxBusParams; BUS_SLOTS_PER_BUS] =
                std::array::from_fn(|j| compile_fx_bus_params(&cfgs[j]));
            let states: [FxBusState; BUS_SLOTS_PER_BUS] =
                std::array::from_fn(|j| fx_bus_state_from_params(&params[j], self.sample_rate));
            let costs = std::array::from_fn(|j| {
                fx_kind_cost(FxKind::parse(cfgs[j].kind_str()).unwrap_or(FxKind::None))
            });
            next_bus_chains.push(BusChainOwner::new(bus_idx, params, states, costs));
        }
        CompiledBusMixerState {
            pan_positions: next_bus_pan_pos,
            pan_gains: next_bus_pan_gains,
            volumes: next_bus_volumes,
            chains: next_bus_chains,
        }
    }

    pub(super) fn refresh_master_active_slot_indices(&mut self) {
        self.master_active_slot_indices.clear();
        self.master_active_slot_indices
            .reserve(self.master_slot_params.len());
        for (idx, params) in self.master_slot_params.iter().enumerate() {
            if !matches!(params, FxBusParams::None) {
                self.master_active_slot_indices.push(idx);
            }
        }
    }

    fn compile_master_mixer_state(
        &self,
        mixer: Option<&MixerConfig>,
    ) -> (Vec<FxBusParams>, Vec<MasterFxState>) {
        let mut next_master_slot_params = Vec::new();
        let mut next_master_slot_state = Vec::new();
        let Some(master) = mixer.and_then(|mixer| mixer.master.as_ref()) else {
            return (next_master_slot_params, next_master_slot_state);
        };
        let slot_count = master.slots.len().min(GLOBAL_FX_SLOT_COUNT);
        next_master_slot_params.reserve_exact(slot_count);
        next_master_slot_state.reserve_exact(slot_count);
        for (slot_idx, slot) in master.slots.iter().take(GLOBAL_FX_SLOT_COUNT).enumerate() {
            let params = compile_fx_bus_params(slot);
            let state = self
                .master_slot_state
                .get(slot_idx)
                .filter(|state| master_fx_state_matches_params(state, &params))
                .cloned()
                .unwrap_or_else(|| master_fx_state_from_params(&params));
            next_master_slot_params.push(params);
            next_master_slot_state.push(state);
        }
        (next_master_slot_params, next_master_slot_state)
    }
}

fn compile_bus_slot_configs(bus: &FxBusConfig) -> [FxBusSlotConfig; BUS_SLOTS_PER_BUS] {
    let mut cfgs: [FxBusSlotConfig; BUS_SLOTS_PER_BUS] =
        std::array::from_fn(|_| FxBusSlotConfig::Kind("none".to_string()));
    for (j, slot) in bus.slots.iter().enumerate().take(BUS_SLOTS_PER_BUS) {
        cfgs[j] = slot.clone();
    }
    cfgs
}
