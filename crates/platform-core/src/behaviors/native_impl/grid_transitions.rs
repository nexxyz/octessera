use crate::behavior::TileTriggerType;
use crate::grid::{GRID_HEIGHT, GRID_WIDTH};

pub(crate) const TILE_COUNT: usize = GRID_WIDTH * GRID_HEIGHT;

pub(crate) fn trigger_types_from_tiles(previous: &[bool], next: &[bool]) -> Vec<TileTriggerType> {
    (0..TILE_COUNT)
        .map(|index| match (previous[index], next[index]) {
            (false, true) => TileTriggerType::Activate,
            (true, false) => TileTriggerType::Deactivate,
            (true, true) => TileTriggerType::Stable,
            (false, false) => TileTriggerType::None,
        })
        .collect()
}
