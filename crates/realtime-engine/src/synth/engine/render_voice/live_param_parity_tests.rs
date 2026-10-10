use super::super::*;
use crate::synth::{FmParamId, PluckParamId, ScalarMutation, SynthParamId};
use serde_json::Value;

fn numeric_leaves(value: &Value, prefix: &str, out: &mut Vec<(String, f64)>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                numeric_leaves(child, &format!("{prefix}.{key}"), out);
            }
        }
        Value::Number(number) => out.push((prefix.into(), number.as_f64().unwrap())),
        _ => {}
    }
}

fn with_leaf(config: &Value, path: &str, value: f64) -> Value {
    let mut patched = config.clone();
    let mut node = &mut patched;
    let mut parts = path.split('.').skip(1).peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            node[part] = if value.fract() == 0.0 {
                serde_json::json!(value as i64)
            } else {
                serde_json::json!(value)
            };
        } else {
            node = &mut node[part];
        }
    }
    patched
}

fn slot(kind: &str, config: Option<&Value>) -> Option<InstrumentSlotConfig> {
    let parse = |kind_matches: bool| config.filter(|_| kind_matches).cloned();
    Some(InstrumentSlotConfig {
        kind: kind.into(),
        synth: match parse(kind == "synth") {
            Some(config) => serde_json::from_value(config).ok()?,
            None => default_synth_config(),
        },
        fm: match parse(kind == "fm") {
            Some(config) => Some(serde_json::from_value(config).ok()?),
            None => (kind == "fm").then(FmConfig::default),
        },
        pluck: match parse(kind == "pluck") {
            Some(config) => Some(serde_json::from_value(config).ok()?),
            None => (kind == "pluck").then(PluckConfig::default),
        },
        drum: None,
        mixer: Some(InstrumentMixerConfig {
            route: "direct".into(),
            pan_pos: DEFAULT_PAN_POSITIONS / 2,
            volume: 100.0,
        }),
    })
}

fn held_engine(kind: &str) -> SynthEngine {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![slot(kind, None).unwrap()],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.note_on(0, 69, 110, 10_000);
    for _ in 0..500 {
        engine.next_sample();
    }
    engine
}

fn assert_live_controls_match_patches<Id: Copy + PartialEq + std::fmt::Debug>(
    kind: &str,
    base: Value,
    all: &[Id],
    from_path: fn(&str) -> Option<Id>,
    set_live: fn(&mut SynthEngine, Id, f32) -> ScalarMutation,
) {
    let mut leaves = Vec::new();
    numeric_leaves(&base, kind, &mut leaves);
    let mut covered = Vec::new();
    for (path, default) in leaves {
        let Some(id) = from_path(&path) else {
            continue;
        };
        covered.push(id);
        let checked = [default + 1.0, default - 1.0].into_iter().any(|value| {
            let mut live = held_engine(kind);
            if set_live(&mut live, id, value as f32) != ScalarMutation::Changed {
                return false;
            }
            let Some(patch) = slot(kind, Some(&with_leaf(&base, &path, value))) else {
                return false;
            };
            let mut prepared = held_engine(kind);
            let _ =
                prepared.apply_prepared_instrument_slot(0, prepare_instrument_slot_config(patch));
            for sample in 0..512 {
                assert_eq!(
                    prepared.next_sample().to_bits(),
                    live.next_sample().to_bits(),
                    "{path} diverges at sample {sample}"
                );
            }
            true
        });
        assert!(checked, "{path} could not be changed live");
    }
    for id in all {
        assert!(covered.contains(id), "{id:?} has no patch field");
    }
}

#[test]
fn every_live_synth_control_sounds_like_the_same_value_loaded_from_a_patch() {
    assert_live_controls_match_patches(
        "synth",
        serde_json::to_value(default_synth_config()).unwrap(),
        &SynthParamId::ALL,
        SynthParamId::from_path,
        |engine, id, value| engine.set_synth_param_typed(0, id, value),
    );
}

#[test]
fn every_live_fm_control_sounds_like_the_same_value_loaded_from_a_patch() {
    assert_live_controls_match_patches(
        "fm",
        serde_json::to_value(FmConfig::default()).unwrap(),
        &FmParamId::ALL,
        FmParamId::from_path,
        |engine, id, value| engine.set_fm_param_typed(0, id, value),
    );
}

#[test]
fn every_live_pluck_control_sounds_like_the_same_value_loaded_from_a_patch() {
    assert_live_controls_match_patches(
        "pluck",
        serde_json::to_value(PluckConfig::default()).unwrap(),
        &PluckParamId::ALL,
        PluckParamId::from_path,
        |engine, id, value| engine.set_pluck_param_typed(0, id, value),
    );
}
