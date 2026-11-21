//! Every bus and Global FX number parameter at its minimum, default and maximum, the others at
//! their defaults, on one fixed input: each setting must sound, stay below full scale, and
//! sound different from the other two.
use crate::synth::fx::{
    fx_bus_state_from_params, master_fx_state_from_params, process_fx_bus_slot,
    process_master_fx_slot, FxBusState,
};
use crate::synth::fx_params::compile_fx_bus_params;
use crate::synth::{fx_param_saved_range, FxBusSlotConfig, FxParamId};
use serde_json::json;
use std::collections::BTreeMap;
use std::f32::consts::TAU;

const SAMPLE_RATE: u32 = 48_000;
const FRAMES: usize = SAMPLE_RATE as usize * 2;
const BUS_TYPES: [&str; 17] = [
    "tremolo",
    "delay",
    "vibrato",
    "chorus",
    "flanger",
    "filter_lfo",
    "wah",
    "reverb",
    "glitch",
    "auto_pan",
    "duck",
    "saturator",
    "distortion",
    "bitcrusher",
    "compressor",
    "eq",
    "vinyl",
];
const GLOBAL_TYPES: [&str; 6] = [
    "vinyl",
    "eq",
    "compressor",
    "saturator",
    "distortion",
    "bitcrusher",
];
const COMPILED_KEYS: [&str; 3] = ["timeMs", "depthMs", "baseMs"];
/// Settings the swept ones need to be heard: a flat EQ ignores its Mid Freq, Mid Q and Mix.
const CONTEXT: [(&str, &str, f32); 1] = [("eq", "midGainDb", 9.0)];
/// Findings accepted on purpose, each with the reason.
const EXCUSED: [(&str, &str, &str); 2] = [
    (
        "delay",
        "spreadPct",
        "Spread widens the bus output after the slot; spread_balance_tests cover it",
    ),
    (
        "compressor",
        "makeupDb",
        "24 dB of makeup is for heavy squeezing; at the default -24 dB threshold it is too loud",
    ),
];

/// Chord stabs, one every 250 ms, decaying to silence by the last half second.
fn input(frame: usize) -> f32 {
    let t = frame as f32 / SAMPLE_RATE as f32;
    if t >= 1.5 {
        return 0.0;
    }
    let envelope = (-(t % 0.25) * 12.0).exp();
    let chord = [110.0, 277.0, 660.0, 2_300.0]
        .iter()
        .map(|hz| (TAU * hz * t).sin())
        .sum::<f32>();
    chord * envelope * 0.12
}

/// A kick-like pulse twice a second for Duck to listen to.
fn duck_source(frame: usize) -> f32 {
    let t = frame as f32 / SAMPLE_RATE as f32;
    (-(t % 0.5) * 20.0).exp() * 0.8
}

fn keys(kind: &str) -> Vec<String> {
    FxParamId::ALL
        .iter()
        .map(|id| {
            serde_json::to_value(id)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        })
        .chain(COMPILED_KEYS.iter().map(|key| key.to_string()))
        .filter(|key| fx_param_saved_range(kind, key).is_some())
        .collect()
}

fn config(kind: &str, key: &str, value: f32) -> FxBusSlotConfig {
    let mut params: BTreeMap<String, serde_json::Value> = CONTEXT
        .iter()
        .filter(|(context_kind, _, _)| *context_kind == kind)
        .map(|(_, context_key, value)| (context_key.to_string(), json!(value)))
        .collect();
    params.insert(key.into(), json!(value));
    FxBusSlotConfig::Config {
        kind: kind.into(),
        params,
    }
}

/// The mono bus output; Auto Pan reports its pan position instead, since it moves only that.
fn render_bus(kind: &str, key: &str, value: f32) -> Vec<f32> {
    let params = compile_fx_bus_params(&config(kind, key, value));
    let mut state = fx_bus_state_from_params(&params, SAMPLE_RATE);
    (0..FRAMES)
        .map(|frame| {
            let x = input(frame);
            let y = process_fx_bus_slot(&params, &mut state, x, duck_source(frame), SAMPLE_RATE);
            match state {
                FxBusState::AutoPan { pos, .. } => x * pos,
                _ => y,
            }
        })
        .collect()
}

fn render_global(kind: &str, key: &str, value: f32) -> Vec<f32> {
    let params = compile_fx_bus_params(&config(kind, key, value));
    let mut state = master_fx_state_from_params(&params);
    (0..FRAMES)
        .flat_map(|frame| {
            let x = input(frame);
            let (l, r) = process_master_fx_slot(&params, &mut state, x, -x * 0.5, SAMPLE_RATE);
            [l, r]
        })
        .collect()
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
}

/// What is wrong with one rendered setting, if anything.
fn fault(output: &[f32]) -> Option<String> {
    if output.iter().any(|sample| !sample.is_finite()) {
        return Some("non-finite".into());
    }
    let level = rms(output);
    if level < 1.0e-3 {
        return Some(format!("silent (rms {level:.5})"));
    }
    let over = output.iter().filter(|sample| sample.abs() >= 1.0).count();
    if over * 100 > output.len() {
        return Some(format!("saturated ({over} samples at full scale)"));
    }
    None
}

/// The level of the difference between two renders against the first, in dB.
fn difference_db(a: &[f32], b: &[f32]) -> f32 {
    let diff: Vec<f32> = a.iter().zip(b).map(|(a, b)| a - b).collect();
    20.0 * (rms(&diff).max(1.0e-12) / rms(a).max(1.0e-12)).log10()
}

fn sweep(kind: &str, render: fn(&str, &str, f32) -> Vec<f32>, findings: &mut Vec<String>) {
    for key in keys(kind) {
        let (min, max, default) = fx_param_saved_range(kind, &key).unwrap();
        let mut values = vec![min, default, max];
        values.dedup();
        let renders: Vec<Vec<f32>> = values
            .iter()
            .map(|value| render(kind, &key, *value))
            .collect();
        if EXCUSED
            .iter()
            .any(|(excused_kind, excused_key, _)| *excused_kind == kind && *excused_key == key)
        {
            continue;
        }
        for (value, output) in values.iter().zip(&renders) {
            if let Some(fault) = fault(output) {
                findings.push(format!("{kind}.{key} = {value}: {fault}"));
            }
        }
        for pair in [(0, 1), (1, 2), (0, 2)] {
            let (Some(a), Some(b)) = (renders.get(pair.0), renders.get(pair.1)) else {
                continue;
            };
            let db = difference_db(a, b);
            if db < -60.0 {
                findings.push(format!(
                    "{kind}.{key}: {} and {} sound the same ({db:.1} dB)",
                    values[pair.0], values[pair.1]
                ));
            }
        }
    }
}

#[test]
fn every_fx_option_sounds_stays_below_full_scale_and_changes_the_sound() {
    let mut findings = Vec::new();
    for kind in BUS_TYPES {
        sweep(kind, render_bus, &mut findings);
    }
    for kind in GLOBAL_TYPES {
        sweep(kind, render_global, &mut findings);
    }
    assert!(
        findings.is_empty(),
        "{}",
        findings.join(
            "
"
        )
    );
}
