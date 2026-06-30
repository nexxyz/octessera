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
#[path = "delay_tests.rs"]
mod delay_tests;
