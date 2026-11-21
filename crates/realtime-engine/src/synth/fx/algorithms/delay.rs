use super::*;

#[derive(Clone, Copy)]
pub(in crate::synth) struct ModDelayParams {
    pub(in crate::synth) rate_hz: f32,
    pub(in crate::synth) depth_ms: f32,
    pub(in crate::synth) base_ms: f32,
    pub(in crate::synth) feedback: f32,
    pub(in crate::synth) mix: f32,
}

/// The longest Delay Time: a whole note at the slowest tempo. Every bus delay line holds this
/// much, so a Time change reads across the tail instead of rebuilding the line.
pub(in crate::synth) const DELAY_MAX_MS: f32 = 6000.0;
/// A new Delay Time is reached on a one-pole glide with this time constant, like tape.
const DELAY_TIME_GLIDE_MS: f32 = 50.0;

#[derive(Clone, Copy, Debug)]
pub(in crate::synth) struct DelayCache {
    time_ms: f32,
    sample_rate: u32,
    delay_samples: f32,
    current_samples: f32,
    glide: f32,
    pub(in crate::synth) min_len: usize,
}

impl DelayCache {
    pub(in crate::synth) fn new(time_ms: f32, sample_rate: u32) -> Self {
        let delay_samples = (time_ms / 1000.0) * sample_rate as f32;
        let desired_len = delay_samples.ceil() as usize + 1;
        Self {
            time_ms,
            sample_rate,
            delay_samples,
            current_samples: delay_samples,
            glide: 1.0 / (DELAY_TIME_GLIDE_MS * sample_rate as f32 / 1000.0),
            min_len: desired_len.max(2),
        }
    }

    fn refresh(&mut self, time_ms: f32, sample_rate: u32) {
        if self.time_ms.to_bits() == time_ms.to_bits() && self.sample_rate == sample_rate {
            return;
        }
        let current = self.current_samples;
        let same_rate = self.sample_rate == sample_rate;
        *self = Self::new(time_ms, sample_rate);
        if same_rate {
            self.current_samples = current;
        }
    }

    fn glide(&mut self, len: usize) -> f32 {
        let target = self.delay_samples.min((len - 1) as f32);
        let diff = target - self.current_samples;
        self.current_samples = if diff.abs() < 0.05 {
            target
        } else {
            self.current_samples + diff * self.glide
        };
        self.current_samples
    }
}

/// A delay line holding up to `longest_ms`, set to `time_ms`.
pub(in crate::synth) fn delay_line_state(
    longest_ms: f32,
    time_ms: f32,
    sample_rate: u32,
) -> FxBusState {
    let longest = DelayCache::new(longest_ms.max(time_ms), sample_rate);
    FxBusState::Delay {
        buf: vec![0.0; longest.min_len],
        idx: 0,
        cache: DelayCache::new(time_ms, sample_rate),
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::synth) struct ModDelayCache {
    rate_hz: f32,
    base_ms: f32,
    depth_ms: f32,
    sample_rate: u32,
    pub(in crate::synth) min_len: usize,
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
    let FxBusState::Delay { buf, idx, cache } = state else {
        return input;
    };
    cache.refresh(time_ms, sample_rate);
    let delay_samples = cache.glide(buf.len());
    let delayed = read_delay(buf, *idx, delay_samples);
    buf[*idx] = (input + delayed * feedback).clamp(-2.0, 2.0);
    *idx = (*idx + 1) % buf.len();
    (input * (1.0 - mix) + delayed * mix).clamp(-1.5, 1.5)
}

pub(in crate::synth) fn process_mod_delay(
    state: &mut FxBusState,
    input: f32,
    params: ModDelayParams,
    sample_rate: u32,
) -> f32 {
    let FxBusState::ModDelay {
        buf,
        idx,
        phase,
        cache,
    } = state
    else {
        return input;
    };
    cache.refresh(&params, sample_rate);
    let delay_ms = (params.base_ms
        + params.depth_ms * (crate::sine::sin_radians(*phase) + 1.0) * 0.5)
        .clamp(0.1, 100.0);
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
mod delay_tests;
#[cfg(test)]
mod time_change_tests;
