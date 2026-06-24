use super::*;

#[derive(Clone, Copy)]
pub(in crate::synth) struct ModDelayParams {
    pub(in crate::synth) rate_hz: f32,
    pub(in crate::synth) depth_ms: f32,
    pub(in crate::synth) base_ms: f32,
    pub(in crate::synth) feedback: f32,
    pub(in crate::synth) mix: f32,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::synth) struct DelayCache {
    time_ms: f32,
    sample_rate: u32,
    delay_samples: f32,
    min_len: usize,
}

impl DelayCache {
    pub(in crate::synth) fn new(time_ms: f32, sample_rate: u32) -> Self {
        let delay_samples = (time_ms / 1000.0) * sample_rate as f32;
        let desired_len = delay_samples.ceil() as usize + 1;
        Self {
            time_ms,
            sample_rate,
            delay_samples,
            min_len: desired_len.max(2),
        }
    }

    fn refresh(&mut self, time_ms: f32, sample_rate: u32) {
        if self.time_ms.to_bits() == time_ms.to_bits() && self.sample_rate == sample_rate {
            return;
        }
        *self = Self::new(time_ms, sample_rate);
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::synth) struct ModDelayCache {
    rate_hz: f32,
    base_ms: f32,
    depth_ms: f32,
    sample_rate: u32,
    min_len: usize,
    phase_inc: f32,
}

impl ModDelayCache {
    pub(in crate::synth) fn new(params: &ModDelayParams, sample_rate: u32) -> Self {
        let need = (((params.base_ms + params.depth_ms + 5.0) / 1000.0) * sample_rate as f32).ceil()
            as usize;
        Self {
            rate_hz: params.rate_hz,
            base_ms: params.base_ms,
            depth_ms: params.depth_ms,
            sample_rate,
            min_len: need.max(2),
            phase_inc: 2.0 * PI * params.rate_hz / sample_rate as f32,
        }
    }

