use super::{Map, Value, USER_DATA_BUNDLE_KIND, USER_DATA_BUNDLE_SCHEMA_VERSION};

pub(super) fn legacy_fields(object: &mut Map<String, Value>) -> Result<(), String> {
    let allowed = [
        "kind",
        "schemaVersion",
        "metadata",
        "manifest",
        "presets",
        "currentState",
        "defaultState",
        "preferences",
        "mediaIncluded",
        "media",
        "current",
        "default",
    ];
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("user-data bundle field `{key}` is unknown"));
        }
    }
    if object
        .get("kind")
        .is_some_and(|kind| kind.as_str() != Some(USER_DATA_BUNDLE_KIND))
    {
        return Err("unsupported user-data bundle kind".into());
    }
    object.entry("metadata").or_insert_with(
        || serde_json::json!({"boardProfile":"legacy", "runtimeVersion":"unknown"}),
    );
    object
        .entry("presets")
        .or_insert_with(|| Value::Array(Vec::new()));
    migrate_presets(object)?;
    for (legacy, current) in [("current", "currentState"), ("default", "defaultState")] {
        if !object.contains_key(current) {
            if let Some(value) = object.remove(legacy) {
                object.insert(current.into(), wrap_state(value));
            }
        }
    }
    if !object.contains_key("currentState") || !object.contains_key("defaultState") {
        return Err("legacy user-data bundle is missing musical state".into());
    }
    object
        .entry("preferences")
        .or_insert_with(|| Value::Object(Map::new()));
    object.entry("mediaIncluded").or_insert(Value::Bool(false));
    object
        .entry("media")
        .or_insert_with(|| Value::Array(Vec::new()));
    object.insert("kind".into(), Value::String(USER_DATA_BUNDLE_KIND.into()));
    object.insert(
        "schemaVersion".into(),
        Value::Number(USER_DATA_BUNDLE_SCHEMA_VERSION.into()),
    );
    object.insert("manifest".into(), Value::Array(Vec::new()));
    Ok(())
}

fn migrate_presets(object: &mut Map<String, Value>) -> Result<(), String> {
    let presets = object
        .get_mut("presets")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "user-data presets must be an array".to_string())?;
    for preset in presets {
        let preset = preset
            .as_object_mut()
            .ok_or_else(|| "legacy user-data preset must be an object".to_string())?;
        if !preset.contains_key("displayName") {
            if let Some(name) = preset.remove("name") {
                preset.insert("displayName".into(), name);
            }
        }
        if !preset.contains_key("patch") {
            if let Some(payload) = preset.remove("payload") {
                preset.insert("patch".into(), payload);
            }
        }
    }
    Ok(())
}

fn wrap_state(value: Value) -> Value {
    if value.get("patch").is_some() {
        value
    } else {
        serde_json::json!({"patch": value})
    }
}
