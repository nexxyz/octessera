//! Every Play FX option, at its menu minimum, its default and its menu maximum, the others at
//! their defaults, pressed on the master over Init Synth stabs and let go again: the three
//! renders must differ, and none may be silent or saturated.

use super::*;
use crate::native_menu::{FactoryLoadKind, NativeMenuAction, PlatformAction};
use crate::native_runner::play_fx::config::{play_fx_param_specs, play_fx_types, PlayFxConfig};
use realtime_engine::synth::{
    prepare_momentary_fx_start_with_epoch, InstrumentSlotConfig, MomentaryFxTarget, SynthEngine,
};

const SAMPLE_RATE: u32 = 44_100;
const BPM: f64 = 120.0;
const STAB_FRAMES: usize = SAMPLE_RATE as usize / 4;
const PRESS_AT: usize = SAMPLE_RATE as usize / 4;
const RELEASE_AT: usize = SAMPLE_RATE as usize * 5 / 4;
const FRAMES: usize = SAMPLE_RATE as usize * 5 / 2;
const STABS: [u8; 4] = [57, 64, 69, 72];
/// Two settings sound different when their renders are at least 1% apart.
const AUDIBLE: f64 = 1.0e-2;
/// Settings the swept ones need to be heard: Pitch Shift at no shift has nothing to slide.
const CONTEXT: [(&str, &str, i64); 1] = [("pitch_shift", "semitones", 7)];

fn init_synth() -> InstrumentSlotConfig {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect(
            PlatformAction::FactoryInstrumentLoad {
                kind: FactoryLoadKind::SynthPreset,
                slot: 0,
                id: "init".into(),
            },
        ))
        .unwrap();
    serde_json::from_value(runner.instrument_audio_config(0).unwrap()).unwrap()
}

/// Interleaved stereo of the stabs with `fx_type` held from `PRESS_AT` to `RELEASE_AT`.
fn render(synth: &InstrumentSlotConfig, fx_type: &str, key: &str, value: i64) -> Vec<f32> {
    let config = PlayFxConfig::sanitized(fx_type, "master", |param| {
        if param == key {
            return Some(value);
        }
        CONTEXT
            .iter()
            .find(|(context_type, context_key, _)| {
                *context_type == fx_type && *context_key == param
            })
            .map(|(_, _, value)| *value)
    });
    let pressed = prepare_momentary_fx_start_with_epoch(
        "sweep".into(),
        1,
        fx_type.into(),
        config.audio_params(BPM),
        MomentaryFxTarget::Global,
        SAMPLE_RATE,
    )
    .unwrap_or_else(|| panic!("{fx_type}.{key} = {value}: the engine refused the press"));
    let mut pressed = Some(pressed);
    let mut released = false;
    let mut stabs = 0;
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    engine.set_instrument_slot(0, synth.clone());
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut rendered = Vec::with_capacity(FRAMES * 2);
    while rendered.len() < FRAMES * 2 {
        let frame = rendered.len() / 2;
        if frame >= stabs * STAB_FRAMES {
            engine.note_on(0, STABS[stabs % STABS.len()], 100, 150);
            stabs += 1;
        }
        if frame >= PRESS_AT {
            if let Some(pressed) = pressed.take() {
                let _ = engine.apply_prepared_momentary_fx_start(pressed);
            }
        }
        if frame >= RELEASE_AT && !released {
            let _ = engine.momentary_fx_stop_by_epoch(1);
            released = true;
        }
        engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
        rendered.extend_from_slice(&out);
    }
    rendered
}

/// How far apart two renders are, relative to the louder one.
fn difference(a: &[f32], b: &[f32]) -> f64 {
    let energy = |samples: &[f32]| samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
    let apart = a
        .iter()
        .zip(b)
        .map(|(a, b)| f64::from(a - b).powi(2))
        .sum::<f64>();
    (apart / energy(a).max(energy(b)).max(1.0e-12)).sqrt()
}

#[test]
fn every_play_fx_option_changes_the_sound_without_silencing_or_clipping_it() {
    let synth = init_synth();
    let mut findings = Vec::new();
    for fx_type in play_fx_types().filter(|fx_type| *fx_type != "none") {
        for spec in play_fx_param_specs(fx_type) {
            let values = [
                ("min", spec.min),
                ("default", spec.default),
                ("max", spec.max),
            ];
            let renders = values.map(|(name, value)| {
                (
                    name,
                    value,
                    render(&synth, fx_type, spec.key, i64::from(value)),
                )
            });
            for (name, value, samples) in &renders {
                let label = format!("{fx_type}.{} {name} = {value}", spec.key);
                if samples.iter().any(|sample| !sample.is_finite()) {
                    findings.push(format!("{label}: non-finite output"));
                    continue;
                }
                let peak = samples.iter().fold(0.0_f32, |peak, s| peak.max(s.abs()));
                if peak < 1.0e-4 {
                    findings.push(format!("{label}: silent"));
                }
                if peak >= 0.999 {
                    findings.push(format!("{label}: saturated at {peak}"));
                }
            }
            for (low, high) in [(0, 1), (1, 2), (0, 2)] {
                let ((low_name, low_value, low_samples), (high_name, high_value, high_samples)) =
                    (&renders[low], &renders[high]);
                let apart = difference(low_samples, high_samples);
                if low_value != high_value && apart < AUDIBLE {
                    findings.push(format!(
                        "{fx_type}.{}: {low_name} {low_value} and {high_name} {high_value} sound the same ({apart:.1e})",
                        spec.key
                    ));
                }
            }
        }
    }
    assert!(findings.is_empty(), "{}", findings.join("\n"));
}
