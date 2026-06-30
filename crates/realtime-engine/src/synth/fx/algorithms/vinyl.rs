use super::*;

#[derive(Clone, Debug)]
pub(in crate::synth) struct VinylState {
    wow_phase: f32,
    flutter_phase: f32,
    crackle_amp: f32,
    crackle_pan: f32,
    rng: u32,
    tone_l: f32,
    tone_r: f32,
}

impl VinylState {
    pub(in crate::synth) fn new() -> Self {
        Self {
            wow_phase: 0.0,
            flutter_phase: 0.0,
            crackle_amp: 0.0,
            crackle_pan: 0.0,
            rng: 0x4d59_5df4,
            tone_l: 0.0,
            tone_r: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::synth::fx) struct VinylParams {
    pub(in crate::synth::fx) saturation: f32,
    pub(in crate::synth::fx) crackle: f32,
    pub(in crate::synth::fx) warp_depth: f32,
    pub(in crate::synth::fx) mix: f32,
}

pub(in crate::synth::fx) fn process_vinyl_mono_bus(
    state: &mut VinylState,
    input: f32,
    params: VinylParams,
    sample_rate: u32,
) -> f32 {
    state.wow_phase = wrap_phase(state.wow_phase + 2.0 * PI * 0.33 / sample_rate as f32);
    state.flutter_phase = wrap_phase(state.flutter_phase + 2.0 * PI * 4.7 / sample_rate as f32);
    let warp = 1.0
        + ((state.wow_phase.sin() * 0.75 + state.flutter_phase.sin() * 0.25)
            * 0.08
            * params.warp_depth);

    let tone_mix = (0.08 + params.warp_depth * 0.24).clamp(0.0, 0.45);
    let drive = 1.0 + params.saturation * 4.0;
    let wet_l = process_saturator(input * warp, drive, 1.0);
    state.tone_l += (wet_l - state.tone_l) * 0.08;
    state.tone_r += (wet_l - state.tone_r) * 0.08;
    let wet_l = wet_l * (1.0 - tone_mix) + state.tone_l * tone_mix;

    advance_crackle(state, params.crackle);
    let noise = next_noise(state) * state.crackle_amp;
    let crackle_l = noise * (1.0 - state.crackle_pan).clamp(0.0, 1.5);

    mix_sample(input, wet_l + crackle_l, params.mix)
}

pub(in crate::synth::fx) fn process_vinyl_stereo(
    state: &mut VinylState,
    left: f32,
    right: f32,
    params: VinylParams,
    sample_rate: u32,
) -> (f32, f32) {
    state.wow_phase = wrap_phase(state.wow_phase + 2.0 * PI * 0.33 / sample_rate as f32);
    state.flutter_phase = wrap_phase(state.flutter_phase + 2.0 * PI * 4.7 / sample_rate as f32);
    let warp = 1.0
        + ((state.wow_phase.sin() * 0.75 + state.flutter_phase.sin() * 0.25)
            * 0.08
            * params.warp_depth);

    let tone_mix = (0.08 + params.warp_depth * 0.24).clamp(0.0, 0.45);
    let drive = 1.0 + params.saturation * 4.0;
    let wet_l = process_saturator(left * warp, drive, 1.0);
    let wet_r = process_saturator(right * warp, drive, 1.0);
    state.tone_l += (wet_l - state.tone_l) * 0.08;
    state.tone_r += (wet_r - state.tone_r) * 0.08;
    let wet_l = wet_l * (1.0 - tone_mix) + state.tone_l * tone_mix;
    let wet_r = wet_r * (1.0 - tone_mix) + state.tone_r * tone_mix;

    advance_crackle(state, params.crackle);
    let noise = next_noise(state) * state.crackle_amp;
    let crackle_l = noise * (1.0 - state.crackle_pan).clamp(0.0, 1.5);
    let crackle_r = noise * (1.0 + state.crackle_pan).clamp(0.0, 1.5);

    (
        mix_sample(left, wet_l + crackle_l, params.mix),
        mix_sample(right, wet_r + crackle_r, params.mix),
    )
}

fn advance_crackle(state: &mut VinylState, crackle: f32) {
    state.rng = state.rng.wrapping_mul(1664525).wrapping_add(1013904223);
    let trigger = ((state.rng >> 8) as f32) / ((u32::MAX >> 8) as f32);
    if trigger < crackle * 0.0015 {
        state.rng = state.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let pan = ((state.rng >> 8) as f32) / ((u32::MAX >> 8) as f32);
        state.rng = state.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let amp = ((state.rng >> 8) as f32) / ((u32::MAX >> 8) as f32);
        state.crackle_pan = pan * 2.0 - 1.0;
        state.crackle_amp = crackle * (0.03 + amp * 0.18);
    }
    state.crackle_amp *= 0.94;
}

fn next_noise(state: &mut VinylState) -> f32 {
    state.rng = state.rng.wrapping_mul(1664525).wrapping_add(1013904223);
    (((state.rng >> 8) as f32) / ((u32::MAX >> 8) as f32)) * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_bus_vinyl_matches_duplicated_input_stereo_left_path() {
        let mut mono = VinylState::new();
        let mut stereo = VinylState::new();
        let sample_rate = 44_100;

        for frame in 0..4096 {
            let input =
                ((frame as f32) * 0.017).sin() * 0.35 + ((frame as f32) * 0.031).cos() * 0.12;
            let params = VinylParams {
                saturation: 0.15,
                crackle: 0.8,
                warp_depth: 0.5,
                mix: 1.0,
            };
            let mono_out = process_vinyl_mono_bus(&mut mono, input, params, sample_rate);
            let (stereo_left, _) =
                process_vinyl_stereo(&mut stereo, input, input, params, sample_rate);
            assert_eq!(mono_out.to_bits(), stereo_left.to_bits(), "frame {frame}");
            assert_vinyl_state_eq(&mono, &stereo, frame);
        }
    }

    fn assert_vinyl_state_eq(left: &VinylState, right: &VinylState, frame: usize) {
        assert_eq!(
            left.wow_phase.to_bits(),
            right.wow_phase.to_bits(),
            "wow {frame}"
        );
        assert_eq!(
            left.flutter_phase.to_bits(),
            right.flutter_phase.to_bits(),
            "flutter {frame}"
        );
        assert_eq!(
            left.crackle_amp.to_bits(),
            right.crackle_amp.to_bits(),
            "amp {frame}"
        );
        assert_eq!(
            left.crackle_pan.to_bits(),
            right.crackle_pan.to_bits(),
            "pan {frame}"
        );
        assert_eq!(left.rng, right.rng, "rng {frame}");
        assert_eq!(
            left.tone_l.to_bits(),
            right.tone_l.to_bits(),
            "tone_l {frame}"
        );
        assert_eq!(
            left.tone_r.to_bits(),
            right.tone_r.to_bits(),
            "tone_r {frame}"
        );
    }
}
