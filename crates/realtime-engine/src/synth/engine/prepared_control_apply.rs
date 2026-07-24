use super::control::active_fx_bus_slots;
use super::prepared_control_prepare::{
    PreparedAudioConfig, PreparedFxBusSlot, PreparedGlobalFxSlot, PreparedInstrumentSlot,
    PreparedInstrumentsConfig, PreparedMomentaryFxStart,
};
use super::retired_state::RetiredAudioState;
use super::support::{MomentaryFxKind, MomentaryFxState};
use super::*;

impl SynthEngine {
    pub fn apply_prepared_audio_config(
        &mut self,
        prepared: PreparedAudioConfig,
    ) -> RetiredAudioState {
        let PreparedAudioConfig {
            instruments,
            sample_banks,
            voice_stealing_mode,
        } = prepared;
        let mut retired = self.apply_prepared_instruments_config(instruments);
        if let Some(banks) = sample_banks {
            retired.sample_banks = Some(std::mem::replace(&mut self.sample_banks, banks));
            for pool in self.sample_voices.iter_mut() {
                for voice in pool.iter_mut() {
                    voice.active = false;
                }
            }
        }
        if let Some(mode) = voice_stealing_mode {
            self.voice_stealing_mode = mode;
        }
        retired
    }

    pub fn apply_prepared_instruments_config(
        &mut self,
        mut prepared: PreparedInstrumentsConfig,
    ) -> RetiredAudioState {
        let mut retired = RetiredAudioState::default();
        self.pan_positions = prepared.pan_positions;
        self.master_volume = prepared.master_volume;
        for (index, slot) in prepared.slots.iter().enumerate() {
            self.slot_kind[index] = slot.kind;
            if slot.kind == InstrumentKind::Synth {
                self.instruments[index] = slot.synth;
                self.synth_render_configs[index] = slot.render_config;
                self.synth_render_revisions[index] =
                    self.synth_render_revisions[index].wrapping_add(1);
            }
            if let Some(route) = slot.route {
                self.slot_route[index] = route;
                self.slot_pan_pos[index] = slot.pan_pos.min(self.pan_positions - 1);
                self.slot_volume[index] = slot.volume;
            }
        }
        for index in 0..INSTRUMENT_SLOT_COUNT {
            self.slot_pan_gains[index] =
                super::support::pan_gains(self.slot_pan_pos[index], self.pan_positions);
        }

        let previous_bus_slot_state = preserve_bus_states(
            &mut prepared.bus_slot_state,
            &prepared.bus_slot_params,
            std::mem::take(&mut self.bus_slot_state),
            &mut prepared.displaced_bus_fx_states,
        );
        let previous_master_slot_state = preserve_master_states(
            &mut prepared.master_slot_state,
            &prepared.master_slot_params,
            std::mem::take(&mut self.master_slot_state),
            &mut prepared.displaced_master_fx_states,
        );
        let previous_activity_frames = preserve_activity(
            &mut prepared.bus_activity_frames,
            std::mem::take(&mut self.bus_activity_frames),
        );
        let previous_spread_state = preserve_spread_state(
            &mut prepared.bus_output_spread_state,
            std::mem::take(&mut self.bus_output_spread_state),
        );

        retired.bus_pan_pos = std::mem::replace(&mut self.bus_pan_pos, prepared.bus_pan_pos);
        retired.bus_pan_gains_cache =
            std::mem::replace(&mut self.bus_pan_gains_cache, prepared.bus_pan_gains_cache);
        retired.bus_volume = std::mem::replace(&mut self.bus_volume, prepared.bus_volume);
        retired.bus_slot_params =
            std::mem::replace(&mut self.bus_slot_params, prepared.bus_slot_params);
        retired.bus_slot_state = previous_bus_slot_state;
        self.bus_slot_state = prepared.bus_slot_state;
        retired.bus_active_slot_indices = std::mem::replace(
            &mut self.bus_active_slot_indices,
            prepared.bus_active_slot_indices,
        );
        retired.bus_active_slot_counts = std::mem::replace(
            &mut self.bus_active_slot_counts,
            prepared.bus_active_slot_counts,
        );
        retired.bus_activity_frames = previous_activity_frames;
        self.bus_activity_frames = prepared.bus_activity_frames;
        retired.bus_output_spread_state = previous_spread_state;
        self.bus_output_spread_state = prepared.bus_output_spread_state;
        retired.bus_mono_scratch =
            std::mem::replace(&mut self.bus_mono_scratch, prepared.bus_mono_scratch);
        retired.bus_mono_snapshot =
            std::mem::replace(&mut self.bus_mono_snapshot, prepared.bus_mono_snapshot);
        self.active_bus_activity_count = self
            .bus_activity_frames
            .iter()
            .filter(|frames| **frames > 0)
            .count();
        self.refresh_routed_bus_slot_count();
        retired.master_slot_params =
            std::mem::replace(&mut self.master_slot_params, prepared.master_slot_params);
        retired.master_slot_state = previous_master_slot_state;
        self.master_slot_state = prepared.master_slot_state;
        retired.master_active_slot_indices = std::mem::replace(
            &mut self.master_active_slot_indices,
            prepared.master_active_slot_indices,
        );
        self.master_activity_frames = prepared.master_activity_frames;
        retired.prepared_slots = prepared.slots;
        retired.displaced_bus_fx_states = prepared.displaced_bus_fx_states;
        retired.displaced_master_fx_states = prepared.displaced_master_fx_states;
        retired
    }

