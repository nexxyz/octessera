//! Every numeric instrument option, at its menu minimum, its default and its menu maximum, on
//! one fixed note: the three renders must differ, and none may be silent or saturated, unless
//! the option is listed below with the reason it legitimately is.
//!
//! The Sampler holds a made-up sample in its first slot on a forward loop, and takes its ranges
//! from its menu rows.

use super::*;
use crate::native_menu::{FactoryLoadKind, NativeMenuAction, NativeMenuValue, PlatformAction};
use realtime_engine::synth::{
    normalize_instrument_slot_config, InstrumentSlotConfig, SampleBuffer, SampleSlotConfig,
    SynthEngine, SAMPLE_SLOTS_PER_INSTRUMENT,
};
use serde_json::Value;

const SAMPLE_RATE: u32 = 44_100;
const NOTE: u8 = 69;
const NOTE_MS: u32 = 400;
const FRAMES: usize = SAMPLE_RATE as usize * 3 / 2;
/// Two settings sound different when their renders are at least 1% apart. A nearly dead range
/// still moves a render by a fraction of a percent: the old noise Tone moved hats by 0.1-0.3%
/// from 90% to 100%.
const AUDIBLE: f64 = 1.0e-2;

/// Options whose extremes are silent on purpose: no level, no sound.
const SILENT_BY_DESIGN: &[&str] = &[
    "synth amp.gainPct min",
    "fm amp.gainPct min",
    "pluck amp.gainPct min",
    "drum amp.gainPct min",
    "drum voices.0.levelPct min",
    "drum voices.1.levelPct min",
    "drum voices.2.levelPct min",
    "drum voices.3.levelPct min",
    "drum voices.4.levelPct min",
    "drum voices.5.levelPct min",
    "drum voices.6.levelPct min",
    "drum voices.7.levelPct min",
    "sample amp.gainPct min",
];

/// Numeric config values the sweep leaves alone. A Drum kit's envelopes have no menu rows, and
/// its velocity levels only pick the velocity of a stamped tile's level, which a plain hit does
/// not use. A Sampler's Base Vel and velocity levels likewise only pick the velocity the runtime
/// sends, and its Sample Slot only picks the slot its menu shows. Its other slots hold no sample.
const NOT_SWEPT: &[&str] = &[
    "sample selectedSlot",
    "sample baseVelocity",
    "sample velocityLevels.high",
    "sample velocityLevels.medium",
    "sample velocityLevels.low",
    "drum ampEnv.attackMs",
    "drum ampEnv.decayMs",
    "drum ampEnv.sustainPct",
    "drum ampEnv.releaseMs",
    "drum filterEnv.attackMs",
    "drum filterEnv.decayMs",
    "drum filterEnv.sustainPct",
    "drum filterEnv.releaseMs",
    "drum velocityLevels.high",
    "drum velocityLevels.medium",
    "drum velocityLevels.low",
];

/// The menu range of a numeric option, as the menu clamps it, in the units the config stores.
fn option_range(family: &str, field: &str, menu: &NativeMenuModel) -> Option<(f64, f64)> {
    let (min, max) = match family {
        "sample" => match menu
            .item_for_key(&format!("instruments.0.sample.{field}"))?
            .value
        {
            NativeMenuValue::Number { min, max, .. } => Some((min, max)),
            _ => None,
        },
        "synth" => synth_config::synth_numeric_field(&format!("synth.{field}"))
            .map(|(_, min, max)| (min, max))
            .or(match field {
                // The rows `menu_apply/instrument_synth.rs` clamps on its own.
                "amp.gainPct" | "filter.resonance" => Some((0, 100)),
                "filter.cutoffHz" => Some((0, 255)),
                _ => None,
            }),
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
/// for Decay, some body for Body Freq and a pitch sweep for Sweep Ms. The Sampler is the default
/// one in Note Mode hold with its first slot on a forward loop, for the loop rows. Also returns
/// the menu that shows the family's rows.
fn sweep_base(family: &str) -> (Value, NativeMenuModel) {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let factory = match family {
        "synth" => Some((FactoryLoadKind::SynthPreset, "init")),
        "fm" => Some((FactoryLoadKind::FmPreset, "init")),
        "pluck" => Some((FactoryLoadKind::PluckPreset, "init")),
        "drum" => Some((FactoryLoadKind::DrumKit, "default")),
        _ => None,
    };
    if let Some((kind, id)) = factory {
        runner
            .execute_confirmed_action(NativeMenuAction::PlatformEffect(
                PlatformAction::FactoryInstrumentLoad {
                    kind,
                    slot: 0,
                    id: id.into(),
                },
            ))
            .unwrap();
    } else {
        let sampler = &mut runner.instruments[0];
        sampler.kind = InstrumentType::Sampler;
        sampler.note_behavior = NoteBehavior::from_wire_name("hold").unwrap();
        sampler.sample_playback[0].loop_mode = "forward";
        runner.menu.rebuild(runner.menu_config());
    }
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
            set(config, "decayMs", 3_000.0);
            set(config, "bodyAmountPct", 50.0);
        }
        "drum" => {
            for voice in config["voices"].as_array_mut().unwrap() {
                voice["sweepSemis"] = 12.into();
                voice["sweepMs"] = 40.into();
            }
        }
        "sample" => {
            set(config, "tuneSemis", 7.0);
            set(config, "ampEnv.decayMs", 300.0);
            set(config, "ampEnv.sustainPct", 70.0);
            set(config, "ampEnv.releaseMs", 600.0);
        }
        _ => {}
    }
    (slot, runner.menu)
}

