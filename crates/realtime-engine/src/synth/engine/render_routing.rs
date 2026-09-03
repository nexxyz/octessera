use super::*;

const FX_BUS_SPREAD_DELAY_MS: f32 = 7.0;
const FX_BUS_SPREAD_SIDE_GAIN: f32 = 0.35;

#[derive(Clone, Debug)]
pub(super) struct FxBusOutputSpreadState {
    buf: Vec<f32>,
    idx: usize,
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
        self.routed_bus_slot_count > 0
            || self.active_bus_activity_count > 0
            || self
                .bus_chains
                .iter()
                .any(|chain| chain.assigned_worker.is_some())
    }

    pub(super) fn prepare_bus_buffers(&mut self) {
        if self.bus_mono_scratch.len() != self.bus_chains.len() {
            self.bus_mono_scratch.resize(self.bus_chains.len(), 0.0);
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
        for bus_idx in 0..self.bus_chains.len() {
            let bus_input = self.bus_mono_scratch[bus_idx];
            let bus_active =
                self.signal_present_mono(bus_input) || self.bus_chains[bus_idx].is_active();
            if !bus_active {
                self.observe_bus_chain(bus_idx, bus_input, 0.0);
                continue;
            }
            let mut bus_output = self.bus_chains[bus_idx].process(
                bus_input,
                slot_out,
                &self.bus_mono_snapshot,
                self.sample_rate,
            );
            let chain_output = bus_output.mono;
            self.observe_bus_chain(bus_idx, bus_input, chain_output);
            let input_present = self.signal_present_mono(bus_input);
            let output_present = self.signal_present_mono(chain_output);
            self.bus_chains[bus_idx].observe_render_hold(
                input_present,
                output_present,
                self.fx_activity_hold_frames,
            );
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
            let (bus_left, bus_right) = self.fx_bus_stereo_output(bus_idx, bus_output);
            left += bus_left;
            right += bus_right;
        }
        self.active_bus_activity_count = self
            .bus_chains
            .iter()
            .filter(|chain| chain.is_active())
            .count();
        (left, right)
    }

    pub(super) fn fx_bus_stereo_output(
        &mut self,
        bus_idx: usize,
        output: BusChainFrameOutput,
    ) -> (f32, f32) {
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
            let volume = self.bus_volume.get(bus_idx).copied().unwrap_or(1.0);
            (bus_left * volume, bus_right * volume)
        } else {
            let (gl, gr) = self.bus_mono_pan_gains(bus_idx);
            let volume = self.bus_volume.get(bus_idx).copied().unwrap_or(1.0);
            (output.mono * gl * volume, output.mono * gr * volume)
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

    pub(super) fn observe_bus_chain(&mut self, bus_idx: usize, input: f32, output: f32) {
        let threshold = self.dsp_config.bus_idle_threshold;
        let was_unassigned = self.bus_chains[bus_idx].assigned_worker.is_none();
        self.bus_chains[bus_idx].observe(input, output, threshold, self.sample_rate);
        if was_unassigned
            && self.bus_chains[bus_idx].assigned_worker.is_none()
            && self.bus_chains[bus_idx].cost_units() > 0
            && BusChainOwner::is_loud(input, output, threshold)
        {
            if let Some(worker) = self.choose_bus_worker(self.bus_chains[bus_idx].cost_units()) {
                self.bus_chains[bus_idx].assigned_worker = Some(worker);
            }
        }
    }

    pub(super) fn choose_bus_worker(&self, chain_cost: u16) -> Option<usize> {
        let mut active_cost_units = self.source_worker_active_cost_units();
        for chain in &self.bus_chains {
            let Some(worker) = chain.assigned_worker else {
                continue;
            };
            if worker < active_cost_units.len() {
                active_cost_units[worker] =
                    active_cost_units[worker].saturating_add(chain.cost_units());
            }
        }
        let load = self.source_worker_load.as_ref()?;
        let projected: [u64; 2] = std::array::from_fn(|worker| {
            let mut candidate = active_cost_units;
            candidate[worker] = candidate[worker].saturating_add(chain_cost);
            load.projected_ns(candidate)[worker]
        });
        Some(
            projected
                .into_iter()
                .enumerate()
                .min_by_key(|(_, value)| *value)
                .map(|(worker, _)| worker)
                .unwrap_or(0),
        )
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
