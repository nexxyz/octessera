use super::render_voice::render_synth_voice_sample;
use super::*;

impl SynthEngine {
    pub fn next_sample(&mut self) -> f32 {
        let (l, r) = self.next_stereo_sample();
        (l + r) * 0.5
    }

    pub fn next_stereo_sample(&mut self) -> (f32, f32) {
        let mut slot_out = [0.0_f32; INSTRUMENT_SLOT_COUNT];
        self.render_sample_voices(&mut slot_out);
        self.render_preview_sample_voices(&mut slot_out);
        self.render_synth_voices(&mut slot_out);
        self.prepare_bus_buffers();
        let (mut left, mut right) = self.mix_instrument_slots(&slot_out);
        (left, right) = self.mix_fx_buses(&slot_out, left, right);
        self.push_dry_history(left, right);
        let master_signal = self.signal_present(left, right)
            || self.active_synth_voice_total() > 0
            || self.active_sample_voice_total() > 0
            || !self.preview_sample_voices.is_empty()
            || !self.momentary_fx.is_empty()
            || self.bus_activity_frames.iter().any(|frames| *frames > 0);
        let master_active = master_signal || self.master_activity_frames > 0;
        if master_active {
            (left, right) = self.apply_master_fx_slots(left, right);
            (left, right) =
                self.process_momentary_fx_target(MomentaryFxTarget::Global, left, right);
            self.master_activity_frames = if master_signal || self.signal_present(left, right) {
                self.fx_activity_hold_frames
            } else {
                self.master_activity_frames.saturating_sub(1)
            };
        }
        self.sample_clock = self.sample_clock.saturating_add(1);
        (
            (left * self.master_volume).clamp(-1.0, 1.0),
            (right * self.master_volume).clamp(-1.0, 1.0),
        )
    }

    fn render_synth_voices(&mut self, slot_out: &mut [f32; INSTRUMENT_SLOT_COUNT]) {
        for pool in self.voices.iter_mut() {
            for v in pool.iter_mut() {
                if !v.active {
                    continue;
                }
                let slot = (v.instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
                let cfg = self.instruments[slot];
                if self.sample_clock >= v.note_off_sample {
                    v.amp_env.begin_release(cfg.amp_env, self.sample_rate);
                    v.filt_env.begin_release(cfg.filter_env, self.sample_rate);
                }
                let amp_env = v.amp_env.next();
                let filt_env = v.filt_env.next();
                if v.amp_env.is_off() {
                    v.active = false;
                    continue;
                }
                let sample = render_synth_voice_sample(
                    self.sample_rate,
                    self.mods[slot],
                    cfg,
                    v,
                    amp_env,
                    filt_env,
                );
                slot_out[slot] += sample;
            }
        }
    }
    fn prepare_bus_buffers(&mut self) {
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

    fn mix_instrument_slots(&mut self, slot_out: &[f32; INSTRUMENT_SLOT_COUNT]) -> (f32, f32) {
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
                let (gl, gr) = pan_gains(self.slot_pan_pos[slot], self.pan_positions);
                left += sample * gl;
                right += sample * gr;
            } else {
                let bus = route - 1;
                if bus < self.bus_mono_scratch.len() {
                    self.bus_mono_scratch[bus] += sample;
                } else {
                    let (gl, gr) = pan_gains(self.slot_pan_pos[slot], self.pan_positions);
                    left += sample * gl;
                    right += sample * gr;
                }
            }
        }
        (left, right)
    }

