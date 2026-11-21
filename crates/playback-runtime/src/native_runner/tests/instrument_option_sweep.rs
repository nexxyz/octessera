//! Every numeric instrument option, at its menu minimum, its default and its menu maximum, on
//! one fixed note: the three renders must differ, and none may be silent or saturated, unless
//! the option is listed below with the reason it legitimately is.

use super::*;
use crate::native_menu::{FactoryLoadKind, NativeMenuAction, PlatformAction};
use realtime_engine::synth::{InstrumentSlotConfig, SynthEngine};
use serde_json::Value;

const SAMPLE_RATE: u32 = 44_100;
const NOTE: u8 = 69;
const NOTE_MS: u32 = 400;
const FRAMES: usize = SAMPLE_RATE as usize * 3 / 2;
/// Two settings sound different when their renders are at least 1% apart. A nearly dead range
/// still moves a render by a fraction of a percent: the old noise Tone moved hats by 0.1-0.3%
/// from 90% to 100%.
const AUDIBLE: f64 = 1.0e-2;

/// Options whose extremes are silent on purpose: no level, no sound. A Plucked Amp Env with no
/// Decay drops straight to its Sustain, so Sustain 0 is silent too.
const SILENT_BY_DESIGN: &[&str] = &[
    "synth amp.gainPct min",
    "fm amp.gainPct min",
    "pluck amp.gainPct min",
    "pluck ampEnv.sustainPct min",
    "drum amp.gainPct min",
    "drum voices.0.levelPct min",
    "drum voices.1.levelPct min",
    "drum voices.2.levelPct min",
    "drum voices.3.levelPct min",
    "drum voices.4.levelPct min",
    "drum voices.5.levelPct min",
    "drum voices.6.levelPct min",
    "drum voices.7.levelPct min",
];

/// The menu range of a numeric option, as the menu clamps it, in the units the config stores.
fn option_range(family: &str, field: &str) -> Option<(f64, f64)> {
    let (min, max) = match family {
        "synth" => synth_config::synth_numeric_field(&format!("synth.{field}"))
            .map(|(_, min, max)| (min, max)),
        "fm" => {
            menu_apply::fast_instruments::fm::numeric_field(field).map(|(_, min, max)| (min, max))
        }
        "pluck" => menu_apply::fast_instruments::pluck::numeric_field(field)
            .map(|(_, min, max)| (min, max)),
        "drum" => drum_config::voice_numeric_field(&format!("drum.{field}"))
            .map(|(_, _, min, max)| (min, max))
            .or_else(|| {
                drum_config::common_numeric_field(&format!("drum.{field}"))
                    .map(|(_, min, max)| (min, max))
            }),
        _ => None,
    }?;
    Some(if field.ends_with("filter.cutoffHz") {
        (
            f64::from(cutoff_display_to_hz(min)),
            f64::from(cutoff_display_to_hz(max)),
        )
    } else {
        (f64::from(min), f64::from(max))
    })
}

/// Every numeric leaf of a config, as a dotted path.
fn numeric_leaves(value: &Value, prefix: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                numeric_leaves(child, &join(prefix, key), out);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                numeric_leaves(child, &join(prefix, &index.to_string()), out);
            }
        }
        Value::Number(_) => out.push(prefix.into()),
        _ => {}
    }
}

fn join(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.into()
    } else {
        format!("{prefix}.{key}")
    }
}

fn leaf_mut<'a>(value: &'a mut Value, path: &str) -> &'a mut Value {
    path.split('.')
        .fold(value, |node, key| match key.parse::<usize>() {
            Ok(index) if node.is_array() => &mut node[index],
            _ => &mut node[key],
        })
}

fn set(value: &mut Value, path: &str, number: f64) {
    *leaf_mut(value, path) = serde_json::json!(number.round() as i64);
}

