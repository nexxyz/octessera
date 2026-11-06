use super::super::render_momentary_fx::{
    apply_prepared_momentary_fx_update, process_momentary_fx_states,
};
use super::super::retired_state::PREVIEW_AUDITION_SLOTS;
use super::super::{parse_momentary_fx_kind, MomentaryFxState, PreparedMomentaryFxUpdate};
use super::{prepare_voice_update, MomentaryFxVoice};
use crate::synth::MomentaryFxTarget;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::f32::consts::TAU;

const RATE: u32 = 48_000;
const FRAMES: usize = RATE as usize;
const TARGET: MomentaryFxTarget = MomentaryFxTarget::Global;

type Retired = [Option<MomentaryFxState>; PREVIEW_AUDITION_SLOTS];

fn params(pairs: &[(&str, f64)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), json!(value)))
        .collect()
}

fn state(fx_type: &str, params: &BTreeMap<String, Value>) -> Vec<MomentaryFxState> {
    let kind = parse_momentary_fx_kind(fx_type).expect("known type");
    let mut states = Vec::with_capacity(2);
    states.push(MomentaryFxState::new(
        fx_type.into(),
        kind,
        params,
        TARGET,
        RATE,
    ));
    states
}

fn sine(frame: usize, hz: f32) -> f32 {
    0.5 * (TAU * hz * frame as f32 / RATE as f32).sin()
}

/// Every new type with musically typical settings.
fn all_types() -> Vec<(&'static str, BTreeMap<String, Value>)> {
    vec![
        (
            "tape_stop",
            params(&[("stopMs", 300.0), ("startMs", 200.0)]),
        ),
        ("reverse", params(&[("lengthMs", 120.0), ("mixPct", 100.0)])),
        (
            "trance_gate",
            params(&[("ratePulses", 6.0), ("depthPct", 100.0), ("smoothPct", 0.0)]),
        ),
        (
            "dub_throw",
            params(&[("timePulses", 6.0), ("feedbackPct", 50.0), ("mixPct", 80.0)]),
        ),
        (
            "wash",
            params(&[("sizePct", 80.0), ("mixPct", 70.0), ("releaseMs", 300.0)]),
        ),
        (
            "hp_riser",
            params(&[
                ("cutoffPct", 60.0),
                ("resonancePct", 40.0),
                ("sweepInMs", 200.0),
            ]),
        ),
        (
            "crush",
            params(&[("bits", 4.0), ("rateDiv", 1.0), ("mixPct", 100.0)]),
        ),
    ]
}

struct Run {
    output: Vec<f32>,
    removed_at: usize,
}

/// Holds the effect for `hold` frames of `input`, releases it, and keeps feeding `input` until
/// the effect removes itself.
fn run(states: &mut Vec<MomentaryFxState>, hold: usize, input: impl Fn(usize) -> f32) -> Run {
    let mut retired: Retired = std::array::from_fn(|_| None);
    let mut output = Vec::new();
    let mut frame = 0;
    while !states.is_empty() {
        assert!(frame < hold + 30 * FRAMES, "effect never ended");
        if frame == hold {
            states[0].begin_release();
        }
        let x = input(frame);
        let (left, _) = process_momentary_fx_states(states, TARGET, x, x, Some(&mut retired), RATE);
        output.push(left);
        frame += 1;
    }
    Run {
        output,
        removed_at: frame,
    }
}

fn max_step(samples: &[f32]) -> f32 {
    samples
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .fold(0.0, f32::max)
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
}

fn zero_crossings(samples: &[f32]) -> usize {
    samples
        .windows(2)
        .filter(|pair| (pair[0] < 0.0) != (pair[1] < 0.0))
        .count()
}

#[test]
fn press_release_and_removal_steps_stay_bounded_for_every_type() {
    let hold = FRAMES;
    let transition = FRAMES / 50;
    let dry = (0..FRAMES).map(|n| sine(n, 220.0)).collect::<Vec<_>>();
    let dry_step = max_step(&dry);
    for (fx_type, params) in all_types() {
        let mut states = state(fx_type, &params);
        let run = run(&mut states, hold, |n| sine(n, 220.0));
        let steady = max_step(&run.output[hold / 2..hold]).max(dry_step);
        let bound = steady * 1.5 + 1.0e-3;
        let press = max_step(&run.output[..transition]);
        let release_end = (hold + transition).min(run.output.len());
        let release = max_step(&run.output[hold - 1..release_end]);
        let mut removal = run.output[run.removed_at - 64..].to_vec();
        removal.extend((run.removed_at..run.removed_at + 64).map(|n| sine(n, 220.0)));
        let removal = max_step(&removal);
        for (moment, step) in [("press", press), ("release", release), ("removal", removal)] {
            assert!(
                step <= bound,
                "{fx_type} {moment} step {step} exceeds {bound} (steady {steady})"
            );
        }
    }
}

