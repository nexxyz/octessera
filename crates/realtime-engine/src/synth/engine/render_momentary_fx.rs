use super::*;

impl SynthEngine {
    pub(super) fn process_momentary_fx_target(
        &mut self,
        target: MomentaryFxTarget,
        left: f32,
        right: f32,
    ) -> (f32, f32) {
        if self.momentary_fx.is_empty() {
            return (left, right);
        }
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
}
