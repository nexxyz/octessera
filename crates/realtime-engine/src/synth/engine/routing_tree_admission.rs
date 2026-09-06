use super::prepared_control_prepare::{
    PreparedAudioConfig, PreparedFxBusSlot, PreparedInstrumentSlot, PreparedInstrumentsConfig,
};
use super::render_plan::RenderPlan;
use super::routing_tree_executor::{RoutingTreeAssignment, RoutingTreeBlockScratch};
use super::routing_tree_plan::RoutingTreePlan;
use super::routing_tree_worker::RoutingTreeOutputBlock;
use super::SynthEngine;
use crate::synth::types::{SynthProfileSnapshot, BUS_COUNT};

impl SynthEngine {
    pub fn sample_clock(&self) -> u64 {
        self.sample_clock
    }

    pub fn take_routing_tree_rejection(&mut self) -> bool {
        let rejected = self.routing_tree_rejection;
        self.routing_tree_rejection = false;
        rejected
    }

    fn reject_routing_tree_mutation(&mut self) {
        self.routing_tree_rejection = true;
    }

    pub fn with_routing_tree_source_event_sample_clock<R>(
        &mut self,
        sample_clock: u64,
        apply: impl FnOnce(&mut SynthEngine) -> R,
    ) -> R {
        let previous = self.routing_tree_source_event_sample_clock;
        self.routing_tree_source_event_sample_clock = Some(sample_clock);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| apply(self)));
        self.routing_tree_source_event_sample_clock = previous;
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(super) fn enable_routing_tree(&mut self) -> bool {
        let Some(assignment) = RoutingTreeBlockScratch::assignment_for_engine(self) else {
            self.reject_routing_tree_mutation();
            return false;
        };
        self.routing_tree_assignment = Some(assignment);
        self.apply_routing_tree_bus_assignment();
        true
    }

    pub(super) fn routing_tree_assignment(&self) -> Option<RoutingTreeAssignment> {
        self.routing_tree_assignment
    }

    pub(super) fn refresh_routing_tree_assignment(&mut self) -> bool {
        let Some(current) = self.routing_tree_assignment else {
            return false;
        };
        let Some(mut next) = RoutingTreeBlockScratch::assignment_for_engine_unvalidated(self)
        else {
            self.reject_routing_tree_mutation();
            return false;
        };
        if self.routing_tree_notes_started && !current.has_same_component_worker_mapping(&next) {
            next.preserve_component_worker_mapping(&current);
        }
        if !next.validate_engine(self) {
            self.reject_routing_tree_mutation();
            return false;
        }
        self.routing_tree_assignment = Some(next);
        self.apply_routing_tree_bus_assignment();
        true
    }

    pub(super) fn routing_tree_mark_note_event(&mut self) {
        self.routing_tree_notes_started = true;
    }

    pub(super) fn routing_tree_assignment_is_valid(&self) -> bool {
        let Some(assignment) = self.routing_tree_assignment.as_ref() else {
            return false;
        };
        if !assignment
            .plan
            .same_structure(&RoutingTreePlan::from_render_plan(&self.render_plan))
        {
            return false;
        }
        if self.synth_voice_pool.has_home() && self.sample_voice_pool.has_home() {
            assignment.validate_engine(self)
        } else {
            true
        }
    }

    pub(super) fn routing_tree_prepared_audio_allowed(&self, config: &PreparedAudioConfig) -> bool {
        self.routing_tree_source_event_sample_clock.is_some()
            && config.instruments.bus_chains.len() <= BUS_COUNT
    }

    pub(super) fn routing_tree_render_plan_allowed(&self, plan: &RenderPlan) -> bool {
        let _ = plan;
        self.routing_tree_source_event_sample_clock.is_some()
    }

    pub(super) fn routing_tree_prepared_instruments_allowed(
        &self,
        config: &PreparedInstrumentsConfig,
    ) -> bool {
        self.routing_tree_source_event_sample_clock.is_some()
            && config.bus_chains.len() <= BUS_COUNT
    }

    pub(super) fn routing_tree_prepared_instrument_slot_allowed(
        &self,
        slot: usize,
        _config: &PreparedInstrumentSlot,
    ) -> bool {
        self.routing_tree_source_event_sample_clock.is_some()
            && self.render_plan.instrument_slots.get(slot).is_some()
    }

    pub(super) fn routing_tree_prepared_fx_bus_slot_allowed(
        &self,
        bus: usize,
        slot: usize,
        _config: &PreparedFxBusSlot,
    ) -> bool {
        self.routing_tree_source_event_sample_clock.is_some()
            && self
                .render_plan
                .bus_fx_slots
                .get(bus)
                .and_then(|slots| slots.get(slot))
                .is_some()
    }

    pub(super) fn reject_routing_tree_mutation_for_control(&mut self) {
        self.reject_routing_tree_mutation();
    }

    pub(super) fn set_routing_tree_profile(&mut self, outputs: [&RoutingTreeOutputBlock; 2]) {
        self.routing_tree_profile = SynthProfileSnapshot {
            active_synth_voices: outputs
                .iter()
                .map(|output| output.active_synth_voices)
                .sum(),
            active_sample_voices: outputs
                .iter()
                .map(|output| output.active_sample_voices)
                .sum(),
            active_preview_sample_voices: outputs
                .iter()
                .map(|output| output.active_preview_sample_voices)
                .sum(),
            active_momentary_fx: self.momentary_fx.len()
                + outputs
                    .iter()
                    .map(|output| output.active_momentary_fx)
                    .sum::<usize>(),
            active_bus_fx_slots: outputs
                .iter()
                .map(|output| output.active_bus_fx_slots)
                .sum(),
            active_global_fx_slots: self.master_active_slot_indices.len(),
            cumulative_voice_steals: self.cumulative_voice_steals,
            cumulative_voice_admission_drops: self.cumulative_voice_admission_drops,
        };
    }

    fn apply_routing_tree_bus_assignment(&mut self) {
        let Some(assignment) = self.routing_tree_assignment else {
            return;
        };
        for bus in 0..self.bus_chains.len() {
            self.bus_chains[bus].assigned_worker = assignment.worker_for_bus(bus);
        }
    }
}
