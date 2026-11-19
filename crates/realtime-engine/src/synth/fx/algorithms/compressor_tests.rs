use super::{process_compressor, CompressorParams};
use crate::synth::fx::FxBusState;

const SAMPLE_RATE: u32 = 48_000;

fn params(threshold_db: f32, ratio: f32, makeup_db: f32, mix: f32) -> CompressorParams {
    CompressorParams {
        threshold_db,
        ratio,
        attack_ms: 10.0,
        release_ms: 100.0,
        makeup_db,
        mix,
    }
}

fn copy(params: &CompressorParams) -> CompressorParams {
    CompressorParams { ..*params }
}

fn compress(state: &mut FxBusState, input: &[f32], params: &CompressorParams) -> Vec<f32> {
    input
        .iter()
        .map(|sample| process_compressor(state, *sample, copy(params), SAMPLE_RATE))
        .collect()
}

fn fresh() -> FxBusState {
    FxBusState::Compressor { env: 0.0 }
}

fn db(level: f32) -> f32 {
    20.0 * level.log10()
}

/// The static curve: everything above the threshold shrinks to 1/ratio, plus the makeup.
fn expected_gain(level: f32, params: &CompressorParams) -> f32 {
    let over = (db(level) - params.threshold_db).max(0.0);
    let reduction = over - over / params.ratio;
    10.0_f32.powf((params.makeup_db - reduction) / 20.0)
}

/// The detector level that produces `gain` above the threshold, read back off the curve.
fn detector_level(gain: f32, params: &CompressorParams) -> f32 {
    let reduction = -db(gain);
    10.0_f32.powf((params.threshold_db + reduction / (1.0 - 1.0 / params.ratio)) / 20.0)
}

#[test]
fn a_steady_level_above_the_threshold_settles_on_the_ratio() {
    for ratio in [1.0, 2.0, 4.0, 10.0] {
        for level in [0.2, 0.5, 0.9] {
            let params = params(-20.0, ratio, 0.0, 1.0);
            let output = compress(&mut fresh(), &[level; 48_000], &params);
            let settled = output[output.len() - 1] / level;
            let expected = expected_gain(level, &params);
            assert!(
                (db(settled) - db(expected)).abs() < 0.05,
                "ratio {ratio} level {level}: gain {} dB, expected {} dB",
                db(settled),
                db(expected)
            );
        }
    }
}

#[test]
fn below_the_threshold_only_the_makeup_applies() {
    for makeup_db in [0.0, 6.0] {
        let params = params(-20.0, 4.0, makeup_db, 1.0);
        let output = compress(&mut fresh(), &[0.05; 4_800], &params);
        let gain = output[output.len() - 1] / 0.05;
        assert!(
            (db(gain) - makeup_db).abs() < 0.01,
            "makeup {makeup_db} dB: gain {} dB",
            db(gain)
        );
    }
}

#[test]
fn the_mix_blends_the_dry_and_compressed_signal() {
    let level = 0.5;
    let params = params(-20.0, 4.0, 0.0, 0.5);
    let output = compress(&mut fresh(), &[level; 48_000], &params);
    let wet = level * expected_gain(level, &params);
    let expected = 0.5 * level + 0.5 * wet;
    assert!((output[output.len() - 1] - expected).abs() < 1.0e-3);
}

#[test]
fn the_detector_reaches_63_percent_of_a_step_after_one_attack_time() {
    let params = params(-20.0, 4.0, 0.0, 1.0);
    let attack_frames = 480;
    let output = compress(&mut fresh(), &[0.5; 480], &params);
    let level = detector_level(output[attack_frames - 1] / 0.5, &params);
    let expected = 0.5 * (1.0 - (-1.0_f32).exp());
    assert!(
        (level - expected).abs() < 0.005,
        "detector {level}, expected {expected}"
    );
}

#[test]
fn the_detector_falls_63_percent_of_a_step_after_one_release_time() {
    let params = params(-20.0, 4.0, 0.0, 1.0);
    let mut state = fresh();
    compress(&mut state, &[0.5; 48_000], &params);
    let release_frames = 4_800;
    let output = compress(&mut state, &[0.2; 4_800], &params);
    let level = detector_level(output[release_frames - 1] / 0.2, &params);
    let expected = 0.2 + 0.3 * (-1.0_f32).exp();
    assert!(
        (level - expected).abs() < 0.005,
        "detector {level}, expected {expected}"
    );
}
