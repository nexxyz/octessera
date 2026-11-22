use super::*;
use crate::synth::fx::algorithms::REVERB_COMB_LENGTHS;
use crate::synth::fx::fx_bus_state_from_params;
use crate::synth::fx_params::ModDelayKind;

const SAMPLE_RATE: u32 = 48_000;

fn run(params: FxBusParams, input: &[f32], sample_rate: u32) -> Vec<f32> {
    let mut state = fx_bus_state_from_params(&params, sample_rate);
    input
        .iter()
        .map(|sample| process_fx_bus_slot(&params, &mut state, *sample, 0.0, sample_rate))
        .collect()
}

fn impulse(frames: usize) -> Vec<f32> {
    let mut input = vec![0.0; frames];
    input[0] = 1.0;
    input
}

fn assert_close(actual: &[f32], expected: &[f32], tolerance: f32) {
    for (frame, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() <= tolerance,
            "frame {frame}: {actual}, expected {expected}"
        );
    }
}

/// An impulse through a feedback line `period` frames long: the dry part at frame zero, then
/// each return `mix * feedback^(n - 1)` at `n * period`.
fn echo_train(frames: usize, period: usize, feedback: f32, mix: f32) -> Vec<f32> {
    (0..frames)
        .map(|frame| match frame {
            0 => 1.0 - mix,
            _ if frame % period == 0 => mix * feedback.powi(frame as i32 / period as i32 - 1),
            _ => 0.0,
        })
        .collect()
}

#[test]
fn delay_repeats_an_impulse_each_delay_time_scaled_by_feedback_and_mix() {
    let (feedback, mix) = (0.5, 0.4);
    let params = FxBusParams::Delay {
        time_ms: 10.0,
        feedback,
        mix,
        spread: 0.0,
    };
    let output = run(params, &impulse(4 * 480 + 20), SAMPLE_RATE);
    assert_close(
        &output,
        &echo_train(4 * 480 + 20, 480, feedback, mix),
        1.0e-4,
    );
}

#[test]
fn a_delay_between_two_frames_splits_the_impulse_between_them() {
    let params = FxBusParams::Delay {
        time_ms: 480.5 / 48.0,
        feedback: 0.0,
        mix: 1.0,
        spread: 0.0,
    };
    let output = run(params, &impulse(600), SAMPLE_RATE);
    let mut expected = vec![0.0; 600];
    expected[480] = 0.5;
    expected[481] = 0.5;
    assert_close(&output, &expected, 1.0e-3);
}

#[test]
fn mod_delay_without_depth_is_a_fixed_delay_at_the_base_time() {
    let (feedback, mix) = (0.3, 0.5);
    let params = FxBusParams::ModDelay {
        kind: ModDelayKind::Chorus,
        rate_hz: 0.8,
        depth_ms: 0.0,
        base_ms: 5.0,
        feedback,
        mix,
    };
    let output = run(params, &impulse(4 * 240 + 20), SAMPLE_RATE);
    assert_close(
        &output,
        &echo_train(4 * 240 + 20, 240, feedback, mix),
        1.0e-4,
    );
}

#[test]
fn vibrato_delay_time_follows_the_lfo_between_base_and_base_plus_depth() {
    let (rate_hz, base_ms, depth_ms) = (5.0, 2.0, 4.0);
    let params = FxBusParams::ModDelay {
        kind: ModDelayKind::Vibrato,
        rate_hz,
        depth_ms,
        base_ms,
        feedback: 0.0,
        mix: 1.0,
    };
    let slope = 1.0e-4;
    let frames = SAMPLE_RATE as usize / 5;
    let ramp: Vec<f32> = (0..frames).map(|frame| frame as f32 * slope).collect();
    let output = run(params, &ramp, SAMPLE_RATE);
    let samples_per_ms = SAMPLE_RATE as f64 / 1000.0;
    let (mut shortest, mut longest) = (f64::MAX, 0.0_f64);
    for (frame, output) in output.iter().enumerate().skip(400) {
        let measured = frame as f64 - f64::from(*output) / f64::from(slope);
        let phase = std::f64::consts::TAU * f64::from(rate_hz) * frame as f64 / 48_000.0;
        let lfo = (phase.sin() + 1.0) * 0.5;
        let expected = (f64::from(base_ms) + f64::from(depth_ms) * lfo) * samples_per_ms;
        assert!(
            (measured - expected).abs() < 0.1,
            "frame {frame}: delay {measured} samples, expected {expected}"
        );
        shortest = shortest.min(measured);
        longest = longest.max(measured);
    }
    assert!((shortest - 2.0 * samples_per_ms).abs() < 0.15, "{shortest}");
    assert!((longest - 6.0 * samples_per_ms).abs() < 0.15, "{longest}");
}

/// What an impulse into the undamped reverb leaves: each comb returns it every comb length,
/// `decay` quieter each time, and the four combs share a quarter of the wet level.
/// Above decay 0.9 the comb input is scaled by `sqrt((1 - d^2) / (1 - 0.9^2))` so a long tail
/// does not clip; at or below 0.9 it is 1.
fn reverb_input_gain(decay: f32) -> f32 {
    if decay <= 0.9 {
        1.0
    } else {
        ((1.0 - decay * decay) / (1.0 - 0.81)).sqrt()
    }
}

fn undamped_reverb_impulse(frames: usize, decay: f32, mix: f32) -> Vec<f32> {
    let input_gain = reverb_input_gain(decay);
    (0..frames)
        .map(|frame| {
            let dry = if frame == 0 { 1.0 - mix } else { 0.0 };
            let wet: f32 = REVERB_COMB_LENGTHS
                .iter()
                .filter(|length| frame > 0 && frame % **length == 0)
                .map(|length| decay.powi((frame / length) as i32 - 1))
                .sum();
            dry + wet * input_gain * 0.25 * mix
        })
        .collect()
}