    pub fn apply_prepared_instrument_slot(
        &mut self,
        index: usize,
        prepared: PreparedInstrumentSlot,
    ) -> RetiredAudioState {
        let retired = RetiredAudioState::default();
        if index >= INSTRUMENT_SLOT_COUNT {
            return retired;
        }
        self.slot_kind[index] = prepared.kind;
        if prepared.kind == InstrumentKind::Synth {
            self.instruments[index] = prepared.synth;
            self.synth_render_configs[index] = prepared.render_config;
            self.synth_render_revisions[index] = self.synth_render_revisions[index].wrapping_add(1);
        }
        if let Some(route) = prepared.route {
            self.slot_route[index] = route;
            self.slot_pan_pos[index] = prepared.pan_pos.min(self.pan_positions - 1);
            self.slot_volume[index] = prepared.volume;
            self.slot_pan_gains[index] =
                super::support::pan_gains(self.slot_pan_pos[index], self.pan_positions);
        }
        self.refresh_routed_bus_slot_count();
        retired
    }

    pub fn apply_prepared_sample_bank(
        &mut self,
        index: usize,
        bank: SampleBankConfig,
    ) -> RetiredAudioState {
        let mut retired = RetiredAudioState::default();
        let Some(current) = self.sample_banks.get_mut(index) else {
            retired.sample_bank = Some(bank);
            return retired;
        };
        retired.sample_bank = Some(std::mem::replace(current, bank));
        for voice in self.sample_voices[index].iter_mut() {
            voice.active = false;
        }
        retired
    }

    pub fn apply_prepared_momentary_fx_start(
        &mut self,
        prepared: PreparedMomentaryFxStart,
    ) -> RetiredAudioState {
        let mut retired = RetiredAudioState::default();
        let mut state = prepared.state;
        if let Some(pos) = self.momentary_fx.iter().position(|fx| fx.id == state.id) {
            store_retired_momentary(
                &mut retired.displaced_momentary_fx,
                self.momentary_fx.remove(pos),
            );
        }
        if self.momentary_fx.iter().any(|fx| fx.kind == state.kind)
            || self.momentary_fx.len() >= super::control::MAX_MOMENTARY_FX
        {
            store_retired_momentary(&mut retired.displaced_momentary_fx, state);
            return retired;
        }
        if state.kind == MomentaryFxKind::PitchShift {
            state
                .pitch_shifter
                .prefill_from_ring(&self.dry_history, self.dry_history_pos);
        }
        self.momentary_fx.push(state);
        retired
    }

