use super::prepared_control_prepare::prepare_instruments_config;
use super::render_plan::prepared_instrument_topology;
use super::retired_state::{store_retired_momentary, RetiredAudioState};
use super::*;

pub(super) mod instrument_slot_control;

pub(super) const MAX_MOMENTARY_FX: usize = 2;
/// The two Play FX plus one releasing tail fading out after a new press took its place.
pub(super) const MOMENTARY_FX_CAPACITY: usize = MAX_MOMENTARY_FX + 1;

impl SynthEngine {
    pub fn momentary_fx_stop_by_epoch(&mut self, epoch: u64) -> RetiredAudioState {
        let retired = RetiredAudioState::default();
        if self.routing_tree_rejects_control_mutation() {
            return retired;
        }
        let Some(pos) = self.momentary_fx.iter().position(|fx| fx.epoch == epoch) else {
            return retired;
        };
        self.stop_momentary_fx_at(pos)
    }

    pub(super) fn stop_momentary_fx_at(&mut self, pos: usize) -> RetiredAudioState {
        let mut retired = RetiredAudioState::default();
        if self.momentary_fx[pos].stop() {
            store_retired_momentary(
                &mut retired.displaced_momentary_fx,
                self.momentary_fx.remove(pos),
            );
        }
        retired
    }

    pub fn set_voice_stealing_mode(&mut self, mode: VoiceStealingMode) {
        self.voice_stealing_mode = mode;
    }

    pub fn set_runtime_load_ratio(&mut self, ratio: f32) {
        let r = ratio.clamp(0.0, 2.0);
        self.smoothed_load_ratio = 0.9 * self.smoothed_load_ratio + 0.1 * r;
    }

    pub fn audio_load_status(&self) -> AudioLoadStatus {
        AudioLoadStatus {
            ratio: self.smoothed_load_ratio,
            worker_utilization: self
                .worker_utilization_ppm
                .map(|utilization| utilization as f32 / 1_000_000.0),
            missed_quantum_flash: false,
            output_clipped: false,
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
            peak: BlockPeak::default(),
        }
    }

    pub fn set_instruments(&mut self, cfg: InstrumentsConfig) {
        let prepared = prepare_instruments_config(cfg, self.sample_rate);
        drop(self.apply_prepared_instruments_config(prepared));
    }

    pub fn set_instrument_slot(&mut self, index: usize, slot: InstrumentSlotConfig) {
        if self.routing_tree_rejects_control_mutation() {
            return;
        }
        if index >= INSTRUMENT_SLOT_COUNT {
            #[cfg(feature = "routing-tree-executor")]
            if self.routing_tree_assignment.is_some() {
                self.reject_routing_tree_mutation_for_control();
            }
            return;
        }
        let render_plan = prepared_instrument_topology(&slot);
        #[cfg(feature = "routing-tree-executor")]
        if self.routing_tree_assignment.is_some() {
            let mut next_render_plan = self.render_plan.clone();
            next_render_plan.install_instrument_slot(index, render_plan);
            next_render_plan.normalize_routes();
            if !self.routing_tree_render_plan_allowed(&next_render_plan) {
                self.reject_routing_tree_mutation_for_control();
                return;
            }
        }
        self.finish_mixer_gain_ramps();
        self.apply_instrument_slot_config(index, slot);
        self.refresh_slot_pan_gains();
        let mut render_plan = render_plan;
        render_plan.route = render_plan.route.map(|route| match route {
            super::render_plan::RenderPlanRoute::Bus(bus) if bus >= self.bus_chains.len() => {
                super::render_plan::RenderPlanRoute::Direct
            }
            route => route,
        });
        self.render_plan.install_instrument_slot(index, render_plan);
        self.refresh_routed_bus_slot_count();
        #[cfg(feature = "routing-tree-executor")]
        if self.routing_tree_assignment.is_some() {
            let _ = self.refresh_routing_tree_assignment();
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
}
