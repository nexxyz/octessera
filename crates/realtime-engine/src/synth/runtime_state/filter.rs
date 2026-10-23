use super::super::types::FilterType;
#[cfg(test)]
use std::cell::Cell;
use std::f32::consts::PI;

#[cfg(test)]
thread_local! {
    static PREPARE_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
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
    /// Samples to pass through unfiltered: `OPEN` while a resonance-free lowpass sits at the top
    /// of the range or a resonance-free highpass at the bottom (nothing runs), then two counted
    /// samples on re-engaging that refill the history the biquad needs.
    pass_frames: u8,
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
            pass_frames: 0,
        }
    }

    pub(in crate::synth) fn prepare(
        &mut self,
        mode: FilterType,
        cutoff_hz: f32,
        q: f32,
        sample_rate: u32,
    ) {
        #[cfg(test)]
        PREPARE_COUNT.with(|count| count.set(count.get() + 1));
        let cutoff = cutoff_hz.clamp(20.0, 20_000.0);
        let qv = q.clamp(0.25, 20.0);
        if self.needs_coeffs(mode, cutoff, qv, sample_rate) {
            self.mode = mode;
            self.cutoff_hz = cutoff;
            self.q = qv;
            self.sample_rate = sample_rate;
            self.coeffs = biquad_coeffs(mode, cutoff, qv, sample_rate);
            let open = qv <= std::f32::consts::FRAC_1_SQRT_2
                && match mode {
                    FilterType::Lowpass => cutoff >= OPEN_LOWPASS_CUTOFF_HZ,
                    FilterType::Highpass => cutoff <= OPEN_HIGHPASS_CUTOFF_HZ,
                    FilterType::Bandpass | FilterType::Notch => false,
                };
            if open {
                self.pass_frames = OPEN;
            } else if self.pass_frames == OPEN {
                self.pass_frames = HISTORY_FRAMES;
            }
        }
    }

    /// True while the prepared settings leave audio unchanged, so callers can skip the filter.
    pub(in crate::synth) fn is_open(&self) -> bool {
        self.pass_frames == OPEN
    }

    pub(in crate::synth) fn process_prepared(&mut self, x: f32) -> f32 {
        if self.pass_frames != 0 {
            if self.pass_frames != OPEN {
                self.pass_frames -= 1;
                self.x2 = self.x1;
                self.x1 = x;
                self.y2 = self.y1;
                self.y1 = x;
            }
            return x;
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

    pub(in crate::synth) fn process(
        &mut self,
        x: f32,
        mode: FilterType,
        cutoff_hz: f32,
        q: f32,
        sample_rate: u32,
    ) -> f32 {
        self.prepare(mode, cutoff_hz, q, sample_rate);
        self.process_prepared(x)
    }

    fn needs_coeffs(&self, mode: FilterType, cutoff: f32, qv: f32, sample_rate: u32) -> bool {
        self.mode != mode
            || self.cutoff_hz != cutoff
            || self.q != qv
            || self.sample_rate != sample_rate
    }
}

#[cfg(test)]
pub(in crate::synth) fn reset_prepare_count_for_test() {
    PREPARE_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(in crate::synth) fn prepare_count_for_test() -> usize {
    PREPARE_COUNT.with(Cell::get)
}

const OPEN_LOWPASS_CUTOFF_HZ: f32 = 20_000.0;
const OPEN_HIGHPASS_CUTOFF_HZ: f32 = 20.0;
const OPEN: u8 = u8::MAX;
const HISTORY_FRAMES: u8 = 2;

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

#[cfg(test)]
mod open_lowpass_tests {
    use super::*;

    #[test]
    fn a_resonance_free_lowpass_at_the_top_passes_audio_and_refills_history_before_resuming() {
        let mut open = BiquadState::new();
        open.prepare(FilterType::Lowpass, 20_000.0, 0.5, 48_000);
        let input = [0.3, -0.7, 0.9, 0.1, -0.2];
        for x in input {
            assert_eq!(open.process_prepared(x), x);
        }
        let mut resonant = BiquadState::new();
        resonant.prepare(FilterType::Lowpass, 20_000.0, 6.0, 48_000);
        assert!(input.iter().any(|&x| resonant.process_prepared(x) != x));

        open.prepare(FilterType::Lowpass, 19_000.0, 0.5, 48_000);
        assert_eq!(open.process_prepared(0.4), 0.4);
        assert_eq!(open.process_prepared(0.5), 0.5);
        assert_eq!((open.x1, open.x2, open.y1, open.y2), (0.5, 0.4, 0.5, 0.4));
        let resumed = open.process_prepared(0.5);
        assert!((resumed - 0.5).abs() < 0.1, "{resumed}");
    }
}
