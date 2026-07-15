use super::options::duck_source_options;
use super::{enum_item_from_strings, number_item, NativeMenuItem};
use crate::delay_timing::normalized_delay_params;
use crate::timing_units::NOTE_UNIT_OPTIONS;

pub(super) fn fx_param_items(
    slot_type: &str,
    prefix: &str,
    params: &serde_json::Value,
    bus_index: Option<usize>,
    bpm: u16,
) -> Vec<NativeMenuItem> {
    match slot_type {
        "duck" => duck_param_items(prefix, params, bus_index),
        "delay" => vec![
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 35.0),
            delay_time_mode_item(prefix, params, bpm),
            delay_time_note_item(prefix, params, bpm),
            fx_number_item("Time ms", prefix, params, "timeMs", 1, 2000, 5, 1.0, 250.0),
            fx_number_item(
                "Feedback", prefix, params, "feedback", 0, 98, 1, 100.0, 0.35,
            ),
        ],
        "tremolo" => vec![
            fx_number_item("Rate Hz", prefix, params, "rateHz", 5, 4000, 5, 100.0, 4.0),
            fx_number_item("Depth %", prefix, params, "depthPct", 0, 100, 1, 1.0, 60.0),
        ],
        "saturator" => vec![
            fx_number_item("Drive", prefix, params, "drive", 0, 200, 1, 10.0, 1.8),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "distortion" => vec![
            fx_number_item("Drive", prefix, params, "drive", 0, 500, 5, 10.0, 2.5),
            fx_number_item("Clip", prefix, params, "clip", 5, 200, 5, 100.0, 0.6),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "bitcrusher" => vec![
            fx_number_item("Bits", prefix, params, "bits", 1, 16, 1, 1.0, 6.0),
            fx_number_item("Rate Div", prefix, params, "rateDiv", 1, 128, 1, 1.0, 4.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "vibrato" | "chorus" | "flanger" => modulation_param_items(prefix, params),
        "filter_lfo" | "wah" => vec![
            fx_number_item("Rate Hz", prefix, params, "rateHz", 2, 2000, 5, 100.0, 0.5),
            fx_number_item(
                "Center", prefix, params, "centerHz", 40, 12000, 20, 1.0, 1600.0,
            ),
            fx_number_item("Depth %", prefix, params, "depthPct", 0, 100, 1, 1.0, 70.0),
            fx_number_item("Q", prefix, params, "q", 25, 2000, 25, 100.0, 1.0),
        ],
        "reverb" => vec![
            fx_number_item("Decay", prefix, params, "decay", 0, 995, 5, 1000.0, 0.72),
            fx_number_item("Damp", prefix, params, "damp", 0, 98, 1, 100.0, 0.35),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 30.0),
        ],
        "auto_pan" => vec![
            fx_number_item("Rate Hz", prefix, params, "rateHz", 2, 2000, 5, 100.0, 0.5),
            fx_number_item("Depth %", prefix, params, "depthPct", 0, 100, 1, 1.0, 100.0),
        ],
        "glitch" => vec![
            fx_number_item("Chance %", prefix, params, "chancePct", 0, 100, 1, 1.0, 8.0),
            fx_number_item("Slice ms", prefix, params, "sliceMs", 5, 500, 5, 1.0, 80.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "compressor" => compressor_param_items(prefix, params),
        "eq" => eq_param_items(prefix, params),
        "vinyl" => vinyl_param_items(prefix, params),
        _ => vec![],
    }
}

fn delay_time_mode_item(prefix: &str, params: &serde_json::Value, bpm: u16) -> NativeMenuItem {
    let params = normalized_delay_params(params, bpm);
    let mode = params
        .get("timeMode")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("ms");
    let options = vec!["ms".to_string(), "note".to_string()];
    enum_item_from_strings(
        "Time Mode",
        format!("{prefix}.timeMode"),
        options,
        if mode == "note" { 1 } else { 0 },
    )
}

fn delay_time_note_item(prefix: &str, params: &serde_json::Value, bpm: u16) -> NativeMenuItem {
    let params = normalized_delay_params(params, bpm);
    let selected_note = params
        .get("timeNote")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("1/8");
    enum_item_from_strings(
        "Time Note",
        format!("{prefix}.timeNote"),
        NOTE_UNIT_OPTIONS
            .iter()
            .map(|option| (*option).to_string())
            .collect(),
        NOTE_UNIT_OPTIONS
            .iter()
            .position(|option| option == &selected_note)
            .unwrap_or(0),
    )
}

fn duck_param_items(
    prefix: &str,
    params: &serde_json::Value,
    bus_index: Option<usize>,
) -> Vec<NativeMenuItem> {
    let options = duck_source_options(bus_index.unwrap_or(usize::MAX));
    vec![
        enum_item_from_strings(
            "Source",
            format!("{prefix}.source"),
            options.clone(),
            options
                .iter()
                .position(|option| option == &fx_param_string(params, "source", "I1"))
                .unwrap_or(0),
        ),
        fx_number_item(
            "Threshold",
            prefix,
            params,
            "threshold",
            0,
            100,
            1,
            100.0,
            0.08,
        ),
        fx_number_item(
            "Amount %",
            prefix,
            params,
            "amountPct",
            0,
            100,
            1,
            1.0,
            60.0,
        ),
        fx_number_item("Attack", prefix, params, "attackMs", 1, 500, 1, 1.0, 8.0),
        fx_number_item(
            "Release",
            prefix,
            params,
            "releaseMs",
            1,
            5000,
            5,
            1.0,
            160.0,
        ),
    ]
}

fn modulation_param_items(prefix: &str, params: &serde_json::Value) -> Vec<NativeMenuItem> {
    vec![
        fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        fx_number_item("Rate Hz", prefix, params, "rateHz", 2, 2000, 5, 100.0, 0.8),
        fx_number_item("Depth ms", prefix, params, "depthMs", 0, 400, 1, 10.0, 6.0),
        fx_number_item("Base ms", prefix, params, "baseMs", 1, 800, 1, 10.0, 8.0),
        fx_number_item(
            "Feedback", prefix, params, "feedback", -95, 95, 1, 100.0, 0.0,
        ),
    ]
}

fn compressor_param_items(prefix: &str, params: &serde_json::Value) -> Vec<NativeMenuItem> {
    vec![
        fx_number_item(
            "Thresh dB",
            prefix,
            params,
            "thresholdDb",
            -120,
            0,
            1,
            2.0,
            -24.0,
        ),
        fx_number_item("Ratio", prefix, params, "ratio", 2, 40, 1, 2.0, 4.0),
        fx_number_item("Attack", prefix, params, "attackMs", 1, 200, 1, 1.0, 10.0),
        fx_number_item(
            "Release",
            prefix,
            params,
            "releaseMs",
            5,
            2000,
            5,
            1.0,
            100.0,
        ),
        fx_number_item("Makeup dB", prefix, params, "makeupDb", 0, 48, 1, 2.0, 0.0),
        fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
    ]
}

fn eq_param_items(prefix: &str, params: &serde_json::Value) -> Vec<NativeMenuItem> {
    vec![
        fx_number_item(
            "Low Gain dB",
            prefix,
            params,
            "lowGainDb",
            -24,
            24,
            1,
            2.0,
            0.0,
        ),
        fx_number_item(
            "Mid Gain dB",
            prefix,
            params,
            "midGainDb",
            -24,
            24,
            1,
            2.0,
            0.0,
        ),
        fx_number_item(
            "High Gain dB",
            prefix,
            params,
            "highGainDb",
            -24,
            24,
            1,
            2.0,
            0.0,
        ),
        fx_number_item(
            "Mid Freq Hz",
            prefix,
            params,
            "midFreqHz",
            40,
            8000,
            10,
            1.0,
            1000.0,
        ),
        fx_number_item("Mid Q", prefix, params, "midQ", 25, 2000, 25, 100.0, 1.0),
        fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
    ]
}

fn vinyl_param_items(prefix: &str, params: &serde_json::Value) -> Vec<NativeMenuItem> {
    vec![
        fx_number_item(
            "Saturation %",
            prefix,
            params,
            "saturationPct",
            0,
            100,
            1,
            1.0,
            15.0,
        ),
        fx_number_item(
            "Crackle %",
            prefix,
            params,
            "cracklePct",
            0,
            100,
            1,
            1.0,
            8.0,
        ),
        fx_number_item(
            "Warp Depth %",
            prefix,
            params,
            "warpDepthPct",
            0,
            100,
            1,
            1.0,
            5.0,
        ),
        fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
    ]
}

#[expect(clippy::too_many_arguments, reason = "FX menu specs are data rows")]
fn fx_number_item(
    label: impl Into<String>,
    prefix: &str,
    params: &serde_json::Value,
    key: &str,
    min: i32,
    max: i32,
    step: i32,
    scale: f64,
    default: f64,
) -> NativeMenuItem {
    number_item(
        label,
        format!("{prefix}.{key}"),
        ((fx_param_number(params, key, default) * scale).round() as i32).clamp(min, max),
        min,
        max,
        step,
    )
}

fn fx_param_number(params: &serde_json::Value, key: &str, default: f64) -> f64 {
    params
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(default)
}

fn fx_param_string(params: &serde_json::Value, key: &str, default: &str) -> String {
    params
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default)
        .into()
}
