use super::*;
use crate::synth::fx::{fx_bus_state_can_preserve, fx_bus_state_from_params};

const SAMPLE_RATE: u32 = 48_000;

fn delay(time_ms: f32, feedback: f32) -> FxBusParams {
    FxBusParams::Delay {
        time_ms,
        feedback,
        mix: 1.0,
        spread: 0.0,
    }
}

/// Runs a 100 ms delay, then `next_ms` from `switch` on, on the line it already had.
fn retimed(input: &[f32], next_ms: f32, feedback: f32, switch: usize) -> Vec<f32> {
    let mut state = fx_bus_state_from_params(&delay(100.0, feedback), SAMPLE_RATE);
    assert!(fx_bus_state_can_preserve(
        &state,
        &delay(next_ms, feedback),
        &fx_bus_state_from_params(&delay(next_ms, feedback), SAMPLE_RATE),
    ));
    input
        .iter()
        .enumerate()
        .map(|(frame, sample)| {
            let time_ms = if frame < switch { 100.0 } else { next_ms };
            process_delay(&mut state, *sample, time_ms, feedback, 1.0, SAMPLE_RATE)
        })
        .collect()
}

#[test]
fn a_longer_time_keeps_the_echoes_already_in_the_line() {
    let mut impulse = vec![0.0; SAMPLE_RATE as usize * 2];
    impulse[0] = 1.0;
    let output = retimed(&impulse, 1_000.0, 0.5, 7_200);
    let after = &output[SAMPLE_RATE as usize..];
    assert!(after.iter().any(|sample| sample.abs() > 0.05));
}

#[test]
fn a_time_change_either_way_glides_without_a_click() {
    let tone: Vec<f32> = (0..SAMPLE_RATE as usize)
        .map(|frame| (frame as f32 * std::f32::consts::TAU * 220.0 / 48_000.0).sin() * 0.5)
        .collect();
    for next_ms in [30.0, 400.0] {
        let output = retimed(&tone, next_ms, 0.0, 12_000);
        let largest_step = output[4_800..]
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0, f32::max);
        assert!(largest_step < 0.25, "{next_ms} ms: step {largest_step}");
    }
}
