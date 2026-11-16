use std::f32::consts::TAU;

/// The power spectrum of a Hann-windowed real block: a complex FFT of half the size over the
/// block's even and odd samples, split into the real block's bins. The tables are built once.
#[derive(Clone)]
pub(super) struct Fft {
    /// Window weights for each sample pair, and the slot each pair takes in the half-size
    /// transform.
    window: Vec<(f32, f32)>,
    order: Vec<usize>,
    /// `e^(-2 pi i k / size)` for `k < size / 2`, for the split into real bins.
    twiddles: Vec<(f32, f32)>,
    /// Each butterfly stage's twiddles in a row, `span - 1..2 * span - 1` for blocks of
    /// `2 * span`, so the inner loop reads them in order.
    stage_cos: Vec<f32>,
    stage_sin: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
    powers: Vec<f32>,
}

impl Fft {
    pub(super) fn new(size: usize) -> Self {
        assert!(size.is_power_of_two() && size >= 4);
        let half = size / 2;
        let bits = half.trailing_zeros();
        let hann = |index: usize| 0.5 - 0.5 * (TAU * index as f32 / size as f32).cos();
        let twiddles = (0..half)
            .map(|index| {
                let angle = -TAU * index as f32 / size as f32;
                (angle.cos(), angle.sin())
            })
            .collect();
        let stage_angles = || {
            std::iter::successors(Some(1), |span| Some(span * 2))
                .take_while(|span| *span < half)
                .flat_map(|span| {
                    (0..span).map(move |index| -TAU * index as f32 / (2 * span) as f32)
                })
        };
        Self {
            stage_cos: stage_angles().map(f32::cos).collect(),
            stage_sin: stage_angles().map(f32::sin).collect(),
            window: (0..half)
                .map(|pair| (hann(2 * pair), hann(2 * pair + 1)))
                .collect(),
            order: (0..half)
                .map(|pair| pair.reverse_bits() >> (usize::BITS - bits))
                .collect(),
            twiddles,
            re: vec![0.0; half],
            im: vec![0.0; half],
            powers: vec![0.0; half],
        }
    }

    /// The squared magnitudes of bins `0..size / 2`; a full-scale sine on a bin peaks near
    /// `(size / 4)^2`.
    pub(super) fn powers(&mut self, samples: &[f32]) -> &[f32] {
        let half = self.re.len();
        let pairs = samples.as_chunks::<2>().0.iter().zip(&self.window);
        for (([even, odd], (even_weight, odd_weight)), &slot) in pairs.zip(&self.order) {
            self.re[slot] = even * even_weight;
            self.im[slot] = odd * odd_weight;
        }
        for (pair_re, pair_im) in self
            .re
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(self.im.as_chunks_mut::<2>().0)
        {
            let [low_re, high_re] = *pair_re;
            let [low_im, high_im] = *pair_im;
            *pair_re = [low_re + high_re, low_re - high_re];
            *pair_im = [low_im + high_im, low_im - high_im];
        }
        let mut span = 2;
        while span < half {
            let cos = &self.stage_cos[span - 1..2 * span - 1];
            let sin = &self.stage_sin[span - 1..2 * span - 1];
            let blocks = self.re.chunks_exact_mut(2 * span);
            for (block_re, block_im) in blocks.zip(self.im.chunks_exact_mut(2 * span)) {
                let (low_re, high_re) = block_re.split_at_mut(span);
                let (low_im, high_im) = block_im.split_at_mut(span);
                let lows = low_re.iter_mut().zip(low_im.iter_mut());
                let highs = high_re.iter_mut().zip(high_im.iter_mut());
                let twiddles = cos.iter().zip(sin);
                for (((low_re, low_im), (high_re, high_im)), (cos, sin)) in
                    lows.zip(highs).zip(twiddles)
                {
                    let re = *high_re * cos - *high_im * sin;
                    let im = *high_re * sin + *high_im * cos;
                    *high_re = *low_re - re;
                    *high_im = *low_im - im;
                    *low_re += re;
                    *low_im += im;
                }
            }
            span *= 2;
        }
        for (bin, power) in self.powers.iter_mut().enumerate() {
            let mirror = (half - bin) % half;
            let (re, im) = (self.re[bin], self.im[bin]);
            let (mirror_re, mirror_im) = (self.re[mirror], -self.im[mirror]);
            let (even_re, even_im) = ((re + mirror_re) * 0.5, (im + mirror_im) * 0.5);
            let (odd_re, odd_im) = ((im - mirror_im) * 0.5, (mirror_re - re) * 0.5);
            let (cos, sin) = self.twiddles[bin];
            let bin_re = even_re + odd_re * cos - odd_im * sin;
            let bin_im = even_im + odd_re * sin + odd_im * cos;
            *power = bin_re * bin_re + bin_im * bin_im;
        }
        &self.powers
    }
}
