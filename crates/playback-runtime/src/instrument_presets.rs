//! The factory instrument presets: per family, a list of overrides on that family's default
//! config, read from `resources/instrument-presets.json`.
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const INSTRUMENT_PRESETS_JSON: &str = include_str!("../../../resources/instrument-presets.json");

#[derive(Debug, Deserialize)]
pub(crate) struct InstrumentPreset {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) set: Value,
}

/// The presets of `family` (`synth`, `fm`, `pluck` or `drum`), in menu order.
pub(crate) fn instrument_presets(family: &str) -> &'static [InstrumentPreset] {
    static PRESETS: OnceLock<BTreeMap<String, Vec<InstrumentPreset>>> = OnceLock::new();
    PRESETS
        .get_or_init(|| {
            serde_json::from_str(INSTRUMENT_PRESETS_JSON).expect("valid instrument presets")
        })
        .get(family)
        .map_or(&[], Vec::as_slice)
}

/// `defaults` with the overrides of preset `id` merged in, or `None` for an unknown preset.
pub(crate) fn instrument_preset_config(family: &str, id: &str, defaults: Value) -> Option<Value> {
    let preset = instrument_presets(family)
        .iter()
        .find(|preset| preset.id == id)?;
    let mut config = defaults;
    merge_overrides(&mut config, &preset.set);
    Some(config)
}

/// Objects merge by key and arrays by position (the eight Drum voices); anything else replaces.
fn merge_overrides(target: &mut Value, overrides: &Value) {
    match (target, overrides) {
        (Value::Object(target), Value::Object(overrides)) => {
            for (key, value) in overrides {
                match target.get_mut(key) {
                    Some(current) => merge_overrides(current, value),
                    None => {
                        target.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (Value::Array(target), Value::Array(overrides)) => {
            for (current, value) in target.iter_mut().zip(overrides) {
                merge_overrides(current, value);
            }
        }
        (target, value) => *target = value.clone(),
    }
}
