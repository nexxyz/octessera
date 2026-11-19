use super::options::duck_source_options;
use super::{enum_item_from_strings, number_item, NativeMenuItem, NativeMenuValue, NumberCurve};
use crate::delay_timing::normalized_delay_params;
use crate::native_runner::fx_param_specs::{fx_param_specs, FxParam, DELAY_TIME_MS};
use crate::timing_units::{note_unit_selection_index, DEFAULT_NOTE_UNIT, NOTE_UNIT_OPTIONS};

pub(super) fn fx_param_items(
    slot_type: &str,
    prefix: &str,
    params: &serde_json::Value,
    bus_index: Option<usize>,
    bpm: u16,
) -> Vec<NativeMenuItem> {
    let mut items = if slot_type == "duck" {
        duck_source_items(prefix, params, bus_index)
    } else {
        Vec::new()
    };
    for spec in fx_param_specs(slot_type) {
        if slot_type == "delay" && spec.key == DELAY_TIME_MS.key {
            items.push(delay_time_mode_item(prefix, params, bpm));
            items.push(delay_time_row_item(
                prefix,
                params,
                bpm,
                delay_time_field(params, bpm),
            ));
        } else {
            items.push(fx_number_item(prefix, params, spec));
        }
    }
    items
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

/// The delay time row its Time Mode shows: `timeNote` in note mode, `timeMs` otherwise.
pub(crate) fn delay_time_field(params: &serde_json::Value, bpm: u16) -> &'static str {
    match normalized_delay_params(params, bpm)
        .get("timeMode")
        .and_then(serde_json::Value::as_str)
    {
        Some("note") => "timeNote",
        _ => "timeMs",
    }
}

/// One delay time row: `timeNote` for that field, `timeMs` otherwise.
pub(crate) fn delay_time_row_item(
    prefix: &str,
    params: &serde_json::Value,
    bpm: u16,
    field: &str,
) -> NativeMenuItem {
    if field == "timeNote" {
        delay_time_note_item(prefix, params, bpm)
    } else {
        fx_number_item(prefix, params, &DELAY_TIME_MS)
    }
}

fn delay_time_note_item(prefix: &str, params: &serde_json::Value, bpm: u16) -> NativeMenuItem {
    let params = normalized_delay_params(params, bpm);
    let selected_note = params
        .get("timeNote")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(DEFAULT_NOTE_UNIT);
    enum_item_from_strings(
        "Time Note",
        format!("{prefix}.timeNote"),
        NOTE_UNIT_OPTIONS
            .iter()
            .map(|option| (*option).to_string())
            .collect(),
        note_unit_selection_index(selected_note),
    )
}

fn duck_source_items(
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
        enum_item_from_strings(
            "Source Tap",
            format!("{prefix}.sourceTap"),
            vec!["pre".into(), "post".into()],
            match fx_param_string(params, "sourceTap", "pre").as_str() {
                "post" => 1,
                _ => 0,
            },
        ),
    ]
}

fn fx_number_item(prefix: &str, params: &serde_json::Value, spec: &FxParam) -> NativeMenuItem {
    let FxParam {
        key,
        label,
        min,
        max,
        step,
        scale,
        default,
    } = *spec;
    let mut item = number_item(
        label,
        format!("{prefix}.{key}"),
        ((fx_param_number(params, key, default) * scale).round() as i32).clamp(min, max),
        min,
        max,
        step,
    );
    if let NativeMenuValue::Number { curve, .. } = &mut item.value {
        if key.ends_with("Ms") {
            *curve = NumberCurve::Time {
                units_per_ms: scale.round() as i32,
            };
        } else if key == "q" || key == "midQ" {
            *curve = NumberCurve::Ratio;
        } else if key == "decay" || key == "feedback" && min >= 0 {
            *curve = NumberCurve::TowardsCeiling {
                ceiling: scale.round() as i32,
            };
        }
    }
    item
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