/// The Init sound of a family, with the options other options depend on switched on: a pulse
/// wave for Pulse Width, some Filter Env Amount for the filter envelope, a sustain below full
/// for Decay, some body for Body Freq and a pitch sweep for Sweep Ms.
fn sweep_base(family: &str) -> Value {
    let (kind, id) = match family {
        "synth" => (FactoryLoadKind::SynthPreset, "init"),
        "fm" => (FactoryLoadKind::FmPreset, "init"),
        "pluck" => (FactoryLoadKind::PluckPreset, "init"),
        _ => (FactoryLoadKind::DrumKit, "default"),
    };
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect(
            PlatformAction::FactoryInstrumentLoad {
                kind,
                slot: 0,
                id: id.into(),
            },
        ))
        .unwrap();
    let mut slot = runner.instrument_audio_config(0).unwrap();
    let config = &mut slot[family];
    set(config, "filter.cutoffHz", 2_000.0);
    set(config, "filter.envAmountPct", 50.0);
    match family {
        "synth" => {
            config["osc1"]["waveform"] = "pulse".into();
            config["osc2"]["waveform"] = "pulse".into();
        }
        "pluck" => {
            set(config, "ampEnv.sustainPct", 70.0);
            set(config, "bodyAmountPct", 50.0);
        }
        "drum" => {
            for voice in config["voices"].as_array_mut().unwrap() {
                voice["sweepSemis"] = 12.into();
                voice["sweepMs"] = 40.into();
            }
        }
        _ => {}
    }
    slot
}

fn render(slot: &Value, voice: Option<u8>) -> Vec<f32> {
    let config: InstrumentSlotConfig = serde_json::from_value(slot.clone()).unwrap();
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    engine.set_instrument_slot(0, config);
    match voice {
        Some(voice) => engine.tile_hit(0, voice, voice, 0, 110),
        None => engine.note_on(0, NOTE, 110, NOTE_MS),
    }
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut rendered = Vec::with_capacity(FRAMES * 2);
    while rendered.len() < FRAMES * 2 {
        engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
        rendered.extend_from_slice(&out);
    }
    rendered
}

fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
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

fn sweep_family(family: &str, findings: &mut Vec<String>) {
    let base = sweep_base(family);
    let mut leaves = Vec::new();
    numeric_leaves(&base[family], "", &mut leaves);
    for field in leaves {
        let Some((min, max)) = option_range(family, &field) else {
            continue;
        };
        let voice = field
            .strip_prefix("voices.")
            .and_then(|rest| rest.split('.').next())
            .map(|voice| voice.parse::<u8>().unwrap());
        let voice = if family == "drum" {
            voice.or(Some(0))
        } else {
            None
        };
        let default = leaf_mut(&mut base[family].clone(), &field)
            .as_f64()
            .unwrap();
        let renders = [("min", min), ("default", default), ("max", max)].map(|(name, value)| {
            let mut slot = base.clone();
            set(&mut slot[family], &field, value);
            (name, value, render(&slot, voice))
        });
        for (name, value, samples) in &renders {
            let label = format!("{family} {field} {name}");
            let loudest = peak(samples);
            assert!(loudest.is_finite(), "{label} = {value}: non-finite output");
            if loudest < 1.0e-4 && !SILENT_BY_DESIGN.contains(&label.as_str()) {
                findings.push(format!("{label} = {value}: silent"));
            }
            if loudest >= 0.999 {
                findings.push(format!("{label} = {value}: saturated at {loudest}"));
            }
        }
        for (low, high) in [(0, 1), (1, 2), (0, 2)] {
            let ((low_name, low_value, low_samples), (high_name, high_value, high_samples)) =
                (&renders[low], &renders[high]);
            let apart = difference(low_samples, high_samples);
            if low_value != high_value && apart < AUDIBLE {
                findings.push(format!(
                    "{family} {field}: {low_name} {low_value} and {high_name} {high_value} sound the same ({apart:.1e})"
                ));
            }
        }
    }
}

#[test]
fn every_instrument_option_changes_the_sound_without_silencing_or_clipping_it() {
    let mut findings = Vec::new();
    for family in ["synth", "fm", "pluck", "drum"] {
        sweep_family(family, &mut findings);
    }
    assert!(findings.is_empty(), "{}", findings.join("\n"));
}
