/// The lengths of the four damped feedback combs shared by the bus reverb, freeze and wash, at
/// 44.1 kHz.
pub const REVERB_COMB_LENGTHS: [usize; 4] = [1557, 1617, 1491, 1422];

/// Empty comb lines at `sample_rate`, `scale` times the shared lengths.
pub(in crate::synth) fn comb_buffers(scale: usize, sample_rate: u32) -> [Vec<f32>; 4] {
    REVERB_COMB_LENGTHS
        .map(|n| super::resident_zeros((n * scale * sample_rate as usize / 44_100).max(1)))
}

const REVERB_FULL_INPUT_DECAY: f32 = 0.9;

/// The bus reverb's comb input gain, recomputed only when the decay changes: unity up to a
/// decay of 0.9, then falling with the tail's energy gain so a long decay does not clip.
#[derive(Clone, Copy, Debug)]
pub(in crate::synth) struct ReverbInputGain {
    decay: f32,
    gain: f32,
}

impl Default for ReverbInputGain {
    fn default() -> Self {
        Self {
            decay: 0.0,
            gain: 1.0,
        }
    }
}

impl ReverbInputGain {
    pub(in crate::synth) fn for_decay(&mut self, decay: f32) -> f32 {
        if self.decay.to_bits() != decay.to_bits() {
            self.decay = decay;
            self.gain = if decay <= REVERB_FULL_INPUT_DECAY {
                1.0
            } else {
                let full = 1.0 - REVERB_FULL_INPUT_DECAY * REVERB_FULL_INPUT_DECAY;
                ((1.0 - decay * decay).max(0.0) / full).sqrt().min(1.0)
            };
        }
        self.gain
    }
}

/// One frame through the four combs, each fed its own input; returns what each comb delayed.
pub(in crate::synth) fn comb_bank_step(
    bufs: &mut [Vec<f32>; 4],
    idxs: &mut [usize; 4],
    lp: &mut [f32; 4],
    inputs: [f32; 4],
    feedback: f32,
    damp: f32,
) -> [f32; 4] {
    let mut delayed = [0.0_f32; 4];
    for i in 0..4 {
        delayed[i] = bufs[i][idxs[i]];
        lp[i] = delayed[i] * (1.0 - damp) + lp[i] * damp;
        bufs[i][idxs[i]] = inputs[i] + lp[i] * feedback;
        idxs[i] = (idxs[i] + 1) % bufs[i].len();
    }
    delayed
}

/// The stereo tail of freeze and wash: the first two combs summed to the left, the last two to
/// the right.
pub(in crate::synth) fn comb_bank_frame(
    bufs: &mut [Vec<f32>; 4],
    idxs: &mut [usize; 4],
    lp: &mut [f32; 4],
    input: (f32, f32),
    feedback: f32,
    damp: f32,
) -> (f32, f32) {
    let delayed = comb_bank_step(
        bufs,
        idxs,
        lp,
        [input.0, input.0, input.1, input.1],
        feedback,
        damp,
    );
    let sum = |combs: &[f32]| combs.iter().fold(0.0_f32, |wet, comb| wet + comb);
    (sum(&delayed[..2]) * 0.5, sum(&delayed[2..]) * 0.5)
}
