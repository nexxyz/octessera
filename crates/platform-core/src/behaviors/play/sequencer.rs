use crate::behavior::{BehaviorContext, BehaviorRenderModel, DeviceInput};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const CELL_COUNT: usize = GRID_WIDTH * GRID_HEIGHT;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequencerState {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<bool>,
}

pub fn init(_config: Value) -> Result<SequencerState, String> {
    Ok(SequencerState {
        width: GRID_WIDTH,
        height: GRID_HEIGHT,
        cells: vec![false; CELL_COUNT],
    })
}

pub fn on_input(
    state: SequencerState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> SequencerState {
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let mut next = state.clone();
            let index = grid_index(x, y);
            next.cells[index] = !next.cells[index];
            next
        }
        _ => state,
    }
}

pub fn on_tick(state: SequencerState, _context: &mut BehaviorContext) -> SequencerState {
    state
}

pub fn render_model(state: &SequencerState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "sequencer".into(),
        status_line: "Manual".into(),
        cells: state.cells.clone(),
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::WHITE,
            inactive: crate::palette::BLACK,
            stable: crate::palette::YELLOW,
        },
        trigger_types: None,
    }
}

pub fn serialize(state: &SequencerState) -> Result<Value, String> {
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn deserialize(data: Value) -> Result<SequencerState, String> {
    serde_json::from_value(data).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_press_toggles_cell_and_tick_is_static() {
        let state = init(Value::Null).unwrap();
        let mut context = BehaviorContext::new(120.0);
        let toggled = on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
        assert!(toggled.cells[grid_index(2, 3)]);

        let untoggled = on_input(
            toggled.clone(),
            DeviceInput::GridPress { x: 2, y: 3 },
            &mut context,
        );
        assert!(!untoggled.cells[grid_index(2, 3)]);
        assert_eq!(on_tick(toggled.clone(), &mut context), toggled);
    }

    #[test]
    fn render_model_matches_ts_behavior_contract() {
        let state = init(Value::Null).unwrap();
        let model = render_model(&state);
        assert_eq!(model.name, "sequencer");
        assert_eq!(model.status_line, "Manual");
        assert_eq!(model.cells.len(), CELL_COUNT);
    }
}
