use super::*;

pub(super) fn apply_trigger_probability_map_payload(target: &mut [String], map: &[Value]) {
    for (tile_index, value) in map.iter().take(GRID_WIDTH * GRID_HEIGHT).enumerate() {
        if let Some(value) = value.as_str() {
            if matches!(value, "zero" | "low" | "high" | "full") {
                if let Some(tile) = target.get_mut(tile_index) {
                    *tile = value.into();
                }
            }
        }
    }
}
