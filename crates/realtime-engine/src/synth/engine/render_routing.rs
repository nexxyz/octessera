use super::*;

impl SynthEngine {
    pub(super) fn prepare_bus_buffers(&mut self) {
        if self.bus_mono_scratch.len() != self.bus_pan_pos.len() {
            self.bus_mono_scratch.resize(self.bus_pan_pos.len(), 0.0);
        } else {
            self.bus_mono_scratch.fill(0.0);
        }
        if self.bus_mono_snapshot.len() != self.bus_mono_scratch.len() {
            self.bus_mono_snapshot
                .resize(self.bus_mono_scratch.len(), 0.0);
        }
    }

    pub(super) fn mix_instrument_slots(
        &mut self,
        slot_out: &[f32; INSTRUMENT_SLOT_COUNT],
    ) -> (f32, f32) {
        let mut left = 0.0_f32;
        let mut right = 0.0_f32;
        for (slot, sample) in slot_out.iter().enumerate() {
            let mut sample = *sample * self.slot_volume[slot];
            let (fx_l, fx_r) = self.process_momentary_fx_target(
                MomentaryFxTarget::Instrument { index: slot },
                sample,
                sample,
            );
            sample = (fx_l + fx_r) * 0.5;
            let route = self.slot_route[slot];
            if route == 0 {
                let (gl, gr) = self.slot_pan_gains[slot];
                left += sample * gl;
                right += sample * gr;
            } else {
                let bus = route - 1;
                if bus < self.bus_mono_scratch.len() {
                    self.bus_mono_scratch[bus] += sample;
                } else {
                    let (gl, gr) = self.slot_pan_gains[slot];
                    left += sample * gl;
                    right += sample * gr;
                }
            }
        }
        (left, right)
    }

    pub(super) fn mix_fx_buses(
        &mut self,
        slot_out: &[f32; INSTRUMENT_SLOT_COUNT],
        mut left: f32,
        mut right: f32,
    ) -> (f32, f32) {
        self.bus_mono_snapshot
            .copy_from_slice(&self.bus_mono_scratch);
        for bus_idx in 0..self.bus_mono_scratch.len() {
            let bus_input = self.bus_mono_scratch[bus_idx];
            let bus_active = self.signal_present_mono(bus_input)
                || self.bus_activity_frames.get(bus_idx).copied().unwrap_or(0) > 0;
            if !bus_active {
                continue;
            }
            let (processed, pan_override) = self.process_fx_bus(bus_idx, slot_out);
            let (fx_l, fx_r) = self.process_momentary_fx_target(
                MomentaryFxTarget::FxBus { index: bus_idx },
                processed,
                processed,
            );
            let processed = (fx_l + fx_r) * 0.5;
            if self.signal_present_mono(processed) || self.signal_present_mono(bus_input) {
                self.set_bus_activity_hold(bus_idx);
            } else {
                let became_inactive =
                    if let Some(counter) = self.bus_activity_frames.get_mut(bus_idx) {
                        let was_active = *counter > 0;
                        *counter = counter.saturating_sub(1);
                        was_active && *counter == 0
                    } else {
                        false
                    };
                if became_inactive {
                    self.active_bus_activity_count =
                        self.active_bus_activity_count.saturating_sub(1);
                }
            }
            let (gl, gr) = self.bus_pan_gains(bus_idx, pan_override);
            left += processed * gl;
            right += processed * gr;
        }
        (left, right)
    }

    fn process_fx_bus(
        &mut self,
        bus_idx: usize,
        slot_out: &[f32; INSTRUMENT_SLOT_COUNT],
    ) -> (f32, Option<f32>) {
        let mut processed = self.bus_mono_scratch[bus_idx];
        let mut pan_override = None;
        if let (Some(params), Some(states)) = (
            self.bus_slot_params.get(bus_idx),
            self.bus_slot_state.get_mut(bus_idx),
        ) {
            for j in 0..BUS_SLOTS_PER_BUS {
                processed = process_fx_bus_slot(
                    &params[j],
                    &mut states[j],
                    processed,
                    slot_out,
                    &self.bus_mono_snapshot,
                    self.sample_rate,
                );
                if let FxBusState::AutoPan { pos, .. } = states[j] {
                    pan_override = Some(pos.clamp(0.0, 1.0));
                }
            }
        }
        (processed, pan_override)
    }

    fn bus_pan_gains(&self, bus_idx: usize, pan_override: Option<f32>) -> (f32, f32) {
        if let Some(pos) = pan_override {
            pan_gains_float(pos)
        } else {
            self.bus_pan_gains_cache
                .get(bus_idx)
                .copied()
                .unwrap_or_else(|| pan_gains(0, self.pan_positions))
        }
    }

    fn set_bus_activity_hold(&mut self, bus_idx: usize) {
        if let Some(counter) = self.bus_activity_frames.get_mut(bus_idx) {
            if *counter == 0 {
                self.active_bus_activity_count += 1;
            }
            *counter = self.fx_activity_hold_frames;
        }
    }

    pub(super) fn push_dry_history(&mut self, left: f32, right: f32) {
        self.dry_history[self.dry_history_pos] = left;
        self.dry_history[self.dry_history_pos + 1] = right;
        self.dry_history_pos += 2;
        if self.dry_history_pos >= self.dry_history.len() {
            self.dry_history_pos = 0;
        }
    }

    pub(super) fn apply_master_fx_slots(&mut self, mut left: f32, mut right: f32) -> (f32, f32) {
        for slot_idx in 0..self.master_slot_params.len() {
            let params = self.master_slot_params[slot_idx];
            if let Some(state) = self.master_slot_state.get_mut(slot_idx) {
                (left, right) =
                    process_master_fx_slot(&params, state, left, right, self.sample_rate);
            }
        }
        (left, right)
    }

    pub(super) fn signal_present_mono(&self, sample: f32) -> bool {
        sample.abs() > 1.0e-5
    }

    pub(super) fn signal_present(&self, left: f32, right: f32) -> bool {
        self.signal_present_mono(left) || self.signal_present_mono(right)
    }
}
