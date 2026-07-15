use super::*;

const FX_BUS_SPREAD_DELAY_MS: f32 = 7.0;
const FX_BUS_SPREAD_SIDE_GAIN: f32 = 0.35;

#[derive(Clone, Debug)]
pub(super) struct FxBusOutputSpreadState {
    buf: Vec<f32>,
    idx: usize,
}

#[derive(Clone, Copy, Debug)]
struct FxBusOutput {
    mono: f32,
    auto_pan_pos: Option<f32>,
    spread: f32,
}

impl FxBusOutputSpreadState {
    pub(super) fn new(sample_rate: u32) -> Self {
        let len = ((FX_BUS_SPREAD_DELAY_MS / 1000.0) * sample_rate as f32)
            .round()
            .max(1.0) as usize;
        Self {
            buf: vec![0.0; len],
            idx: 0,
        }
    }

    fn process(&mut self, mono: f32, spread: f32) -> (f32, f32) {
        if spread <= 0.0 {
            let (center_l, center_r) = pan_gains_float(0.5);
            return (mono * center_l, mono * center_r);
        }
        let delayed = self.buf[self.idx];
        self.buf[self.idx] = mono;
        self.idx = (self.idx + 1) % self.buf.len();
        let side = ((mono - delayed) * spread * FX_BUS_SPREAD_SIDE_GAIN).clamp(-0.5, 0.5);
        let (center_l, center_r) = pan_gains_float(0.5);
        (
            (mono * center_l + side).clamp(-1.5, 1.5),
            (mono * center_r - side).clamp(-1.5, 1.5),
        )
    }
}

impl SynthEngine {
    pub(super) fn should_process_fx_buses(&self) -> bool {
        self.routed_bus_slot_count > 0 || self.active_bus_activity_count > 0
    }

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
        let process_momentary = !self.momentary_fx.is_empty();
        if self.routed_bus_slot_count == 0 {
            for (slot, sample) in slot_out.iter().enumerate() {
                let mut sample = *sample * self.slot_volume[slot];
                if process_momentary {
                    let (fx_l, fx_r) = self.process_momentary_fx_target(
                        MomentaryFxTarget::Instrument { index: slot },
                        sample,
                        sample,
                    );
                    sample = (fx_l + fx_r) * 0.5;
                }
                let (gl, gr) = self.slot_pan_gains[slot];
                left += sample * gl;
                right += sample * gr;
            }
            return (left, right);
        }
        for (slot, sample) in slot_out.iter().enumerate() {
            let mut sample = *sample * self.slot_volume[slot];
            if process_momentary {
                let (fx_l, fx_r) = self.process_momentary_fx_target(
                    MomentaryFxTarget::Instrument { index: slot },
                    sample,
                    sample,
                );
                sample = (fx_l + fx_r) * 0.5;
            }
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
            let mut bus_output = self.process_fx_bus(bus_idx, slot_out);
            bus_output.mono = if self.momentary_fx.is_empty() {
                bus_output.mono
            } else {
                let (fx_l, fx_r) = self.process_momentary_fx_target(
                    MomentaryFxTarget::FxBus { index: bus_idx },
                    bus_output.mono,
                    bus_output.mono,
                );
                (fx_l + fx_r) * 0.5
            };
            if self.signal_present_mono(bus_output.mono) || self.signal_present_mono(bus_input) {
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
            let (bus_left, bus_right) = self.fx_bus_stereo_output(bus_idx, bus_output);
            left += bus_left;
            right += bus_right;
        }
        (left, right)
    }

    fn process_fx_bus(
        &mut self,
        bus_idx: usize,
        slot_out: &[f32; INSTRUMENT_SLOT_COUNT],
    ) -> FxBusOutput {
        let mut processed = self.bus_mono_scratch[bus_idx];
        let mut auto_pan_pos = None;
        let mut spread = 0.0_f32;
        if let (Some(params), Some(states)) = (
            self.bus_slot_params.get(bus_idx),
            self.bus_slot_state.get_mut(bus_idx),
        ) {
            let active_indices = self.bus_active_slot_indices[bus_idx];
            let active_count = self.bus_active_slot_counts[bus_idx];
            for j in active_indices.iter().take(active_count).copied() {
                processed = process_fx_bus_slot(
                    &params[j],
                    &mut states[j],
                    processed,
                    slot_out,
                    &self.bus_mono_snapshot,
                    self.sample_rate,
                );
                match (&params[j], &states[j]) {
                    (
                        FxBusParams::Delay {
                            mix,
                            spread: slot_spread,
                            ..
                        },
                        _,
                    ) => {
                        spread = spread.max(slot_spread * mix);
                    }
                    (_, FxBusState::AutoPan { pos, .. }) => {
                        auto_pan_pos = Some(pos.clamp(0.0, 1.0));
                    }
                    _ => {}
                }
            }
        }
        FxBusOutput {
            mono: processed,
            auto_pan_pos,
            spread,
        }
    }

    fn fx_bus_stereo_output(&mut self, bus_idx: usize, output: FxBusOutput) -> (f32, f32) {
        let stereo_output = if output.spread > 0.0 {
            Some(self.bus_output_spread_state[bus_idx].process(output.mono, output.spread))
        } else if let Some(pos) = output.auto_pan_pos {
            let (gl, gr) = pan_gains_float(pos);
            Some((output.mono * gl, output.mono * gr))
        } else {
            None
        };
        if let Some((mut bus_left, mut bus_right)) = stereo_output {
            if output.spread > 0.0 {
                if let Some(pos) = output.auto_pan_pos {
                    let (gl, gr) = stereo_balance_gains(pos);
                    bus_left *= gl;
                    bus_right *= gr;
                }
            }
            let (gl, gr) = self.bus_stereo_balance_gains(bus_idx);
            bus_left *= gl;
            bus_right *= gr;
            (bus_left, bus_right)
        } else {
            let (gl, gr) = self.bus_mono_pan_gains(bus_idx);
            (output.mono * gl, output.mono * gr)
        }
    }

    fn bus_mono_pan_gains(&self, bus_idx: usize) -> (f32, f32) {
        self.bus_pan_gains_cache
            .get(bus_idx)
            .copied()
            .unwrap_or_else(|| pan_gains(0, self.pan_positions))
    }

    fn bus_stereo_balance_gains(&self, bus_idx: usize) -> (f32, f32) {
        let Some(pan_pos) = self.bus_pan_pos.get(bus_idx).copied() else {
            return (1.0, 1.0);
        };
        if self.pan_positions <= 1 {
            return (1.0, 1.0);
        }
        let pos = (pan_pos.min(self.pan_positions - 1) as f32) / ((self.pan_positions - 1) as f32);
        stereo_balance_gains(pos)
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
        for slot_idx in self.master_active_slot_indices.iter().copied() {
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

fn stereo_balance_gains(pos: f32) -> (f32, f32) {
    let pos = pos.clamp(0.0, 1.0);
    if pos <= 0.5 {
        let right = (pos * 2.0 * std::f32::consts::FRAC_PI_2).sin();
        (1.0, right)
    } else {
        let left = ((1.0 - pos) * 2.0 * std::f32::consts::FRAC_PI_2).sin();
        (left, 1.0)
    }
}
