use super::{Value, CONFIG_KIND, CONFIG_SCHEMA_VERSION, GRID_HEIGHT, GRID_WIDTH};

pub(super) fn validate_config_payload(payload: &Value) -> Result<(), String> {
    let object = payload
        .as_object()
        .ok_or_else(|| "configuration payload must be an object".to_string())?;
    if object.get("kind").and_then(Value::as_str) != Some(CONFIG_KIND)
        || object.get("schemaVersion").and_then(Value::as_u64) != Some(CONFIG_SCHEMA_VERSION)
    {
        return Err("prepared configuration has an invalid envelope".into());
    }
    if object.get("revision").is_some_and(|value| !value.is_u64()) {
        return Err("configuration revision must be an unsigned integer".into());
    }
    let runtime = object
        .get("runtimeConfig")
        .and_then(Value::as_object)
        .ok_or_else(|| "configuration runtimeConfig must be an object".to_string())?;
    validate_known_fields(payload, None)?;
    if let Some(mapping) = object.get("mappingConfig") {
        serde_json::from_value::<platform_core::MappingConfig>(mapping.clone())
            .map_err(|error| format!("invalid mappingConfig: {error}"))?;
    }
    validate_layers(runtime)?;
    validate_sparks_assignments(runtime)
}

fn validate_layers(runtime: &serde_json::Map<String, Value>) -> Result<(), String> {
    let Some(layers) = runtime.get("layers") else {
        return Ok(());
    };
    for (index, layer) in layers
        .as_array()
        .ok_or_else(|| "runtimeConfig.layers must be an array".to_string())?
        .iter()
        .enumerate()
    {
        let layer = layer
            .as_object()
            .ok_or_else(|| format!("layer {index} must be an object"))?;
        let Some(worlds) = layer.get("worlds") else {
            continue;
        };
        let worlds = worlds
            .as_object()
            .ok_or_else(|| format!("layer {index}.worlds must be an object"))?;
        if let Some(behavior_id) = worlds.get("behaviorId") {
            let behavior_id = behavior_id
                .as_str()
                .ok_or_else(|| format!("layer {index}.worlds.behaviorId must be a string"))?;
            if platform_core::get_native_behavior(behavior_id).is_none() {
                return Err(format!("unsupported native behavior `{behavior_id}`"));
            }
        }
    }
    Ok(())
}

fn validate_sparks_assignments(runtime: &serde_json::Map<String, Value>) -> Result<(), String> {
    let Some(assignments) = runtime
        .get("sparksFx")
        .and_then(|sparks| sparks.get("assignments"))
    else {
        return Ok(());
    };
    for (index, assignment) in assignments
        .as_array()
        .ok_or_else(|| "sparksFx.assignments must be an array".to_string())?
        .iter()
        .enumerate()
    {
        let assignment = assignment
            .as_object()
            .ok_or_else(|| format!("sparksFx assignment {index} must be an object"))?;
        for key in ["x", "y"] {
            let value = assignment
                .get(key)
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("sparksFx assignment {index}.{key} must be an integer"))?;
            let limit = if key == "x" { GRID_WIDTH } else { GRID_HEIGHT };
            if value >= u64::try_from(limit).unwrap_or(u64::MAX) {
                return Err(format!("sparksFx assignment {index}.{key} is out of range"));
            }
        }
    }
    Ok(())
}

fn validate_known_fields(value: &Value, key: Option<&str>) -> Result<(), String> {
    if let Some(key) = key {
        validate_field_type(key, value)?;
    }
    match value {
        Value::Object(object) => object
            .iter()
            .try_for_each(|(key, value)| validate_known_fields(value, Some(key)))?,
        Value::Array(values) => values
            .iter()
            .try_for_each(|value| validate_known_fields(value, None))?,
        _ => {}
    }
    Ok(())
}

