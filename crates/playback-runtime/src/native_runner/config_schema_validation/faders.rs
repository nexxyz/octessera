use super::canonical::{array_field, enum_field, object_field, object_value, unsigned_field};
use super::mapping_bindings::{validate_binding_field, validate_press_action};
use super::Value;
use serde_json::Map;

const PATH: &str = "runtimeConfig.faders";

pub(super) fn validate_faders(runtime: &Map<String, Value>) -> Result<(), String> {
    let faders = object_field(runtime, "faders", "runtimeConfig")?
        .ok_or_else(|| format!("{PATH} must be present"))?;
    let count = super::super::play_pages::faders::FADER_COUNT;
    let sliders = array_field(faders, "sliders", PATH, count)?
        .filter(|sliders| sliders.len() == count)
        .ok_or_else(|| format!("{PATH}.sliders must contain exactly eight sliders"))?;
    let top = (platform_core::GRID_HEIGHT - 1) as u64;
    for (index, value) in sliders.iter().enumerate() {
        let path = format!("{PATH}.sliders[{index}]");
        let slider = object_value(value, &path)?;
        for target in ["target", "shiftTarget"] {
            validate_binding_field(slider, target, &path)?;
        }
    }
    unsigned_field(faders, "actionCols", PATH, 0, count as u64)?;
    if !faders.contains_key("actionCols") {
        return Err(format!("{PATH}.actionCols must be present"));
    }
    let tiles = array_field(faders, "tiles", PATH, count * platform_core::GRID_HEIGHT)?
        .ok_or_else(|| format!("{PATH}.tiles must be present"))?;
    let mut positions = std::collections::BTreeSet::new();
    for (index, value) in tiles.iter().enumerate() {
        let path = format!("{PATH}.tiles[{index}]");
        let tile = object_value(value, &path)?;
        validate_tile(tile, &path, count as u64 - 1, top)?;
        if !positions.insert((
            tile.get("x").and_then(Value::as_u64),
            tile.get("y").and_then(Value::as_u64),
        )) {
            return Err(format!("{path} repeats an earlier tile position"));
        }
    }
    Ok(())
}

/// An action tile keeps only its actions, a param tile only its target key.
fn validate_tile(
    tile: &Map<String, Value>,
    path: &str,
    max_x: u64,
    max_y: u64,
) -> Result<(), String> {
    for (field, max) in [("x", max_x), ("y", max_y)] {
        if !tile.contains_key(field) {
            return Err(format!("{path}.{field} must be present"));
        }
        unsigned_field(tile, field, path, 0, max)?;
    }
    enum_field(tile, "type", path, &["action", "param"])?;
    let action = tile.get("type").and_then(Value::as_str) == Some("action");
    for field in ["action", "shiftAction"] {
        let value = tile.get(field).unwrap_or(&Value::Null);
        if !action && !value.is_null() {
            return Err(format!("{path}.{field} must be null on a param tile"));
        }
        validate_press_action(value, &format!("{path}.{field}"))?;
    }
    match tile.get("target").unwrap_or(&Value::Null) {
        Value::Null => Ok(()),
        Value::String(key) if !action && super::super::supported_aux_turn_key(key) => Ok(()),
        _ => Err(format!(
            "{path}.target must be null or a supported key on a param tile"
        )),
    }
}
