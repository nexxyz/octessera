use std::f32::consts::PI;

pub(super) const PITCH_BUF_FRAMES: usize = 2048;
const PITCH_MIN_DELAY: f32 = 64.0;
const PITCH_RANGE: f32 = 1024.0;
pub(super) const PITCH_FILL_FRAMES: u32 = (PITCH_MIN_DELAY + PITCH_RANGE) as u32;
const PITCH_ACTIVATION_RAMP_MS: u32 = 10;
pub(super) const PITCH_DEFAULT_SLIDE_IN_MS: f32 = 120.0;
pub(super) const PITCH_DEFAULT_SLIDE_OUT_MS: f32 = 180.0;

pub(super) fn pitch_activation_ramp_len(sample_rate: u32) -> u32 {
    (sample_rate.saturating_mul(PITCH_ACTIVATION_RAMP_MS) / 1_000).max(1)
}

#[derive(Clone)]
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
        let frames = buf.len() / 2;
        let base = pos.floor();
        let i = (base as usize) % frames;
        let frac = pos - base;
        let idx = i * 2 + ch;
        let a = buf.get(idx).copied().unwrap_or(0.0);
        let next = (i + 1) % frames;
        let b = buf.get(next * 2 + ch).copied().unwrap_or(0.0);
        a + frac * (b - a)
    }
}

#[cfg(test)]
mod tests;