fn validate_field_type(key: &str, value: &Value) -> Result<(), String> {
    if matches!(key, "revision" | "schemaVersion") && value.as_u64().is_none() {
        return Err(format!("{key} must be an unsigned integer"));
    }
    if integer_field(key) {
        if signed_integer_field(key) {
            value
                .as_i64()
                .ok_or_else(|| format!("{key} must be an integer"))?;
        } else if !value.as_u64().is_some_and(|value| field_range(key, value)) {
            return Err(format!("{key} must be an integer in its supported range"));
        }
    }
    if key == "velocity"
        && value.is_number()
        && value
            .as_u64()
            .is_none_or(|value| value > u64::from(u8::MAX))
    {
        return Err("velocity must be an unsigned integer in its supported range".into());
    }
    if matches!(key, "bpm" | "min" | "max" | "step" | "userMin" | "userMax")
        && value.as_f64().is_none()
    {
        return Err(format!("{key} must be a number"));
    }
    if string_field(key)
        && ((matches!(key, "path" | "turnKey") && !value.is_null() && !value.is_string())
            || (!matches!(key, "path" | "turnKey") && !value.is_string()))
    {
        return Err(format!("{key} must be a string"));
    }
    if bool_field(key) && !value.is_boolean() {
        return Err(format!("{key} must be a boolean"));
    }
    Ok(())
}

fn integer_field(key: &str) -> bool {
    matches!(
        key,
        "activeLayerIndex"
            | "schemaVersion"
            | "revision"
            | "masterVolume"
            | "noteLengthMs"
            | "velocityScalePct"
            | "audioOutputBufferFrames"
            | "displayBrightness"
            | "gridBrightness"
            | "buttonBrightness"
            | "screenSleepSeconds"
            | "dimTimerSeconds"
            | "cycleMeasures"
            | "swingPct"
            | "scanSections"
            | "delaySteps"
            | "retriggerCount"
            | "depthPct"
            | "gainPct"
            | "velocitySensitivityPct"
            | "baseVelocity"
            | "high"
            | "medium"
            | "low"
            | "volume"
            | "channel"
            | "durationMs"
            | "selectedSlot"
            | "stepIntervalSteps"
            | "gatePct"
            | "octaveSpread"
            | "from"
            | "to"
            | "tuneSemis"
            | "gridOffset"
    )
}

fn signed_integer_field(key: &str) -> bool {
    matches!(
        key,
        "tuneSemis"
            | "gridOffset"
            | "stepIntervalSteps"
            | "noteLengthMs"
            | "gatePct"
            | "octaveSpread"
    )
}

fn field_range(key: &str, value: u64) -> bool {
    match key {
        "activeLayerIndex"
        | "masterVolume"
        | "displayBrightness"
        | "gridBrightness"
        | "buttonBrightness"
        | "cycleMeasures"
        | "swingPct"
        | "scanSections"
        | "delaySteps"
        | "retriggerCount"
        | "depthPct"
        | "gainPct"
        | "velocitySensitivityPct"
        | "baseVelocity"
        | "high"
        | "medium"
        | "low"
        | "velocity"
        | "volume"
        | "channel"
        | "selectedSlot"
        | "stepIntervalSteps"
        | "gatePct"
        | "octaveSpread"
        | "from"
        | "to" => value <= u64::from(u32::MAX),
        "noteLengthMs" => value <= u64::from(u32::MAX),
        "velocityScalePct" => value <= u64::from(u16::MAX),
        "audioOutputBufferFrames" => value <= u64::from(u32::MAX),
        "screenSleepSeconds" | "dimTimerSeconds" | "durationMs" | "maxMinutes" => {
            value <= u64::from(u16::MAX)
        }
        _ => true,
    }
}

fn string_field(key: &str) -> bool {
    matches!(
        key,
        "kind"
            | "type"
            | "behaviorId"
            | "noteBehavior"
            | "name"
            | "route"
            | "mode"
            | "source"
            | "scanMode"
            | "scanAxis"
            | "scanUnit"
            | "scanDirection"
            | "triggerProbabilityMode"
            | "root"
            | "outOfRange"
            | "curve"
            | "velocityCurve"
            | "voiceStealingMode"
            | "numericDisplayMode"
            | "audioOut"
            | "syncMode"
            | "path"
            | "action"
            | "turnKey"
            | "period"
    )
}

fn bool_field(key: &str) -> bool {
    matches!(
        key,
        "autoName"
            | "enabled"
            | "eventEnabled"
            | "stateNotesEnabled"
            | "restartEachSection"
            | "velocityLevelsEnabled"
            | "inputEventsWhilePaused"
            | "ghostCells"
            | "showGridlines"
            | "autoSaveDefault"
            | "rollingBackups"
            | "auxAutoMapEnabled"
            | "midiOutEnabled"
            | "midiClockOutEnabled"
            | "clockOutEnabled"
            | "clockInEnabled"
            | "respondToStartStop"
    )
}
