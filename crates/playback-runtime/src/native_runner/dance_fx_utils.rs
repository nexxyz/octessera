use std::collections::BTreeMap;

use super::{json, Value};

pub(super) fn default_dance_fx_selected() -> Value {
    json!({ "fxType": "none", "targetKey": "master", "params": {} })
}

pub(super) fn sanitize_dance_fx_config(config: &Value) -> Value {
    let fx_type = match dance_fx_type(config) {
        "stutter" | "freeze" | "filter_sweep" | "pitch_shift" => dance_fx_type(config),
        _ => "none",
    };
    let target_key = match dance_fx_target_key(config) {
        "master" | "fx_bus_1" | "fx_bus_2" | "instrument_1" | "instrument_2" | "instrument_3"
        | "instrument_4" | "instrument_5" | "instrument_6" | "instrument_7" | "instrument_8" => {
            dance_fx_target_key(config)
        }
        _ => "master",
    };
    let mut params = serde_json::Map::new();
    for key in dance_fx_param_keys(fx_type) {
        let value = config
            .get("params")
            .and_then(|params| params.get(*key))
            .and_then(Value::as_i64)
            .unwrap_or_else(|| i64::from(dance_fx_param_default(fx_type, key)));
        params.insert(
            (*key).into(),
            json!(sanitize_dance_fx_param(fx_type, key, value)),
        );
    }
    json!({ "fxType": fx_type, "targetKey": target_key, "params": params })
}

fn sanitize_dance_fx_param(fx_type: &str, key: &str, value: i64) -> i64 {
    match (fx_type, key) {
        ("stutter", "rateHz") => value.clamp(1, 32),
        ("stutter", "depthPct") => value.clamp(0, 100),
        ("freeze", "releaseMs") => value.clamp(10, 5000),
        ("freeze", "mixPct") => value.clamp(0, 100),
        ("filter_sweep", "cutoffPct") => value.clamp(0, 100),
        ("filter_sweep", "resonancePct") => value.clamp(0, 100),
        ("filter_sweep", "sweepInMs") => value.clamp(10, 3000),
        ("filter_sweep", "sweepOutMs") => value.clamp(10, 3000),
        ("pitch_shift", "semitones") => value.clamp(-24, 24),
        ("pitch_shift", "cents") => value.clamp(-100, 100),
        ("pitch_shift", "mixPct") => value.clamp(0, 100),
        _ => value,
    }
}

pub(super) fn dance_fx_type(config: &Value) -> &str {
    config
        .get("fxType")
        .and_then(Value::as_str)
        .unwrap_or("none")
}

pub(super) fn dance_fx_target_key(config: &Value) -> &str {
    config
        .get("targetKey")
        .and_then(Value::as_str)
        .unwrap_or("master")
}

pub(super) fn dance_fx_params_map(config: &Value) -> serde_json::Map<String, Value> {
    config
        .get("params")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

pub(super) fn dance_fx_params(config: &Value) -> BTreeMap<String, Value> {
    dance_fx_params_map(config).into_iter().collect()
}

pub(super) fn dance_fx_param_keys(fx_type: &str) -> &'static [&'static str] {
    match fx_type {
        "stutter" => &["rateHz", "depthPct"],
        "freeze" => &["releaseMs", "mixPct"],
        "filter_sweep" => &["cutoffPct", "resonancePct", "sweepInMs", "sweepOutMs"],
        "pitch_shift" => &["semitones", "cents", "mixPct"],
        _ => &[],
    }
}

pub(super) fn dance_fx_param_default(fx_type: &str, key: &str) -> i32 {
    match (fx_type, key) {
        ("stutter", "rateHz") => 8,
        ("stutter", "depthPct") => 100,
        ("freeze", "releaseMs") => 500,
        ("freeze", "mixPct") => 100,
        ("filter_sweep", "cutoffPct") => 50,
        ("filter_sweep", "resonancePct") => 0,
        ("filter_sweep", "sweepInMs") => 120,
        ("filter_sweep", "sweepOutMs") => 180,
        ("pitch_shift", "semitones") => 0,
        ("pitch_shift", "cents") => 0,
        ("pitch_shift", "mixPct") => 100,
        _ => 0,
    }
}

pub(super) fn momentary_fx_color(fx_type: &str) -> Value {
    match fx_type {
        "stutter" => json!({ "r": 255, "g": 170, "b": 40 }),
        "freeze" => json!({ "r": 80, "g": 210, "b": 255 }),
        "filter_sweep" => json!({ "r": 80, "g": 255, "b": 120 }),
        "pitch_shift" => json!({ "r": 190, "g": 90, "b": 255 }),
        _ => json!({ "r": 40, "g": 40, "b": 40 }),
    }
}