    fn refresh(&mut self, params: &ModDelayParams, sample_rate: u32) {
        if self.rate_hz.to_bits() == params.rate_hz.to_bits()
            && self.base_ms.to_bits() == params.base_ms.to_bits()
            && self.depth_ms.to_bits() == params.depth_ms.to_bits()
            && self.sample_rate == sample_rate
        {
            return;
        }
        *self = Self::new(params, sample_rate);
    }
}

pub(in crate::synth) fn process_delay(
    state: &mut FxBusState,
    input: f32,
    time_ms: f32,
    feedback: f32,
    mix: f32,
    sample_rate: u32,
) -> f32 {
    let fallback_cache = DelayCache::new(time_ms, sample_rate);
    let FxBusState::Delay { buf, idx, cache } = state else {
        *state = FxBusState::Delay {
            buf: vec![0.0; fallback_cache.min_len],
            idx: 0,
            cache: fallback_cache,
        };
        return input;
    };
    cache.refresh(time_ms, sample_rate);
    if buf.len() < cache.min_len {
        buf.resize(cache.min_len, 0.0);
    }
    let delayed = read_delay(buf, *idx, cache.delay_samples);
    buf[*idx] = input + delayed * feedback;
    *idx = (*idx + 1) % buf.len();
    (input * (1.0 - mix) + delayed * mix).clamp(-1.5, 1.5)
}

pub(in crate::synth) fn process_mod_delay(
    state: &mut FxBusState,
    input: f32,
    params: ModDelayParams,
    sample_rate: u32,
) -> f32 {
    let fallback_cache = ModDelayCache::new(&params, sample_rate);
    let FxBusState::ModDelay {
        buf,
        idx,
        phase,
        cache,
    } = state
    else {
        *state = FxBusState::ModDelay {
            buf: vec![0.0; ((sample_rate as f32) * 0.08) as usize],
            idx: 0,
            phase: 0.0,
            cache: fallback_cache,
        };
        return input;
    };
    cache.refresh(&params, sample_rate);
    if buf.len() < cache.min_len {
        buf.resize(cache.min_len, 0.0);
    }
    let delay_ms =
        (params.base_ms + params.depth_ms * ((*phase).sin() + 1.0) * 0.5).clamp(0.1, 100.0);
    let delayed = read_delay(buf, *idx, delay_ms * sample_rate as f32 / 1000.0);
    buf[*idx] = (input + delayed * params.feedback).clamp(-2.0, 2.0);
    *idx = (*idx + 1) % buf.len();
    *phase = wrap_phase(*phase + cache.phase_inc);
    (input * (1.0 - params.mix) + delayed * params.mix).clamp(-1.5, 1.5)
}

fn read_delay(buf: &[f32], write_idx: usize, delay_samples: f32) -> f32 {
    let len = buf.len() as f32;
    let pos = (write_idx as f32 - delay_samples).rem_euclid(len);
    let i0 = pos.floor() as usize % buf.len();
    let i1 = (i0 + 1) % buf.len();
    let frac = pos - pos.floor();
    buf[i0] * (1.0 - frac) + buf[i1] * frac
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_matches_reference_and_only_grows_buffer() {
        let mut actual = FxBusState::Delay {
            buf: vec![0.0; 2],
            idx: 0,
            cache: DelayCache::new(3.25, 44_100),
        };
        let mut expected = FxBusState::Delay {
            buf: vec![0.0; 2],
            idx: 0,
            cache: DelayCache::new(3.25, 44_100),
        };

        for frame in 0..2048 {
            let time_ms = if frame < 700 {
                3.25
            } else if frame < 1400 {
                11.75
            } else {
                4.5
            };
            let input = ((frame as f32) * 0.023).sin() * 0.5;
            let actual_out = process_delay(&mut actual, input, time_ms, 0.35, 0.6, 44_100);
            let expected_out =
                reference_process_delay(&mut expected, input, time_ms, 0.35, 0.6, 44_100);
            assert_eq!(
                actual_out.to_bits(),
                expected_out.to_bits(),
                "sample {frame}"
            );
            assert_delay_state_eq(&actual, &expected, frame);
        }
    }

    #[test]
    fn mod_delay_matches_reference_phase_and_only_grows_buffer() {
        let mut actual = FxBusState::ModDelay {
            buf: vec![0.0; 2],
            idx: 0,
            phase: 0.0,
            cache: ModDelayCache::new(
                &ModDelayParams {
                    rate_hz: 0.7,
                    depth_ms: 4.0,
                    base_ms: 8.0,
                    feedback: 0.2,
                    mix: 0.45,
                },
                44_100,
            ),
        };
        let mut expected = FxBusState::ModDelay {
            buf: vec![0.0; 2],
            idx: 0,
            phase: 0.0,
            cache: ModDelayCache::new(
                &ModDelayParams {
                    rate_hz: 0.7,
                    depth_ms: 4.0,
                    base_ms: 8.0,
                    feedback: 0.2,
                    mix: 0.45,
                },
                44_100,
            ),
        };

        for frame in 0..2048 {
            let (base_ms, depth_ms) = if frame < 700 {
                (8.0, 4.0)
            } else if frame < 1400 {
                (18.0, 12.0)
            } else {
                (6.0, 2.0)
            };
            let params = ModDelayParams {
                rate_hz: 0.7,
                depth_ms,
                base_ms,
                feedback: 0.2,
                mix: 0.45,
            };
            let input = ((frame as f32) * 0.017).sin() * 0.4 + ((frame as f32) * 0.031).cos() * 0.1;
            let actual_out = process_mod_delay(&mut actual, input, params, 44_100);
            let expected_out = reference_process_mod_delay(&mut expected, input, params, 44_100);
            assert_eq!(
                actual_out.to_bits(),
                expected_out.to_bits(),
                "sample {frame}"
            );
            assert_mod_delay_state_eq(&actual, &expected, frame);
        }
    }

    #[test]
    fn delay_caches_refresh_across_param_and_sample_rate_changes() {
        let mut delay = FxBusState::Delay {
            buf: vec![0.0; 2],
            idx: 0,
            cache: DelayCache::new(2.0, 44_100),
        };
        let mut mod_delay = FxBusState::ModDelay {
            buf: vec![0.0; 2],
            idx: 0,
            phase: 0.0,
            cache: ModDelayCache::new(
                &ModDelayParams {
                    rate_hz: 0.3,
                    depth_ms: 2.0,
                    base_ms: 4.0,
                    feedback: 0.1,
                    mix: 0.5,
                },
                44_100,
            ),
        };
        for frame in 0..128 {
            let sample_rate = if frame < 64 { 44_100 } else { 48_000 };
            let _ = process_delay(
                &mut delay,
                0.1,
                2.0 + frame as f32 * 0.01,
                0.2,
                0.5,
                sample_rate,
            );
            let params = ModDelayParams {
                rate_hz: if frame < 64 { 0.3 } else { 0.9 },
                depth_ms: 2.0 + frame as f32 * 0.01,
                base_ms: 4.0,
                feedback: 0.1,
                mix: 0.5,
            };
            let _ = process_mod_delay(&mut mod_delay, 0.1, params, sample_rate);
        }
    }

    fn reference_process_delay(
        state: &mut FxBusState,
        input: f32,
        time_ms: f32,
        feedback: f32,
        mix: f32,
        sample_rate: u32,
    ) -> f32 {
        let delay_samples = (time_ms / 1000.0) * sample_rate as f32;
        let desired_len = delay_samples.ceil() as usize + 1;
        let FxBusState::Delay { buf, idx, .. } = state else {
            *state = FxBusState::Delay {
                buf: vec![0.0; desired_len.max(2)],
                idx: 0,
                cache: DelayCache::new(time_ms, sample_rate),
            };
            return input;
        };
        if buf.len() < desired_len.max(2) {
            buf.resize(desired_len.max(2), 0.0);
        }
        let delayed = reference_read_delay(buf, *idx, delay_samples);
        buf[*idx] = input + delayed * feedback;
        *idx = (*idx + 1) % buf.len();
        (input * (1.0 - mix) + delayed * mix).clamp(-1.5, 1.5)
    }

    fn reference_process_mod_delay(
        state: &mut FxBusState,
        input: f32,
        params: ModDelayParams,
        sample_rate: u32,
    ) -> f32 {
        let FxBusState::ModDelay {
            buf, idx, phase, ..
        } = state
        else {
            *state = FxBusState::ModDelay {
                buf: vec![0.0; ((sample_rate as f32) * 0.08) as usize],
                idx: 0,
                phase: 0.0,
                cache: ModDelayCache::new(&params, sample_rate),
            };
            return input;
        };
        let need = (((params.base_ms + params.depth_ms + 5.0) / 1000.0) * sample_rate as f32).ceil()
            as usize;
        if buf.len() < need.max(2) {
            buf.resize(need.max(2), 0.0);
        }
        let delay_ms =
            (params.base_ms + params.depth_ms * ((*phase).sin() + 1.0) * 0.5).clamp(0.1, 100.0);
        let delayed = reference_read_delay(buf, *idx, delay_ms * sample_rate as f32 / 1000.0);
        buf[*idx] = (input + delayed * params.feedback).clamp(-2.0, 2.0);
        *idx = (*idx + 1) % buf.len();
        *phase = wrap_phase(*phase + 2.0 * PI * params.rate_hz / sample_rate as f32);
        (input * (1.0 - params.mix) + delayed * params.mix).clamp(-1.5, 1.5)
    }

    fn reference_read_delay(buf: &[f32], write_idx: usize, delay_samples: f32) -> f32 {
        let len = buf.len() as f32;
        let pos = (write_idx as f32 - delay_samples).rem_euclid(len);
        let i0 = pos.floor() as usize % buf.len();
        let i1 = (i0 + 1) % buf.len();
        let frac = pos - pos.floor();
        buf[i0] * (1.0 - frac) + buf[i1] * frac
    }

    fn assert_delay_state_eq(actual: &FxBusState, expected: &FxBusState, frame: usize) {
        let FxBusState::Delay {
            buf: actual_buf,
            idx: actual_idx,
            ..
        } = actual
        else {
            panic!("actual state mismatch")
        };
        let FxBusState::Delay {
            buf: expected_buf,
            idx: expected_idx,
            ..
        } = expected
        else {
            panic!("expected state mismatch")
        };
        assert_eq!(actual_idx, expected_idx, "idx {frame}");
        assert_eq!(actual_buf.len(), expected_buf.len(), "len {frame}");
        for (idx, (actual, expected)) in actual_buf.iter().zip(expected_buf).enumerate() {
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "buf {idx} sample {frame}"
            );
        }
    }

    fn assert_mod_delay_state_eq(actual: &FxBusState, expected: &FxBusState, frame: usize) {
        let FxBusState::ModDelay {
            buf: actual_buf,
            idx: actual_idx,
            phase: actual_phase,
            ..
        } = actual
        else {
            panic!("actual state mismatch")
        };
        let FxBusState::ModDelay {
            buf: expected_buf,
            idx: expected_idx,
            phase: expected_phase,
            ..
        } = expected
        else {
            panic!("expected state mismatch")
        };
        assert_eq!(actual_idx, expected_idx, "idx {frame}");
        assert_eq!(
            actual_phase.to_bits(),
            expected_phase.to_bits(),
            "phase {frame}"
        );
        assert_eq!(actual_buf.len(), expected_buf.len(), "len {frame}");
        for (idx, (actual, expected)) in actual_buf.iter().zip(expected_buf).enumerate() {
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "buf {idx} sample {frame}"
            );
        }
    }
}