/// A quarter second of a fading two-partial tone, so Start and the loop points each land on a
/// different part of it.
fn held_sample() -> SampleBuffer {
    let frames = SAMPLE_RATE as usize / 4;
    SampleBuffer {
        samples: (0..frames)
            .map(|frame| {
                let t = frame as f32 / SAMPLE_RATE as f32;
                let tone = (std::f32::consts::TAU * 220.0 * t).sin()
                    + 0.5 * (std::f32::consts::TAU * 1_330.0 * t).sin();
                tone * 0.2 * (1.0 - 2.0 * t)
            })
            .collect::<Vec<_>>()
            .into(),
        channels: 1,
        sample_rate: SAMPLE_RATE,
    }
}

/// One note, or one tile hit for a Drum kit; the Sampler's first tile is held for the note's
/// length, then let go.
fn render(slot: &Value, voice: Option<u8>) -> Vec<f32> {
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    let note_frames = (SAMPLE_RATE * NOTE_MS / 1_000) as usize;
    let mut release_at = None;
    if slot["type"] == "sampler" {
        let normalized = normalize_instrument_slot_config(slot).unwrap();
        engine.set_instrument_slot(0, normalized.slot.clone());
        let mut slots = vec![SampleSlotConfig::default(); SAMPLE_SLOTS_PER_INSTRUMENT];
        slots[0].buffer = Some(held_sample());
        let _ = engine.set_sample_bank(0, normalized.active_sample().unwrap().bank(slots));
        engine.tile_hit(0, 0, 0, 0, 110);
        release_at = Some(note_frames * 2);
    } else {
        let config: InstrumentSlotConfig = serde_json::from_value(slot.clone()).unwrap();
        engine.set_instrument_slot(0, config);
        match voice {
            Some(voice) => engine.tile_hit(0, voice, voice, 0, 110),
            None => engine.note_on(0, NOTE, 110, NOTE_MS),
        }
    }
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut rendered = Vec::with_capacity(FRAMES * 2);
    while rendered.len() < FRAMES * 2 {
        if release_at.is_some_and(|at| rendered.len() >= at) {
            engine.tile_hit(0, 0, 0, 0, 0);
            release_at = None;
        }
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
    let (base, menu) = sweep_base(family);
    let mut leaves = Vec::new();
    numeric_leaves(&base[family], "", &mut leaves);
    for field in leaves {
        let unloaded_slot =
            family == "sample" && field.starts_with("slots.") && !field.starts_with("slots.0.");
        if unloaded_slot || NOT_SWEPT.contains(&format!("{family} {field}").as_str()) {
            continue;
        }
        let Some((min, max)) = option_range(family, &field, &menu) else {
            findings.push(format!(
                "{family} {field}: a numeric value with no menu range"
            ));
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
            assert!(
                samples.iter().all(|sample| sample.is_finite()),
                "{label} = {value}: non-finite output"
            );
            let loudest = peak(samples);
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
    for family in ["synth", "fm", "pluck", "drum", "sample"] {
        sweep_family(family, &mut findings);
    }
    assert!(findings.is_empty(), "{}", findings.join("\n"));
}