#[test]
fn undamped_reverb_returns_each_comb_echo_scaled_by_the_decay() {
    let frames = 3 * REVERB_COMB_LENGTHS[3] + 10;
    for decay in [0.3, 0.72, 0.95] {
        for mix in [1.0, 0.3] {
            let params = FxBusParams::Reverb {
                mix,
                decay,
                damp: 0.0,
            };
            let output = run(params, &impulse(frames), 44_100);
            assert_close(
                &output,
                &undamped_reverb_impulse(frames, decay, mix),
                1.0e-6,
            );
        }
    }
}

fn energy(samples: &[f32]) -> f64 {
    samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum()
}

#[test]
fn damped_reverb_tail_keeps_decaying_and_damping_shortens_it() {
    let second = 44_100;
    let tail = |damp: f32| {
        let params = FxBusParams::Reverb {
            mix: 1.0,
            decay: 0.9,
            damp,
        };
        run(params, &impulse(3 * second), 44_100)
    };
    let damped = tail(0.35);
    let windows: Vec<f64> = damped.chunks(second).map(energy).collect();
    assert!(
        windows[0] > windows[1] && windows[1] > windows[2],
        "{windows:?}"
    );
    let undamped = tail(0.0);
    assert!(energy(&damped[second..]) < energy(&undamped[second..]) * 0.5);
}

fn glitch(chance: f32, slice_ms: f32, mix: f32) -> FxBusParams {
    FxBusParams::Glitch {
        chance,
        slice_ms,
        mix,
    }
}

fn test_tone(frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|frame| (frame as f32 * 0.013).sin() * 0.6 + (frame as f32 * 0.071).cos() * 0.2)
        .collect()
}

#[test]
fn glitch_that_never_fires_passes_the_input_through_bit_for_bit() {
    let input = test_tone(SAMPLE_RATE as usize);
    for mix in [1.0, 0.5, 0.0] {
        let output = run(glitch(0.0, 80.0, mix), &input, SAMPLE_RATE);
        for (frame, (output, input)) in output.iter().zip(&input).enumerate() {
            assert_eq!(output.to_bits(), input.to_bits(), "mix {mix} frame {frame}");
        }
    }
}

#[test]
fn glitch_that_always_fires_hears_one_slice_then_keeps_repeating_it() {
    for slice_ms in [10.0, 500.0] {
        let slice = (slice_ms * 48.0) as usize;
        let input = test_tone(slice * 4);
        let output = run(glitch(1.0, slice_ms, 1.0), &input, SAMPLE_RATE);
        for frame in 0..slice {
            assert_eq!(
                output[frame].to_bits(),
                input[frame].to_bits(),
                "frame {frame}"
            );
        }
        for (frame, output) in output.iter().enumerate().skip(slice) {
            let into_slice = frame % slice;
            if (96..slice - 96).contains(&into_slice) {
                assert!(
                    (output - input[into_slice]).abs() < 1.0e-6,
                    "{slice_ms} ms frame {frame}"
                );
            }
        }
    }
}

#[test]
fn glitch_at_full_chance_crossfades_each_repeat_into_its_own_start_without_live_input() {
    for slice_ms in [10.0, 500.0] {
        let slice = (slice_ms * 48.0) as usize;
        let heard = test_tone(slice * 6);
        let mut loud = heard.clone();
        for (frame, sample) in loud.iter_mut().enumerate().skip(slice + 96) {
            *sample = (frame as f32 * 0.9).sin() * 0.9;
        }
        let quiet_after = run(glitch(1.0, slice_ms, 1.0), &heard, SAMPLE_RATE);
        let loud_after = run(glitch(1.0, slice_ms, 1.0), &loud, SAMPLE_RATE);
        for frame in slice + 96..slice * 6 {
            assert_eq!(
                quiet_after[frame].to_bits(),
                loud_after[frame].to_bits(),
                "{slice_ms} ms frame {frame}"
            );
        }
        let largest_input_step = 0.6 * 0.013 + 0.2 * 0.071;
        let largest_step = quiet_after[slice + 96..]
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0, f32::max);
        assert!(
            largest_step < largest_input_step * 1.5,
            "{slice_ms} ms step {largest_step}"
        );
    }
}

#[test]
fn glitch_rolls_chance_once_per_slice() {
    let slice = 480;
    let input = test_tone(SAMPLE_RATE as usize * 10);
    let output = run(glitch(0.25, 10.0, 1.0), &input, SAMPLE_RATE);
    let repeated: Vec<bool> = output
        .chunks(slice)
        .zip(input.chunks(slice))
        .map(|(output, input)| {
            let middle = 96..slice - 96;
            let changed = middle
                .clone()
                .filter(|frame| output[*frame] != input[*frame]);
            let changed = changed.count();
            assert!(
                changed == 0 || changed == middle.len(),
                "a slice is all or nothing"
            );
            changed > 0
        })
        .collect();
    let share = repeated.iter().filter(|hit| **hit).count() as f32 / repeated.len() as f32;
    assert!((0.18..0.32).contains(&share), "repeated share {share}");
}

#[test]
fn glitch_fades_each_repeat_in_and_out() {
    let input = test_tone(SAMPLE_RATE as usize * 2);
    let output = run(glitch(0.5, 30.0, 1.0), &input, SAMPLE_RATE);
    let largest_input_step = 0.6 * 0.013 + 0.2 * 0.071;
    let largest_step = output
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .fold(0.0, f32::max);
    assert!(
        largest_step < largest_input_step + 1.6 / 96.0,
        "step {largest_step}"
    );
}
