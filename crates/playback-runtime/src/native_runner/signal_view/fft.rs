use std::f32::consts::TAU;

/// An in-place radix-2 FFT over a Hann-windowed block, with its tables built once.
#[derive(Clone)]
pub(super) struct Fft {
    window: Vec<f32>,
    twiddles: Vec<(f32, f32)>,
    re: Vec<f32>,
    im: Vec<f32>,
    magnitudes: Vec<f32>,
}

impl Fft {
    pub(super) fn new(size: usize) -> Self {
        assert!(size.is_power_of_two() && size >= 2);
        let window = (0..size)
            .map(|index| 0.5 - 0.5 * (TAU * index as f32 / size as f32).cos())
            .collect();
        let twiddles = (0..size / 2)
            .map(|index| {
                let angle = -TAU * index as f32 / size as f32;
                (angle.cos(), angle.sin())
            })
            .collect();
        Self {
            window,
            twiddles,
            re: vec![0.0; size],
            im: vec![0.0; size],
            magnitudes: vec![0.0; size / 2],
        }
    }

    /// The magnitudes of bins `0..size / 2`; a full-scale sine on a bin peaks near `size / 4`.
    pub(super) fn magnitudes(&mut self, samples: &[f32]) -> &[f32] {
        let size = self.re.len();
        let bits = size.trailing_zeros();
        for (index, (sample, weight)) in samples.iter().zip(&self.window).enumerate() {
            let target = index.reverse_bits() >> (usize::BITS - bits);
            self.re[target] = sample * weight;
            self.im[target] = 0.0;
        }
        let mut half = 1;
        while half < size {
            let stride = size / (half * 2);
            for start in (0..size).step_by(half * 2) {
                for offset in 0..half {
                    let (cos, sin) = self.twiddles[offset * stride];
                    let (even, odd) = (start + offset, start + offset + half);
                    let re = self.re[odd] * cos - self.im[odd] * sin;
                    let im = self.re[odd] * sin + self.im[odd] * cos;
                    self.re[odd] = self.re[even] - re;
                    self.im[odd] = self.im[even] - im;
                    self.re[even] += re;
                    self.im[even] += im;
                }
            }
            half *= 2;
        }
        for (bin, magnitude) in self.magnitudes.iter_mut().enumerate() {
            *magnitude = self.re[bin].hypot(self.im[bin]);
        }
        &self.magnitudes
    }
}
