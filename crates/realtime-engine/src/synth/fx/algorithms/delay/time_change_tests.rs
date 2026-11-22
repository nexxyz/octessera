use super::*;
use crate::synth::fx::{
    fx_bus_state_can_preserve, fx_bus_state_from_params, land_loaded_delay_time,
};

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

#[test]
fn a_load_lands_a_kept_line_on_its_time_at_once_and_keeps_the_echoes() {
    let mut kept = fx_bus_state_from_params(&delay(100.0, 0.0), SAMPLE_RATE);
    let mut output = vec![process_delay(&mut kept, 1.0, 100.0, 0.0, 1.0, SAMPLE_RATE)];
    for _ in 1..2_400 {
        output.push(process_delay(&mut kept, 0.0, 100.0, 0.0, 1.0, SAMPLE_RATE));
    }
    land_loaded_delay_time(
        &mut kept,
        &fx_bus_state_from_params(&delay(300.0, 0.0), SAMPLE_RATE),
    );
    for _ in 2_400..16_000 {
        output.push(process_delay(&mut kept, 0.0, 300.0, 0.0, 1.0, SAMPLE_RATE));
    }
    assert!(output[14_400] > 0.99, "the echo lands 300 ms on");
    let elsewhere = output[2_400..]
        .iter()
        .enumerate()
        .filter(|(frame, _)| *frame + 2_400 != 14_400)
        .fold(0.0_f32, |peak, (_, sample)| peak.max(sample.abs()));
    assert!(elsewhere < 0.01, "no glide smears it: {elsewhere}");
}

#[test]
fn a_time_change_glides_all_the_way_onto_the_new_time() {
    for (from_ms, to_ms) in [(100.0, 250.0), (100.0, 6_000.0), (6_000.0, 5_000.0)] {
        let mut state = fx_bus_state_from_params(&delay(from_ms, 0.0), SAMPLE_RATE);
        for _ in 0..SAMPLE_RATE * 2 {
            process_delay(&mut state, 0.0, to_ms, 0.0, 1.0, SAMPLE_RATE);
        }
        let FxBusState::Delay { cache, .. } = &state else {
            panic!("a delay line");
        };
        let target = to_ms / 1_000.0 * SAMPLE_RATE as f32;
        assert_eq!(cache.current_samples(), target, "{from_ms} to {to_ms} ms");
    }
}
