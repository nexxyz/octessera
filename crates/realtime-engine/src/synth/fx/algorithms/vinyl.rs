use super::*;

const TUNED_SAMPLE_RATE: u32 = 44_100;

#[derive(Clone, Copy, Debug)]
struct VinylRateConstants {
    sample_rate: u32,
    tone_coef: f32,
    crackle_trigger: f32,
    crackle_decay: f32,
}

impl VinylRateConstants {
    fn for_rate(sample_rate: u32) -> Self {
        let samples_per_tuned_sample = f64::from(TUNED_SAMPLE_RATE) / f64::from(sample_rate.max(1));
        Self {
            sample_rate,
            tone_coef: (1.0 - 0.92_f64.powf(samples_per_tuned_sample)) as f32,
            crackle_trigger: (0.0015 * samples_per_tuned_sample) as f32,
            crackle_decay: 0.94_f64.powf(samples_per_tuned_sample) as f32,
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::synth) struct VinylState {
    wow_phase: f32,
    flutter_phase: f32,
    crackle_amp: f32,
    crackle_pan: f32,
    rng: u32,
    tone_l: f32,
    tone_r: f32,
    rate: VinylRateConstants,
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
            rate: VinylRateConstants::for_rate(TUNED_SAMPLE_RATE),
        }
    }

    fn rate_constants(&mut self, sample_rate: u32) -> VinylRateConstants {
        if self.rate.sample_rate != sample_rate {
            self.rate = VinylRateConstants::for_rate(sample_rate);
        }
        self.rate
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
        + ((crate::sine::sin_radians(state.wow_phase) * 0.75
            + crate::sine::sin_radians(state.flutter_phase) * 0.25)
            * 0.08
            * params.warp_depth);

    let tone_mix = (0.08 + params.warp_depth * 0.24).clamp(0.0, 0.45);
    let drive = 1.0 + params.saturation * 4.0;
    let wet_l = process_saturator(input * warp, drive, 1.0);
    let rate = state.rate_constants(sample_rate);
    state.tone_l += (wet_l - state.tone_l) * rate.tone_coef;
    state.tone_r += (wet_l - state.tone_r) * rate.tone_coef;
    let wet_l = wet_l * (1.0 - tone_mix) + state.tone_l * tone_mix;

    advance_crackle(state, params.crackle, rate);
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
        + ((crate::sine::sin_radians(state.wow_phase) * 0.75
            + crate::sine::sin_radians(state.flutter_phase) * 0.25)
            * 0.08
            * params.warp_depth);

    let tone_mix = (0.08 + params.warp_depth * 0.24).clamp(0.0, 0.45);
    let drive = 1.0 + params.saturation * 4.0;
    let wet_l = process_saturator(left * warp, drive, 1.0);
    let wet_r = process_saturator(right * warp, drive, 1.0);
    let rate = state.rate_constants(sample_rate);
    state.tone_l += (wet_l - state.tone_l) * rate.tone_coef;
    state.tone_r += (wet_r - state.tone_r) * rate.tone_coef;
    let wet_l = wet_l * (1.0 - tone_mix) + state.tone_l * tone_mix;
    let wet_r = wet_r * (1.0 - tone_mix) + state.tone_r * tone_mix;

    advance_crackle(state, params.crackle, rate);
    let noise = next_noise(state) * state.crackle_amp;
    let crackle_l = noise * (1.0 - state.crackle_pan).clamp(0.0, 1.5);
    let crackle_r = noise * (1.0 + state.crackle_pan).clamp(0.0, 1.5);

    (
        mix_sample(left, wet_l + crackle_l, params.mix),
        mix_sample(right, wet_r + crackle_r, params.mix),
    )
}

fn advance_crackle(state: &mut VinylState, crackle: f32, rate: VinylRateConstants) {
    state.rng = state.rng.wrapping_mul(1664525).wrapping_add(1013904223);
    let trigger = ((state.rng >> 8) as f32) / ((u32::MAX >> 8) as f32);
    if trigger < crackle * rate.crackle_trigger {
        state.rng = state.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let pan = ((state.rng >> 8) as f32) / ((u32::MAX >> 8) as f32);
        state.rng = state.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let amp = ((state.rng >> 8) as f32) / ((u32::MAX >> 8) as f32);
        state.crackle_pan = pan * 2.0 - 1.0;
        state.crackle_amp = crackle * (0.03 + amp * 0.18);
    }
    state.crackle_amp *= rate.crackle_decay;
}

fn next_noise(state: &mut VinylState) -> f32 {
    state.rng = state.rng.wrapping_mul(1664525).wrapping_add(1013904223);
    (((state.rng >> 8) as f32) / ((u32::MAX >> 8) as f32)) * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATES: [u32; 2] = [44_100, 48_000];

    fn crackles_per_second(sample_rate: u32) -> f64 {
        let mut state = VinylState::new();
        let params = VinylParams {
            saturation: 0.0,
            crackle: 1.0,
            warp_depth: 0.0,
            mix: 1.0,
        };
        let seconds = 30;
        let mut crackles = 0;
        let mut previous = 0.0;
        for _ in 0..sample_rate * seconds {
            process_vinyl_stereo(&mut state, 0.0, 0.0, params, sample_rate);
            if state.crackle_amp > previous {
                crackles += 1;
            }
            previous = state.crackle_amp;
        }
        f64::from(crackles) / f64::from(seconds)
    }

    fn crackle_ring_ms(sample_rate: u32) -> f64 {
        let mut state = VinylState::new();
        state.crackle_amp = 1.0;
        let rate = state.rate_constants(sample_rate);
        let mut samples = 0;
        while state.crackle_amp > 0.01 {
            advance_crackle(&mut state, 0.0, rate);
            samples += 1;
        }
        1000.0 * f64::from(samples) / f64::from(sample_rate)
    }

    fn tone_level(sample_rate: u32) -> f64 {
        let mut state = VinylState::new();
        let params = VinylParams {
            saturation: 0.0,
            crackle: 0.0,
            warp_depth: 1.0,
            mix: 1.0,
        };
        let frames = sample_rate as usize;
        let energy: f64 = (0..frames)
            .map(|frame| {
                let input =
                    (std::f32::consts::TAU * 600.0 * frame as f32 / sample_rate as f32).sin() * 0.1;
                let (left, _) = process_vinyl_stereo(&mut state, input, input, params, sample_rate);
                f64::from(left * left)
            })
            .sum();
        10.0 * (energy / frames as f64).log10()
    }

    #[test]
    fn crackle_density_and_ring_follow_time_not_samples() {
        let [low, high] = RATES.map(crackles_per_second);
        assert!(
            (high / low - 1.0).abs() < 0.05,
            "{low} vs {high} crackles/s"
        );
        let [low, high] = RATES.map(crackle_ring_ms);
        assert!((high - low).abs() < 0.03, "{low} vs {high} ms ring");
    }

    #[test]
    fn tone_filter_follows_time_not_samples() {
        let [low, high] = RATES.map(tone_level);
        assert!((high - low).abs() < 0.01, "{low} vs {high} dB at 600 Hz");
    }

    #[test]
    fn tuned_rate_keeps_the_shipped_constants() {
        let rate = VinylRateConstants::for_rate(TUNED_SAMPLE_RATE);
        assert_eq!(rate.tone_coef.to_bits(), 0.08_f32.to_bits());
        assert_eq!(rate.crackle_trigger.to_bits(), 0.0015_f32.to_bits());
        assert_eq!(rate.crackle_decay.to_bits(), 0.94_f32.to_bits());
    }

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