#[test]
fn every_type_leaves_the_signal_untouched_after_removal() {
    for (fx_type, params) in all_types() {
        let mut states = state(fx_type, &params);
        let _ = run(&mut states, FRAMES / 4, |n| sine(n, 330.0));
        for frame in 0..256 {
            let x = sine(frame, 330.0);
            let (left, right) = process_momentary_fx_states(&mut states, TARGET, x, -x, None, RATE);
            assert_eq!(left.to_bits(), x.to_bits(), "{fx_type}");
            assert_eq!(right.to_bits(), (-x).to_bits(), "{fx_type}");
        }
    }
}

#[test]
fn processing_release_and_removal_allocate_nothing() {
    for (fx_type, params) in all_types() {
        let mut states = state(fx_type, &params);
        let mut retired: Retired = std::array::from_fn(|_| None);
        let ((), allocations, deallocations) =
            crate::synth::test_allocator::count_allocations_and_deallocations(|| {
                for frame in 0..3 * FRAMES {
                    if frame == FRAMES {
                        states[0].begin_release();
                    }
                    let x = sine(frame, 440.0);
                    let _ = process_momentary_fx_states(
                        &mut states,
                        TARGET,
                        x,
                        x,
                        Some(&mut retired),
                        RATE,
                    );
                }
            });
        assert!(states.is_empty(), "{fx_type} still running");
        assert_eq!((allocations, deallocations), (0, 0), "{fx_type}");
    }
}

#[test]
fn tape_stop_pitch_falls_to_silence_and_spins_back_up() {
    let mut states = state(
        "tape_stop",
        &params(&[("stopMs", 500.0), ("startMs", 500.0)]),
    );
    let hold = FRAMES;
    let run = run(&mut states, hold, |n| sine(n, 1_000.0));
    let window = FRAMES / 10;
    let falling = (0..4)
        .map(|index| zero_crossings(&run.output[index * window..(index + 1) * window]))
        .collect::<Vec<_>>();
    assert!(
        falling.windows(2).all(|pair| pair[1] < pair[0]),
        "pitch must fall while stopping: {falling:?}"
    );
    assert!(
        run.output[hold - window..hold]
            .iter()
            .all(|x| x.abs() < 1.0e-6),
        "a stopped tape is silent"
    );
    let rising = (1..5)
        .map(|index| {
            zero_crossings(&run.output[hold + index * window..hold + (index + 1) * window])
        })
        .collect::<Vec<_>>();
    assert!(
        rising.windows(2).all(|pair| pair[1] > pair[0]),
        "pitch must rise after release: {rising:?}"
    );
}

#[test]
fn reverse_plays_each_window_backwards_after_capturing_one() {
    let length = FRAMES / 10;
    let fade = FRAMES / 200;
    let mut states = state(
        "reverse",
        &params(&[("lengthMs", 100.0), ("mixPct", 100.0)]),
    );
    let ramp = |n: usize| n as f32 * 1.0e-5;
    let run = run(&mut states, 4 * length, ramp);
    for (frame, value) in run.output[..length - 1].iter().enumerate() {
        assert_eq!(*value, ramp(frame), "dry while capturing");
    }
    let first = &run.output[length + 2 * fade..2 * length - 2 * fade];
    assert!(
        first.windows(2).all(|pair| pair[1] < pair[0]),
        "a rising input must play back falling"
    );
    let expected = ramp(length - 1 - 2 * fade);
    assert!(
        (first[0] - expected).abs() < 1.0e-4,
        "{} vs {expected}",
        first[0]
    );
}

#[test]
fn trance_gate_period_follows_the_tempo() {
    for (bpm, pulses, expected) in [
        (120.0, 6.0, 6_000.0),
        (90.0, 12.0, 16_000.0),
        (150.0, 4.0, 3_200.0),
    ] {
        let mut states = state(
            "trance_gate",
            &params(&[
                ("ratePulses", pulses),
                ("depthPct", 100.0),
                ("tempoBpm", bpm),
            ]),
        );
        let run = run(&mut states, 5 * expected as usize, |_| 0.5);
        let falls = run
            .output
            .windows(2)
            .enumerate()
            .filter(|(_, pair)| pair[0] >= 0.25 && pair[1] < 0.25)
            .map(|(frame, _)| frame)
            .collect::<Vec<_>>();
        assert!(falls.len() >= 4, "{bpm} bpm gate edges {falls:?}");
        for pair in falls.windows(2).take(3) {
            let period = (pair[1] - pair[0]) as f32;
            assert!(
                (period - expected).abs() <= 1.0,
                "{bpm} bpm period {period}"
            );
        }
    }
}

