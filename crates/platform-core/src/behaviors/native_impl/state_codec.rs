use super::grid_transitions::TILE_COUNT;
use crate::behavior::TileTriggerType;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn serialize<T: Serialize>(state: &T) -> Result<Value, String> {
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn deserialize<T: for<'de> Deserialize<'de>>(data: Value) -> Result<T, String> {
    serde_json::from_value(data).map_err(|error| error.to_string())
}

/// Saved trigger types fitted to the grid: missing tiles are `None`, extra ones dropped.
pub(crate) fn restore_trigger_types(saved: Option<Vec<TileTriggerType>>) -> Vec<TileTriggerType> {
    let mut triggers = saved.unwrap_or_default();
    triggers.resize(TILE_COUNT, TileTriggerType::None);
    triggers
}
