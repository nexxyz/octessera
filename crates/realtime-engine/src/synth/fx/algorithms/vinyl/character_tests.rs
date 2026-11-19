use super::*;

const SAMPLE_RATE: u32 = 48_000;

fn params(saturation: f32, crackle: f32, mix: f32) -> VinylParams {
    VinylParams {
        saturation,
        crackle,
        warp_depth: 0.0,
        mix,
    }
}

fn run(input: f32, params: VinylParams, frames: usize) -> Vec<f32> {
    let mut state = VinylState::new();
    (0..frames)
        .map(|_| process_vinyl_mono_bus(&mut state, input, params, SAMPLE_RATE))
        .collect()
}

#[test]
fn without_crackle_or_warp_a_steady_level_settles_on_the_saturation_curve() {
    for saturation in [0.0, 0.5, 1.0] {
        let output = run(0.4, params(saturation, 0.0, 1.0), 4_800);
        let expected = (0.4 * (1.0 + 4.0 * saturation)).tanh();
        let settled = output[output.len() - 1];
        assert!(
            (settled - expected).abs() < 1.0e-5,
            "saturation {saturation}: {settled}, expected {expected}"
        );
    }
}

#[test]
fn without_crackle_silence_stays_silent_and_with_crackle_it_pops() {
    let quiet = run(0.0, params(0.5, 0.0, 1.0), SAMPLE_RATE as usize);
    assert!(quiet.iter().all(|sample| *sample == 0.0));
    let crackling = run(0.0, params(0.5, 1.0, 1.0), SAMPLE_RATE as usize);
    assert!(crackling.iter().any(|sample| sample.abs() > 1.0e-3));
}

#[test]
fn a_zero_mix_passes_the_input_through_bit_for_bit() {
    let mut state = VinylState::new();
    for frame in 0..SAMPLE_RATE as usize {
        let input = (frame as f32 * 0.021).sin() * 0.7;
        let output = process_vinyl_mono_bus(&mut state, input, params(1.0, 1.0, 0.0), SAMPLE_RATE);
        assert_eq!(output.to_bits(), input.to_bits(), "frame {frame}");
    }
}
