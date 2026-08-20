use crate::behavior::CellTriggerType;
use crate::grid::{GRID_HEIGHT, GRID_WIDTH};

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
