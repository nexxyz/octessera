/// Four damped feedback combs, the first two summed to the left and the last two to the right:
/// the tail network shared by freeze and wash.
pub(in crate::synth::engine) fn comb_bank_frame(
    bufs: &mut [Vec<f32>; 4],
    idxs: &mut [usize; 4],
    lp: &mut [f32; 4],
    input: (f32, f32),
    feedback: f32,
    damp: f32,
) -> (f32, f32) {
    let mut wet_l = 0.0_f32;
    let mut wet_r = 0.0_f32;
    for i in 0..4 {
        let delayed = bufs[i][idxs[i]];
        lp[i] = delayed * (1.0 - damp) + lp[i] * damp;
        let channel_in = if i < 2 { input.0 } else { input.1 };
        bufs[i][idxs[i]] = channel_in + lp[i] * feedback;
        idxs[i] = (idxs[i] + 1) % bufs[i].len();
        if i < 2 {
            wet_l += delayed;
        } else {
            wet_r += delayed;
        }
    }
    (wet_l * 0.5, wet_r * 0.5)
}
