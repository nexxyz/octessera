use super::canonical::{bool_value, number_value, signed_value, unsigned_value_range};
use super::Value;

pub(super) fn walk_scalars(value: &Value, path: &str) -> Result<(), String> {
    if is_opaque_scalar_subtree(path) {
        return Ok(());
    }
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let next_path = format!("{path}.{key}");
                validate_scalar(key, value, &next_path)?;
                walk_scalars(value, &next_path)?;
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                walk_scalars(value, &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_opaque_scalar_subtree(path: &str) -> bool {
    [
        ".behaviorConfig",
        ".behaviorConfigHistory",
        ".savedState",
        ".behaviorState",
        ".params",
    ]
    .iter()
    .any(|suffix| path.ends_with(suffix))
}

fn validate_scalar(key: &str, value: &Value, path: &str) -> Result<(), String> {
    if matches!(key, "path" | "turnKey") && !matches!(value, Value::Null | Value::String(_)) {
        return Err(format!("{path} must be a string or null"));
    }
    match key {
        "revision" | "schemaVersion" => unsigned_value(value, path)?,
        "masterVolume" | "displayBrightness" | "gridBrightness" | "buttonBrightness" => {
            unsigned_range(value, path, 0, 100)?
        }
        "velocityScalePct" => unsigned_range(value, path, 0, 200)?,
        "screenSleepSeconds" | "dimTimerSeconds" => unsigned_range(value, path, 0, 600)?,
        "swingPct" => unsigned_range(value, path, 0, 75)?,
        "bpm" => number_value(value, path, 40.0, 240.0)?,
        "noteLengthMs" => signed_value(value, path, 10, 2000)?,
        "scanSections" => unsigned_range(value, path, 1, 8)?,
        "delaySteps" => unsigned_range(value, path, 0, 16)?,
        "retriggerCount" => unsigned_range(value, path, 0, 8)?,
        "depthPct" | "gainPct" | "velocitySensitivityPct" => unsigned_range(value, path, 0, 100)?,
        "channel" => unsigned_range(value, path, 0, 16)?,
        "durationMs" => unsigned_range(value, path, 10, 5000)?,
        "selectedSlot" => unsigned_range(
            value,
            path,
            0,
            (platform_core::SAMPLE_SLOT_COUNT - 1) as u64,
        )?,
        "audioOutputBufferFrames" => unsigned_value(value, path)?,
        "enabled" | "autoName" | "eventEnabled" | "stateNotesEnabled" | "saveGridState"
        | "xInvert" | "yInvert" | "invert" | "showGridlines" => bool_value(value, path)?,
        "path" | "turnKey" => {}
        _ => {}
    }
    Ok(())
}

fn unsigned_value(value: &Value, path: &str) -> Result<(), String> {
    if value.as_u64().is_none() {
        return Err(format!("{path} must be an unsigned integer"));
    }
    Ok(())
}

fn unsigned_range(value: &Value, path: &str, min: u64, max: u64) -> Result<(), String> {
    unsigned_value_range(value, path, min, max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scalar_walk_is_opaque_only_at_dynamic_subtree_paths() {
        let opaque = json!({
            "revision": "owned",
            "enabled": "owned",
            "durationMs": "owned",
            "channel": "owned"
        });
        assert!(walk_scalars(&json!({ "params": opaque }), "configuration").is_ok());
        assert!(walk_scalars(&json!({ "revision": "broken" }), "configuration").is_err());
    }
}
