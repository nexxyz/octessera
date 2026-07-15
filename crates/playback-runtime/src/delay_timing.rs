use serde_json::{json, Map, Value};

use crate::timing_units::{note_unit_to_pulses, NOTE_UNIT_OPTIONS};

pub(crate) const DELAY_TIME_MIN_MS: i32 = 1;
pub(crate) const DELAY_TIME_MAX_MS: i32 = 2000;
pub(crate) const MIN_VISIBLE_BPM: f64 = 40.0;
pub(crate) const MAX_VISIBLE_BPM: f64 = 240.0;

pub(crate) fn clamp_visible_bpm(value: f64) -> f64 {
    value.clamp(MIN_VISIBLE_BPM, MAX_VISIBLE_BPM)
}

pub(crate) fn visible_bpm_u16(value: f64) -> u16 {
    clamp_visible_bpm(value).round() as u16
}

pub(crate) fn note_ms(note: &str, bpm: u16) -> i32 {
    let bpm = u32::from(bpm.max(1));
    let pulses = note_unit_to_pulses(note);
    ((60_000 * pulses + (bpm * 24 / 2)) / (bpm * 24))
        .clamp(DELAY_TIME_MIN_MS as u32, DELAY_TIME_MAX_MS as u32) as i32
}

pub(crate) fn nearest_note_for_ms(time_ms: i32, bpm: u16) -> &'static str {
    NOTE_UNIT_OPTIONS
        .iter()
        .min_by_key(|note| (note_ms(note, bpm) - time_ms).abs())
        .copied()
        .unwrap_or("1/8")
}

pub(crate) fn valid_note(note: &str) -> bool {
    NOTE_UNIT_OPTIONS.contains(&note)
}

pub(crate) fn normalized_delay_params(params: &Value, bpm: u16) -> Value {
    let mut object = params.as_object().cloned().unwrap_or_default();
    let time_ms = object
        .get("timeMs")
        .and_then(Value::as_i64)
        .map(|value| (value as i32).clamp(DELAY_TIME_MIN_MS, DELAY_TIME_MAX_MS))
        .unwrap_or(250);
    let mode = match object.get("timeMode").and_then(Value::as_str) {
        Some("note") => "note",
        _ => "ms",
    };
    let note = object
        .get("timeNote")
        .and_then(Value::as_str)
        .filter(|note| valid_note(note))
        .map(str::to_string)
        .unwrap_or_else(|| nearest_note_for_ms(time_ms, bpm).to_string());
    let materialized_ms = if mode == "note" {
        note_ms(&note, bpm)
    } else {
        time_ms
    };
    object.insert("timeMode".into(), json!(mode));
    object.insert("timeNote".into(), json!(note));
    object.insert("timeMs".into(), json!(materialized_ms));
    Value::Object(object)
}

pub(crate) fn strip_delay_timing_metadata(params: &Value) -> Map<String, Value> {
    params
        .as_object()
        .map(|object| {
            object
                .iter()
                .filter(|(key, _)| key.as_str() != "timeMode" && key.as_str() != "timeNote")
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn strip_invalid_timing_metadata(params: &Value) -> Value {
    let mut params = params.clone();
    if let Some(object) = params.as_object_mut() {
        object.remove("timeMode");
        object.remove("timeNote");
    }
    params
}