#[test]
fn trance_gate_tempo_update_keeps_its_place_in_the_period() {
    let mut states = state(
        "trance_gate",
        &params(&[("ratePulses", 6.0), ("tempoBpm", 120.0)]),
    );
    for _ in 0..1_500 {
        let _ = process_momentary_fx_states(&mut states, TARGET, 0.5, 0.5, None, RATE);
    }
    let update = prepare_voice_update(
        states[0].kind,
        0,
        &params(&[("ratePulses", 6.0), ("tempoBpm", 60.0)]),
        RATE,
    )
    .expect("gate update");
    let PreparedMomentaryFxUpdate::TranceGate { period, .. } = update else {
        panic!("gate update kind");
    };
    assert_eq!(period, 12_000.0);
    let _ = apply_prepared_momentary_fx_update(&mut states[0], update);
    let MomentaryFxVoice::TranceGate(voice) = &states[0].voice else {
        panic!("gate voice");
    };
    assert_eq!(voice.phase, 3_000.0);
}

#[test]
fn dub_throw_echoes_keep_trailing_after_release_then_end() {
    let mut states = state(
        "dub_throw",
        &params(&[
            ("timePulses", 12.0),
            ("feedbackPct", 50.0),
            ("mixPct", 100.0),
            ("tempoBpm", 120.0),
        ]),
    );
    let delay = 12_000;
    let burst = |n: usize| if n < 2_000 { sine(n, 500.0) } else { 0.0 };
    let run = run(&mut states, 4_000, burst);
    let echo = |index: usize| {
        run.output[index * delay..index * delay + 2_100]
            .iter()
            .map(|x| x.abs())
            .fold(0.0, f32::max)
    };
    assert!(echo(1) > 0.2, "first echo after release {}", echo(1));
    assert!(
        echo(2) < echo(1) && echo(2) > 0.05,
        "second echo decays: {} then {}",
        echo(1),
        echo(2)
    );
    assert!(run.removed_at > 4 * delay, "tail outlives the hold");
    assert!(run.output[run.removed_at - 64..]
        .iter()
        .all(|x| x.abs() < 1.0e-4));
}

#[test]
fn wash_tail_decays_over_release_then_ends() {
    let release = FRAMES;
    let hold = FRAMES / 2;
    let mut states = state(
        "wash",
        &params(&[("sizePct", 90.0), ("mixPct", 100.0), ("releaseMs", 1_000.0)]),
    );
    let input = |n: usize| {
        if n < hold {
            sine(n, 300.0) + sine(n, 1_270.0) * 0.5
        } else {
            0.0
        }
    };
    let run = run(&mut states, hold, input);
    let quarter = (run.removed_at - hold) / 4;
    let levels = (0..4)
        .map(|index| rms(&run.output[hold + index * quarter..hold + (index + 1) * quarter]))
        .collect::<Vec<_>>();
    assert!(
        levels[0] > 0.02,
        "wash keeps a tail after release: {levels:?}"
    );
    assert!(
        levels.windows(2).all(|pair| pair[1] < pair[0]),
        "tail decays: {levels:?}"
    );
    assert!(run.removed_at <= hold + release + 1, "ends with Release");
}

#[test]
fn hp_riser_removes_lows_and_keeps_highs_at_its_target() {
    let riser = params(&[
        ("cutoffPct", 60.0),
        ("resonancePct", 0.0),
        ("sweepInMs", 100.0),
    ]);
    let level = |hz: f32| {
        let mut states = state("hp_riser", &riser);
        let run = run(&mut states, FRAMES / 2, |n| sine(n, hz));
        let dry = (0..FRAMES / 4).map(|n| sine(n, hz)).collect::<Vec<_>>();
        rms(&run.output[FRAMES / 4..FRAMES / 2]) / rms(&dry)
    };
    assert!(level(100.0) < 0.05, "lows cut {}", level(100.0));
    assert!(level(8_000.0) > 0.9, "highs pass {}", level(8_000.0));
}

#[test]
fn crush_quantizes_to_its_bit_depth_and_holds_samples() {
    let mut states = state(
        "crush",
        &params(&[("bits", 4.0), ("rateDiv", 3.0), ("mixPct", 100.0)]),
    );
    let run = run(&mut states, FRAMES / 4, |n| sine(n, 220.0));
    let steady = &run.output[FRAMES / 8..FRAMES / 4];
    for value in steady {
        let steps = value * 8.0;
        assert!(
            (steps - steps.round()).abs() < 1.0e-4,
            "{value} is not on the 4-bit grid"
        );
    }
    assert!(
        steady
            .windows(3)
            .all(|frames| frames[0] == frames[1] || frames[1] == frames[2]),
        "downsampling holds each sample"
    );
}
