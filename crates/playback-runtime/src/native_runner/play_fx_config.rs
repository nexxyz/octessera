use std::collections::BTreeMap;

use super::{json, Value};

/// One Play FX parameter: its saved key, menu label, default, range and encoder step.
pub(crate) struct PlayFxParam {
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) default: i32,
    pub(crate) min: i32,
    pub(crate) max: i32,
    pub(crate) step: i32,
}

const fn param(
    key: &'static str,
    label: &'static str,
    default: i32,
    min: i32,
    max: i32,
    step: i32,
) -> PlayFxParam {
    PlayFxParam {
        key,
        label,
        default,
        min,
        max,
        step,
    }
}

/// Each Play FX type's parameters in menu row order.
const PLAY_FX_PARAMS: [(&str, &[PlayFxParam]); 4] = [
    (
        "stutter",
        &[
            param("rateHz", "Rate Hz", 8, 1, 32, 1),
            param("depthPct", "Depth", 100, 0, 100, 1),
        ],
    ),
    (
        "freeze",
        &[
            param("releaseMs", "Release Ms", 500, 10, 5000, 10),
            param("mixPct", "Mix", 100, 0, 100, 1),
        ],
    ),
    (
        "filter_sweep",
        &[
            param("cutoffPct", "Cutoff", 50, 0, 100, 1),
            param("resonancePct", "Res", 0, 0, 100, 1),
            param("sweepInMs", "Sweep In", 120, 10, 3000, 10),
            param("sweepOutMs", "Sweep Out", 180, 10, 3000, 10),
        ],
    ),
    (
        "pitch_shift",
        &[
            param("semitones", "Semitones", 0, -24, 24, 1),
            param("cents", "Cents", 0, -100, 100, 1),
            param("slideInMs", "Slide In", 120, 10, 3000, 10),
            param("slideOutMs", "Slide Out", 180, 10, 3000, 10),
            param("mixPct", "Mix", 100, 0, 100, 1),
        ],
    ),
];

/// The parameters of a Play FX type; `none` and unknown types have none.
pub(crate) fn play_fx_param_specs(fx_type: &str) -> &'static [PlayFxParam] {
    PLAY_FX_PARAMS
        .iter()
        .find(|(name, _)| *name == fx_type)
        .map_or(&[], |(_, params)| params)
}

pub(super) fn default_play_fx_selected() -> Value {
    json!({ "fxType": "none", "targetKey": "master", "params": {} })
}

pub(super) fn sanitize_play_fx_config(config: &Value) -> Value {
    let fx_type = PLAY_FX_PARAMS
        .iter()
        .map(|(name, _)| *name)
        .find(|name| *name == play_fx_type(config))
        .unwrap_or("none");
    let target_key = match play_fx_target_key(config) {
        "master" | "fx_bus_1" | "fx_bus_2" | "instrument_1" | "instrument_2" | "instrument_3"
        | "instrument_4" | "instrument_5" | "instrument_6" | "instrument_7" | "instrument_8" => {
            play_fx_target_key(config)
        }
        _ => "master",
    };
    let mut params = serde_json::Map::new();
    for spec in play_fx_param_specs(fx_type) {
        let value = config
            .get("params")
            .and_then(|params| params.get(spec.key))
            .and_then(Value::as_i64)
            .unwrap_or_else(|| i64::from(spec.default));
        params.insert(
            spec.key.into(),
            json!(value.clamp(i64::from(spec.min), i64::from(spec.max))),
        );
    }
    json!({ "fxType": fx_type, "targetKey": target_key, "params": params })
}

pub(super) fn play_fx_type(config: &Value) -> &str {
    config
        .get("fxType")
        .and_then(Value::as_str)
        .unwrap_or("none")
}

pub(super) fn play_fx_target_key(config: &Value) -> &str {
    config
        .get("targetKey")
        .and_then(Value::as_str)
        .unwrap_or("master")
}

pub(super) fn play_fx_params_map(config: &Value) -> serde_json::Map<String, Value> {
    config
        .get("params")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

pub(super) fn play_fx_params(config: &Value) -> BTreeMap<String, Value> {
    play_fx_params_map(config).into_iter().collect()
}
