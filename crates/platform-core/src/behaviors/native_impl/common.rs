use crate::behavior::{BehaviorConfigItem, BehaviorConfigItemType, CellTriggerType};
use crate::grid::{GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const CELL_COUNT: usize = GRID_WIDTH * GRID_HEIGHT;

pub(crate) fn trigger_types_from_cells(previous: &[bool], next: &[bool]) -> Vec<CellTriggerType> {
    (0..CELL_COUNT)
        .map(|index| match (previous[index], next[index]) {
            (false, true) => CellTriggerType::Activate,
            (true, false) => CellTriggerType::Deactivate,
            (true, true) => CellTriggerType::Stable,
            (false, false) => CellTriggerType::None,
        })
        .collect()
}

pub(crate) fn number_item(
    key: &str,
    label: &str,
    min: i32,
    max: i32,
    step: i32,
) -> BehaviorConfigItem {
    BehaviorConfigItem {
        key: key.into(),
        label: label.into(),
        item_type: BehaviorConfigItemType::Number,
        min: Some(min),
        max: Some(max),
        step: Some(step),
        options: None,
    }
}

pub(crate) fn enum_item(key: &str, label: &str, options: &[&str]) -> BehaviorConfigItem {
    BehaviorConfigItem {
        key: key.into(),
        label: label.into(),
        item_type: BehaviorConfigItemType::Enum,
        min: None,
        max: None,
        step: None,
        options: Some(options.iter().map(|option| (*option).to_string()).collect()),
    }
}

pub(crate) fn action_item(key: &str, label: &str) -> BehaviorConfigItem {
    BehaviorConfigItem {
        key: key.into(),
        label: label.into(),
        item_type: BehaviorConfigItemType::Action,
        min: None,
        max: None,
        step: None,
        options: None,
    }
}

pub fn serialize<T: Serialize>(state: &T) -> Result<Value, String> {
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn deserialize<T: for<'de> Deserialize<'de>>(data: Value) -> Result<T, String> {
    serde_json::from_value(data).map_err(|error| error.to_string())
}
