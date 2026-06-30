use super::super::types::FilterType;
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::synth) struct BiquadState {
    pub(in crate::synth) x1: f32,
    pub(in crate::synth) x2: f32,
    pub(in crate::synth) y1: f32,
    pub(in crate::synth) y2: f32,
    mode: FilterType,
    cutoff_hz: f32,
    q: f32,
    sample_rate: u32,
    coeffs: BiquadCoeffs,
}

impl BiquadState {
    pub(in crate::synth) fn new() -> Self {
        Self {
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
            mode: FilterType::Lowpass,
            cutoff_hz: 0.0,
            q: 0.0,
            sample_rate: 0,
            coeffs: BiquadCoeffs::passthrough(),
        }
    }

    pub(in crate::synth) fn process(
        &mut self,
        x: f32,
        mode: FilterType,
        cutoff_hz: f32,
        q: f32,
        sample_rate: u32,
    ) -> f32 {
        let cutoff = cutoff_hz.clamp(20.0, 20_000.0);
        let qv = q.clamp(0.25, 20.0);
        if self.needs_coeffs(mode, cutoff, qv, sample_rate) {
            self.mode = mode;
            self.cutoff_hz = cutoff;
            self.q = qv;
            self.sample_rate = sample_rate;
            self.coeffs = biquad_coeffs(mode, cutoff, qv, sample_rate);
        }

        let y = self.coeffs.b0 * x + self.coeffs.b1 * self.x1 + self.coeffs.b2 * self.x2
            - self.coeffs.a1 * self.y1
            - self.coeffs.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    fn needs_coeffs(&self, mode: FilterType, cutoff: f32, qv: f32, sample_rate: u32) -> bool {
        self.mode != mode
            || self.cutoff_hz != cutoff
            || self.q != qv
            || self.sample_rate != sample_rate
    }
}

impl BiquadCoeffs {
    fn passthrough() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

fn biquad_coeffs(mode: FilterType, cutoff: f32, qv: f32, sample_rate: u32) -> BiquadCoeffs {
    let w0 = 2.0 * PI * cutoff / (sample_rate as f32);
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * qv);

    let (b0, b1, b2, a0, a1, a2) = match mode {
        FilterType::Lowpass => lowpass_coeffs(cos_w0, alpha),
        FilterType::Highpass => highpass_coeffs(cos_w0, alpha),
        FilterType::Bandpass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha),
        FilterType::Notch => (
            1.0,
            -2.0 * cos_w0,
            1.0,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        ),
    };

    BiquadCoeffs {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

fn lowpass_coeffs(cos_w0: f32, alpha: f32) -> (f32, f32, f32, f32, f32, f32) {
    (
        (1.0 - cos_w0) * 0.5,
        1.0 - cos_w0,
        (1.0 - cos_w0) * 0.5,
        1.0 + alpha,
        -2.0 * cos_w0,
        1.0 - alpha,
    )
}

fn highpass_coeffs(cos_w0: f32, alpha: f32) -> (f32, f32, f32, f32, f32, f32) {
    (
        (1.0 + cos_w0) * 0.5,
        -(1.0 + cos_w0),
        (1.0 + cos_w0) * 0.5,
        1.0 + alpha,
        -2.0 * cos_w0,
        1.0 - alpha,
    )
}