    pub fn apply_prepared_fx_bus_slot(
        &mut self,
        bus_index: usize,
        slot_index: usize,
        mut prepared: PreparedFxBusSlot,
    ) -> RetiredAudioState {
        let mut retired = RetiredAudioState::default();
        if bus_index >= self.bus_slot_params.len() || slot_index >= BUS_SLOTS_PER_BUS {
            retired.displaced_bus_fx_states = prepared.displaced_states;
            return retired;
        }
        let old_state = std::mem::replace(
            &mut self.bus_slot_state[bus_index][slot_index],
            FxBusState::None,
        );
        if fx_bus_state_matches_params(&old_state, &prepared.params) {
            prepared.state = old_state;
        } else {
            prepared.displaced_states.push(old_state);
        }
        self.bus_slot_params[bus_index][slot_index] = prepared.params;
        self.bus_slot_state[bus_index][slot_index] = prepared.state;
        let (active_indices, active_count) = active_fx_bus_slots(&self.bus_slot_params[bus_index]);
        self.bus_active_slot_indices[bus_index] = active_indices;
        self.bus_active_slot_counts[bus_index] = active_count;
        retired.displaced_bus_fx_states = prepared.displaced_states;
        retired
    }

    pub fn apply_prepared_global_fx_slot(
        &mut self,
        slot_index: usize,
        mut prepared: PreparedGlobalFxSlot,
    ) -> RetiredAudioState {
        let mut retired = RetiredAudioState::default();
        if slot_index >= self.master_slot_params.len() {
            retired.displaced_master_fx_states = prepared.displaced_states;
            return retired;
        }
        let old_state =
            std::mem::replace(&mut self.master_slot_state[slot_index], MasterFxState::None);
        if master_fx_state_matches_params(&old_state, &prepared.params) {
            prepared.state = old_state;
        } else {
            prepared.displaced_states.push(old_state);
        }
        self.master_slot_params[slot_index] = prepared.params;
        self.master_slot_state[slot_index] = prepared.state;
        self.refresh_master_active_slot_indices();
        retired.displaced_master_fx_states = prepared.displaced_states;
        retired
    }
}

fn preserve_bus_states(
    next: &mut [[FxBusState; BUS_SLOTS_PER_BUS]],
    params: &[[FxBusParams; BUS_SLOTS_PER_BUS]],
    mut previous: Vec<[FxBusState; BUS_SLOTS_PER_BUS]>,
    displaced: &mut Vec<FxBusState>,
) -> Vec<[FxBusState; BUS_SLOTS_PER_BUS]> {
    for (bus_index, old_states) in previous.iter_mut().enumerate() {
        let Some(next_states) = next.get_mut(bus_index) else {
            continue;
        };
        for slot_index in 0..BUS_SLOTS_PER_BUS {
            if fx_bus_state_matches_params(&old_states[slot_index], &params[bus_index][slot_index])
            {
                let old_state = std::mem::replace(&mut old_states[slot_index], FxBusState::None);
                displaced.push(std::mem::replace(&mut next_states[slot_index], old_state));
            }
        }
    }
    previous
}

fn preserve_master_states(
    next: &mut [MasterFxState],
    params: &[FxBusParams],
    mut previous: Vec<MasterFxState>,
    displaced: &mut Vec<MasterFxState>,
) -> Vec<MasterFxState> {
    for (index, old_state) in previous.iter_mut().enumerate() {
        let Some(next_state) = next.get_mut(index) else {
            continue;
        };
        if master_fx_state_matches_params(old_state, &params[index]) {
            let old_state = std::mem::replace(old_state, MasterFxState::None);
            displaced.push(std::mem::replace(next_state, old_state));
        }
    }
    previous
}

fn preserve_activity(next: &mut [u32], previous: Vec<u32>) -> Vec<u32> {
    for (next, previous) in next.iter_mut().zip(previous.iter()) {
        *next = *previous;
    }
    previous
}

fn preserve_spread_state(
    next: &mut [FxBusOutputSpreadState],
    previous: Vec<FxBusOutputSpreadState>,
) -> Vec<FxBusOutputSpreadState> {
    let mut previous = previous;
    for (next, previous) in next.iter_mut().zip(previous.iter_mut()) {
        std::mem::swap(next, previous);
    }
    previous
}

fn store_retired_momentary(slots: &mut [Option<MomentaryFxState>; 2], state: MomentaryFxState) {
    let slot = slots
        .iter_mut()
        .find(|slot| slot.is_none())
        .expect("retired momentary FX capacity exceeded");
    *slot = Some(state);
}

#[cfg(test)]
#[path = "prepared_control_tests.rs"]
mod prepared_control_tests;
