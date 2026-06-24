use super::*;
use std::f32::consts::PI;

pub(super) const FREEZE_INJECT_MS: u32 = 120;
pub(super) const DRY_HISTORY_FRAMES: usize = 2048;
pub(super) const PITCH_BUF_FRAMES: usize = 2048;
const PITCH_MIN_DELAY: f32 = 64.0;
const PITCH_RANGE: f32 = 1024.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InstrumentKind {
    Synth,
    Sample,
    Midi,
    None,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SampleVoice {
    pub(super) active: bool,
    pub(super) sample_slot: usize,
    pub(super) pos: f32,
    pub(super) step: f32,
    pub(super) gain: f32,
}

impl SampleVoice {
    pub(super) const fn off() -> Self {
        Self {
            active: false,
            sample_slot: 0,
            pos: 0.0,
            step: 1.0,
            gain: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct PreviewSampleVoice {
    pub(super) slot: usize,
    pub(super) buffer: SampleBuffer,
    pub(super) pos: f32,
    pub(super) step: f32,
    pub(super) gain: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MomentaryFxKind {
    Stutter,
    Freeze,
    FilterSweep,
    PitchShift,
}

pub(super) struct MomentaryFxState {
    pub(super) id: String,
    pub(super) kind: MomentaryFxKind,
    pub(super) params: BTreeMap<String, Value>,
    pub(super) target: MomentaryFxTarget,
    pub(super) releasing: bool,
    pub(super) release_pos: u32,
    pub(super) release_len: u32,
    pub(super) sweep_pos: f32,
    pub(super) filt_l: BiquadState,
    pub(super) filt_r: BiquadState,
    pub(super) pitch_shifter: LivePitchShift,
    pub(super) pitch_ramp_pos: u32,
    pub(super) pitch_ramp_len: u32,
    pub(super) stutter_l: Vec<f32>,
    pub(super) stutter_r: Vec<f32>,
    pub(super) stutter_write: usize,
    pub(super) stutter_ready: bool,
    pub(super) stutter_segment_len: usize,
    pub(super) stutter_ramp_len: usize,
    pub(super) stutter_ramp_pos: usize,
    pub(super) freeze_bufs: [Vec<f32>; 4],
    pub(super) freeze_idxs: [usize; 4],
    pub(super) freeze_lp: [f32; 4],
    pub(super) freeze_inject_pos: u32,
    pub(super) freeze_inject_len: u32,
}

impl MomentaryFxState {
    pub(super) fn new(
        id: String,
        kind: MomentaryFxKind,
        params: BTreeMap<String, Value>,
        target: MomentaryFxTarget,
        sample_rate: u32,
    ) -> Self {
        let ramp_samples = ((sample_rate as f32 * 0.002) as usize).max(1);
        let pitch_ramp_len = ((sample_rate as f32 * 0.002) as u32).max(1);
        let stutter_segment_len = stutter_segment_len(sample_rate, &params);
        const DELAY_LENS: [usize; 4] = [1557, 1617, 1491, 1422];
        let freeze_bufs: [Vec<f32>; 4] =
            DELAY_LENS.map(|n| vec![0.0; (n * sample_rate as usize / 44_100).max(1)]);
        let freeze_inject_len = (sample_rate * FREEZE_INJECT_MS / 1000).max(1);
        Self {
            id,
            kind,
            params,
            target,
            releasing: false,
            release_pos: 0,
            release_len: 0,
            sweep_pos: 0.0,
            filt_l: BiquadState::new(),
            filt_r: BiquadState::new(),
            pitch_shifter: LivePitchShift::new(sample_rate),
            pitch_ramp_pos: 0,
            pitch_ramp_len,
            stutter_l: vec![0.0; sample_rate as usize],
            stutter_r: vec![0.0; sample_rate as usize],
            stutter_write: 0,
            stutter_ready: false,
            stutter_segment_len,
            stutter_ramp_len: ramp_samples,
            stutter_ramp_pos: 0,
            freeze_bufs,
            freeze_idxs: [0; 4],
            freeze_lp: [0.0; 4],
            freeze_inject_pos: 0,
            freeze_inject_len,
        }
    }
}

pub(super) struct LivePitchShift {
    buf: Vec<f32>,
    buf_len: usize,
    pub(super) write_pos: usize,
    pos: f32,
    min_delay: f32,
    range: f32,
}

impl LivePitchShift {
    pub(super) fn new(sample_rate: u32) -> Self {
        let _ = sample_rate;
        Self {
            buf: vec![0.0; PITCH_BUF_FRAMES * 2],
            buf_len: PITCH_BUF_FRAMES,
            write_pos: 0,
            pos: PITCH_RANGE * 0.25,
            min_delay: PITCH_MIN_DELAY,
            range: PITCH_RANGE,
        }
    }

    pub(super) fn prefill_from_ring(&mut self, ring: &[f32], write_pos: usize) {
        let frames = ring.len().min(self.buf.len()) / 2;
        let offset = self.buf_len.saturating_sub(frames);
        let tail_frames = frames.min(ring.len().saturating_sub(write_pos) / 2);
        for i in 0..tail_frames {
            let src = (write_pos + i) * 2;
            let dst = (offset + i) * 2;
            if src + 1 < ring.len() && dst + 1 < self.buf.len() {
                self.buf[dst] = ring[src];
                self.buf[dst + 1] = ring[src + 1];
            }
        }
        let head_frames = frames.saturating_sub(tail_frames);
        for i in 0..head_frames {
            let src = i * 2;
            let dst = (offset + tail_frames + i) * 2;
            if src + 1 < ring.len() && dst + 1 < self.buf.len() {
                self.buf[dst] = ring[src];
                self.buf[dst + 1] = ring[src + 1];
            }
        }
        self.write_pos = self.buf_len - 1;
        self.pos = PITCH_RANGE * 0.25;
    }

    pub(super) fn process_frame(&mut self, l: f32, r: f32, ratio: f32) -> (f32, f32) {
        let buf_len_f = self.buf_len as f32;
        let min_delay = self.min_delay;
        let range = self.range;

        self.buf[self.write_pos * 2] = l;
        self.buf[self.write_pos * 2 + 1] = r;
        self.write_pos = (self.write_pos + 1) % self.buf_len;

        self.pos += 1.0 - ratio;
        let pos_norm = ((self.pos % range) + range) % range;

        let delay_a = min_delay + pos_norm;
        let delay_b = min_delay + ((pos_norm + range * 0.5) % range);

        let read_a = (self.write_pos as f32 - delay_a + buf_len_f) % buf_len_f;
        let read_b = (self.write_pos as f32 - delay_b + buf_len_f) % buf_len_f;

        let phase = ((pos_norm / range) + 0.5) % 1.0;
        let angle = phase * PI;
        let gain_a = angle.cos().powi(2);
        let gain_b = angle.sin().powi(2);

        let out_l = gain_a * Self::interp(&self.buf, read_a, 0)
            + gain_b * Self::interp(&self.buf, read_b, 0);
        let out_r = gain_a * Self::interp(&self.buf, read_a, 1)
            + gain_b * Self::interp(&self.buf, read_b, 1);

        (out_l, out_r)
    }

    fn interp(buf: &[f32], pos: f32, ch: usize) -> f32 {
        let i = pos as usize;
        let frac = pos - i as f32;
        let idx = i * 2 + ch;
        let a = buf.get(idx).copied().unwrap_or(0.0);
        let b = buf.get(idx + 2).copied().unwrap_or(0.0);
        a + frac * (b - a)
    }
}

pub(super) fn stutter_segment_len(sample_rate: u32, params: &BTreeMap<String, Value>) -> usize {
    let rate = param_f32(params, "rateHz", 8.0).clamp(1.0, 32.0);
    ((sample_rate as f32 / rate) as usize).clamp(48, sample_rate as usize)
}

pub(super) fn parse_route(route: &str) -> usize {
    if route == "direct" {
        return 0;
    }
    if let Some(rest) = route
        .strip_prefix("fx_bus_")
        .or_else(|| route.strip_prefix("bus_"))
    {
        if let Ok(n) = rest.parse::<usize>() {
            if n >= 1 {
                return n;
            }
        }
    }
    0
}

pub(super) fn parse_instrument_kind(kind: &str) -> InstrumentKind {
    match kind {
        "sampler" => InstrumentKind::Sample,
        "midi" => InstrumentKind::Midi,
        "none" => InstrumentKind::None,
        _ => InstrumentKind::Synth,
    }
}

pub(super) fn parse_momentary_fx_kind(kind: &str) -> Option<MomentaryFxKind> {
    match kind {
        "stutter" => Some(MomentaryFxKind::Stutter),
        "freeze" => Some(MomentaryFxKind::Freeze),
        "filter_sweep" => Some(MomentaryFxKind::FilterSweep),
        "pitch_shift" => Some(MomentaryFxKind::PitchShift),
        _ => None,
    }
}

pub(super) fn param_f32(params: &BTreeMap<String, Value>, key: &str, fallback: f32) -> f32 {
    params
        .get(key)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}

pub(super) fn sample_slot_for_note(note: u8) -> usize {
    note.saturating_sub(36)
        .min((SAMPLE_SLOTS_PER_INSTRUMENT - 1) as u8) as usize
}

pub(super) fn mono_frame(buffer: &SampleBuffer, frame: usize) -> f32 {
    let channels = buffer.channels.max(1) as usize;
    let base = frame.saturating_mul(channels);
    if channels == 1 {
        return buffer.samples.get(base).copied().unwrap_or(0.0);
    }
    let left = buffer.samples.get(base).copied().unwrap_or(0.0);
    let right = buffer.samples.get(base + 1).copied().unwrap_or(left);
    (left + right) * 0.5
}

pub(super) fn pan_gains(pan_pos: usize, positions: usize) -> (f32, f32) {
    if positions <= 1 {
        return (0.70710677, 0.70710677);
    }
    let t = (pan_pos.min(positions - 1) as f32) / ((positions - 1) as f32);
    let theta = t * (std::f32::consts::FRAC_PI_2);
    (theta.cos(), theta.sin())
}

pub(super) fn pan_gains_float(pos: f32) -> (f32, f32) {
    let theta = pos.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (theta.cos(), theta.sin())
}

pub(super) fn midi_note_to_hz(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}