    fn mix_fx_buses(
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
                if let Some(counter) = self.bus_activity_frames.get_mut(bus_idx) {
                    *counter = self.fx_activity_hold_frames;
                }
            } else if let Some(counter) = self.bus_activity_frames.get_mut(bus_idx) {
                *counter = counter.saturating_sub(1);
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
            let pan = self.bus_pan_pos.get(bus_idx).copied().unwrap_or(0);
            pan_gains(pan, self.pan_positions)
        }
    }

    fn push_dry_history(&mut self, left: f32, right: f32) {
        self.dry_history[self.dry_history_pos] = left;
        self.dry_history[self.dry_history_pos + 1] = right;
        self.dry_history_pos += 2;
        if self.dry_history_pos >= self.dry_history.len() {
            self.dry_history_pos = 0;
        }
    }

    fn apply_master_fx_slots(&mut self, mut left: f32, mut right: f32) -> (f32, f32) {
        for slot_idx in 0..self.master_slot_params.len() {
            let params = self.master_slot_params[slot_idx];
            if let Some(state) = self.master_slot_state.get_mut(slot_idx) {
                (left, right) =
                    process_master_fx_slot(&params, state, left, right, self.sample_rate);
            }
        }
        (left, right)
    }

    fn process_momentary_fx_target(
        &mut self,
        target: MomentaryFxTarget,
        left: f32,
        right: f32,
    ) -> (f32, f32) {
        let sample_rate = self.sample_rate;
        let mut l = left;
        let mut r = right;
        for fx in self.momentary_fx.iter_mut() {
            if fx.target != target {
                continue;
            }
            match fx.kind {
                MomentaryFxKind::Stutter => {
                    let depth = (param_f32(&fx.params, "depthPct", 100.0) / 100.0).clamp(0.0, 1.0);
                    let segment_len = fx.stutter_segment_len.min(fx.stutter_l.len()).max(1);
                    let ramp_len = fx.stutter_ramp_len.min(segment_len / 4).max(1);

                    if !fx.stutter_ready {
                        fx.stutter_l[fx.stutter_write] = l;
                        fx.stutter_r[fx.stutter_write] = r;
                        fx.stutter_write += 1;
                        if fx.stutter_write >= segment_len {
                            fx.stutter_ready = true;
                            fx.stutter_write = 0;
                            fx.stutter_ramp_pos = 0;
                        }
                    } else {
                        let read = fx.stutter_write;
                        let mut wet_l = fx.stutter_l[read];
                        let mut wet_r = fx.stutter_r[read];

                        let eff_wet = if fx.stutter_ramp_pos < ramp_len {
                            let ramp = fx.stutter_ramp_pos as f32 / ramp_len as f32;
                            fx.stutter_ramp_pos += 1;
                            depth * ramp
                        } else {
                            depth
                        };

                        if read < ramp_len {
                            let fade_in = read as f32 / ramp_len as f32;
                            let end_read = segment_len - ramp_len + read;
                            wet_l = wet_l * fade_in + fx.stutter_l[end_read] * (1.0 - fade_in);
                            wet_r = wet_r * fade_in + fx.stutter_r[end_read] * (1.0 - fade_in);
                        }

                        l = l * (1.0 - eff_wet) + wet_l * eff_wet;
                        r = r * (1.0 - eff_wet) + wet_r * eff_wet;

                        fx.stutter_write += 1;
                        if fx.stutter_write >= segment_len {
                            fx.stutter_write = 0;
                        }
                    }
                }
                MomentaryFxKind::Freeze => {
                    let mix = (param_f32(&fx.params, "mixPct", 100.0) / 100.0).clamp(0.0, 1.0);
                    let feedback = 0.997_f32;
                    let damp = 0.35_f32;

                    if fx.releasing {
                        let total = fx.release_len.max(1) as f32;
                        let fade = 1.0 - (fx.release_pos as f32 / total);
                        fx.release_pos += 1;

                        let mut wet_l = 0.0_f32;
                        let mut wet_r = 0.0_f32;
                        for i in 0..4 {
                            let delayed = fx.freeze_bufs[i][fx.freeze_idxs[i]];
                            fx.freeze_lp[i] = delayed * (1.0 - damp) + fx.freeze_lp[i] * damp;
                            fx.freeze_bufs[i][fx.freeze_idxs[i]] = fx.freeze_lp[i] * feedback;
                            fx.freeze_idxs[i] = (fx.freeze_idxs[i] + 1) % fx.freeze_bufs[i].len();
                            if i < 2 {
                                wet_l += delayed;
                            } else {
                                wet_r += delayed;
                            }
                        }
                        wet_l *= 0.5;
                        wet_r *= 0.5;
                        l = l * (1.0 - mix * fade) + wet_l * mix;
                        r = r * (1.0 - mix * fade) + wet_r * mix;
                    } else {
                        let injecting = fx.freeze_inject_pos < fx.freeze_inject_len;
                        let inject_gain = if injecting { 1.0 } else { 0.0 };
                        if injecting {
                            fx.freeze_inject_pos += 1;
                        }

                        let mut wet_l = 0.0_f32;
                        let mut wet_r = 0.0_f32;
                        for i in 0..4 {
                            let delayed = fx.freeze_bufs[i][fx.freeze_idxs[i]];
                            fx.freeze_lp[i] = delayed * (1.0 - damp) + fx.freeze_lp[i] * damp;
                            let channel_in = if i < 2 { l } else { r };
                            fx.freeze_bufs[i][fx.freeze_idxs[i]] =
                                channel_in * inject_gain + fx.freeze_lp[i] * feedback;
                            fx.freeze_idxs[i] = (fx.freeze_idxs[i] + 1) % fx.freeze_bufs[i].len();
                            if i < 2 {
                                wet_l += delayed;
                            } else {
                                wet_r += delayed;
                            }
                        }
                        wet_l *= 0.5;
                        wet_r *= 0.5;
                        l = l * (1.0 - mix) + wet_l * mix;
                        r = r * (1.0 - mix) + wet_r * mix;
                    }
                }
                MomentaryFxKind::FilterSweep => {
                    let cutoff_pct =
                        (param_f32(&fx.params, "cutoffPct", 35.0) / 100.0).clamp(0.0, 1.0);
                    let resonance_pct =
                        (param_f32(&fx.params, "resonancePct", 70.0) / 100.0).clamp(0.0, 1.0);
                    let q = 0.5 + resonance_pct * 11.5;
                    let target_cutoff = 120.0 + cutoff_pct * 8_000.0;

                    if fx.releasing {
                        let out_len =
                            ms_to_samples(param_f32(&fx.params, "sweepOutMs", 500.0), sample_rate)
                                .max(1) as f32;
                        fx.sweep_pos -= 1.0 / out_len;
                        if fx.sweep_pos < 0.0 {
                            fx.sweep_pos = 0.0;
                        }
                    } else {
                        let in_len =
                            ms_to_samples(param_f32(&fx.params, "sweepInMs", 200.0), sample_rate)
                                .max(1) as f32;
                        fx.sweep_pos += 1.0 / in_len;
                        if fx.sweep_pos > 1.0 {
                            fx.sweep_pos = 1.0;
                        }
                    }

                    let cutoff = 20_000.0 + (target_cutoff - 20_000.0) * fx.sweep_pos;
                    l = fx
                        .filt_l
                        .process(l, FilterType::Lowpass, cutoff, q, sample_rate);
                    r = fx
                        .filt_r
                        .process(r, FilterType::Lowpass, cutoff, q, sample_rate);
                }
                MomentaryFxKind::PitchShift => {
                    let semitones = param_f32(&fx.params, "semitones", 7.0).clamp(-24.0, 24.0);
                    let cents = param_f32(&fx.params, "cents", 0.0).clamp(-100.0, 100.0);
                    let mix = (param_f32(&fx.params, "mixPct", 100.0) / 100.0).clamp(0.0, 1.0);
                    let total_semitones = semitones + cents / 100.0;
                    let ratio = 2.0_f32.powf(total_semitones / 12.0);

                    let (wet_l, wet_r) = fx.pitch_shifter.process_frame(l, r, ratio);
                    let ramp = if fx.pitch_ramp_pos < fx.pitch_ramp_len {
                        let r = fx.pitch_ramp_pos as f32 / fx.pitch_ramp_len as f32;
                        fx.pitch_ramp_pos += 1;
                        r
                    } else {
                        1.0
                    };
                    let wet_mix = mix * ramp;
                    l = l * (1.0 - wet_mix) + wet_l * wet_mix;
                    r = r * (1.0 - wet_mix) + wet_r * wet_mix;
                }
            }
        }

        self.momentary_fx.retain(|fx| {
            if !fx.releasing {
                return true;
            }
            match fx.kind {
                MomentaryFxKind::FilterSweep => fx.sweep_pos > 0.0,
                MomentaryFxKind::Freeze => {
                    let total =
                        ms_to_samples(param_f32(&fx.params, "releaseMs", 500.0), sample_rate);
                    fx.release_pos < total
                }
                _ => false,
            }
        });

        (l, r)
    }

    pub(super) fn sample_note_on(&mut self, slot: usize, midi_note: u8, velocity: u8) {
        let sample_slot = sample_slot_for_note(midi_note);
        let Some(bank) = self.sample_banks.get(slot) else {
            return;
        };
        let Some(Some(buffer)) = bank.slots.get(sample_slot).map(|s| s.buffer.as_ref()) else {
            return;
        };
        if buffer.samples.is_empty() || buffer.channels == 0 || buffer.sample_rate == 0 {
            return;
        }
        let vel = (velocity.max(1) as f32 / 127.0).clamp(0.0, 1.0);
        let vel_sens = (bank.velocity_sensitivity_pct / 100.0).clamp(0.0, 1.0);
        let gain = (bank.gain_pct / 100.0).clamp(0.0, 2.0) * ((1.0 - vel_sens) + vel_sens * vel);
        let pitch = 2.0_f32.powf(bank.tune_semis / 12.0);
        let step = pitch * buffer.sample_rate as f32 / self.sample_rate as f32;
        let (voice_index, stole_voice) = {
            let pool = &mut self.sample_voices[slot];
            let active = pool.iter().filter(|voice| voice.active).count();
            if active >= MAX_SAMPLE_VOICES_PER_SLOT {
                (Self::steal_active_sample_voice_index(pool), true)
            } else {
                match pool.iter().position(|voice| !voice.active) {
                    Some(i) => (i, false),
                    None => (Self::steal_active_sample_voice_index(pool), true),
                }
            }
        };
        if stole_voice {
            self.record_voice_steal();
        }
        let pool = &mut self.sample_voices[slot];
        pool[voice_index] = SampleVoice {
            active: true,
            sample_slot,
            pos: 0.0,
            step,
            gain,
        };

        self.enforce_voice_budgets();
    }

    fn render_sample_voices(&mut self, slot_out: &mut [f32; INSTRUMENT_SLOT_COUNT]) {
        for (slot, out) in slot_out.iter_mut().enumerate().take(INSTRUMENT_SLOT_COUNT) {
            let Some(bank) = self.sample_banks.get(slot) else {
                continue;
            };
            for voice in self.sample_voices[slot].iter_mut() {
                if !voice.active {
                    continue;
                }
                let Some(Some(buffer)) =
                    bank.slots.get(voice.sample_slot).map(|s| s.buffer.as_ref())
                else {
                    voice.active = false;
                    continue;
                };
                let frames = buffer.samples.len() / buffer.channels as usize;
                if frames == 0 || voice.pos >= frames as f32 {
                    voice.active = false;
                    continue;
                }
                let frame = voice.pos.floor() as usize;
                let frac = voice.pos - frame as f32;
                let next_frame = (frame + 1).min(frames - 1);
                let sample = mono_frame(buffer, frame) * (1.0 - frac)
                    + mono_frame(buffer, next_frame) * frac;
                *out += sample * voice.gain;
                voice.pos += voice.step;
            }
        }
    }

    fn render_preview_sample_voices(&mut self, slot_out: &mut [f32; INSTRUMENT_SLOT_COUNT]) {
        for voice in self.preview_sample_voices.iter_mut() {
            let frames = voice.buffer.samples.len() / voice.buffer.channels as usize;
            if frames == 0 || voice.pos >= frames as f32 {
                voice.pos = frames as f32;
                continue;
            }
            let frame = voice.pos.floor() as usize;
            let frac = voice.pos - frame as f32;
            let next_frame = (frame + 1).min(frames - 1);
            let sample = mono_frame(&voice.buffer, frame) * (1.0 - frac)
                + mono_frame(&voice.buffer, next_frame) * frac;
            slot_out[voice.slot] += sample * voice.gain;
            voice.pos += voice.step;
        }
        self.preview_sample_voices.retain(|voice| {
            let frames = voice.buffer.samples.len() / voice.buffer.channels as usize;
            frames > 0 && voice.pos < frames as f32
        });
    }

    fn signal_present_mono(&self, sample: f32) -> bool {
        sample.abs() > 1.0e-5
    }

    fn signal_present(&self, left: f32, right: f32) -> bool {
        self.signal_present_mono(left) || self.signal_present_mono(right)
    }
}
