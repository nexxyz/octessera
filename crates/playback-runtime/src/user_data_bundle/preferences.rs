use super::{UserPreferenceDelta, Value};
use crate::native_runner::validate_user_data_config_payload;
use serde_json::Map;
use std::collections::BTreeMap;

const SCALARS: &[&str] = &[
    "masterVolume",
    "ghostCells",
    "inputEventsWhilePaused",
    "numericDisplayMode",
    "dimTimerSeconds",
    "screenSleepSeconds",
    "displayBrightness",
    "gridBrightness",
    "buttonBrightness",
    "autoSaveDefault",
    "rollingBackups",
    "auxAutoMapEnabled",
];
const OBJECTS: &[(&str, &[&str])] = &[
    ("hdmi", &["mode", "showGridlines", "cycleMeasures"]),
    (
        "midi",
        &[
            "enabled",
            "syncMode",
            "clockOutEnabled",
            "clockInEnabled",
            "respondToStartStop",
        ],
    ),
    ("usb", &["midiOutEnabled"]),
    ("audioOutputs", &["dac", "usb", "hdmi"]),
    ("recording", &["maxMinutes"]),
    ("sound", &["audioOutputBufferFrames"]),
];

pub(super) fn projection(runtime: &Map<String, Value>) -> BTreeMap<String, Value> {
    let mut output = BTreeMap::new();
    for key in SCALARS {
        if let Some(value) = runtime.get(*key) {
            output.insert((*key).into(), value.clone());
        }
    }
    for (key, fields) in OBJECTS {
        let Some(source) = runtime.get(*key).and_then(Value::as_object) else {
            continue;
        };
        let selected = fields
            .iter()
            .filter_map(|field| {
                source
                    .get(*field)
                    .map(|value| ((*field).into(), value.clone()))
            })
            .collect::<Map<String, Value>>();
        if !selected.is_empty() {
            output.insert((*key).into(), Value::Object(selected));
        }
    }
    output
}

pub(super) fn shape(delta: &UserPreferenceDelta) -> Result<(), String> {
    for (key, value) in &delta.values {
        if SCALARS.contains(&key.as_str()) {
            if !value.is_boolean() && !value.is_number() && !value.is_string() {
                return Err(format!("preferences.{key} has an invalid value"));
            }
            continue;
        }
        let Some((_, fields)) = OBJECTS.iter().find(|(name, _)| *name == key) else {
            return Err(format!("preferences.{key} is unknown"));
        };
        let object = value
            .as_object()
            .ok_or_else(|| format!("preferences.{key} must be an object"))?;
        for field in object.keys() {
            if !fields.contains(&field.as_str()) {
                return Err(format!("preferences.{key}.{field} is unknown"));
            }
        }
    }
    Ok(())
}

pub(super) fn validate(
    delta: &UserPreferenceDelta,
    canonical_defaults: &Value,
) -> Result<(), String> {
    shape(delta)?;
    let _ = apply(canonical_defaults, delta)?;
    Ok(())
}

pub(super) fn apply(
    canonical_defaults: &Value,
    delta: &UserPreferenceDelta,
) -> Result<Value, String> {
    let mut output = canonical_defaults.clone();
    let runtime = output
        .get_mut("runtimeConfig")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "canonical defaults.runtimeConfig must be an object".to_string())?;
    for (key, value) in &delta.values {
        merge_value(runtime.entry(key.clone()).or_insert(Value::Null), value);
    }
    validate_user_data_config_payload(&output)?;
    Ok(output)
}

fn merge_value(target: &mut Value, source: &Value) {
    if let Value::Object(source) = source {
        if let Some(target) = target.as_object_mut() {
            for (key, value) in source {
                merge_value(target.entry(key.clone()).or_insert(Value::Null), value);
            }
            return;
        }
    }
    *target = source.clone();
}
