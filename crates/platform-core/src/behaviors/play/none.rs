use crate::behavior::{BehaviorContext, BehaviorRenderModel, DeviceInput};
use crate::grid::{GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const CELL_COUNT: usize = GRID_WIDTH * GRID_HEIGHT;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoneState {
    pub cells: Vec<bool>,
}

pub fn init(_config: Value) -> Result<NoneState, String> {
    Ok(NoneState {
        cells: vec![false; CELL_COUNT],
    })
}

pub fn on_input(
    state: NoneState,
    _input: DeviceInput,
    _context: &mut BehaviorContext,
) -> NoneState {
    state
}

pub fn on_tick(state: NoneState, _context: &mut BehaviorContext) -> NoneState {
    state
}

pub fn render_model(_state: &NoneState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "none".into(),
        status_line: "Idle".into(),
        cells: vec![false; CELL_COUNT],
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::GRAY,
            inactive: crate::palette::BLACK,
            stable: crate::palette::GRAY,
        },
        trigger_types: None,
    }
}

pub fn serialize(state: &NoneState) -> Result<Value, String> {
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn deserialize(data: Value) -> Result<NoneState, String> {
    serde_json::from_value(data).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_ignores_inputs_and_ticks() {
        let state = init(Value::Null).unwrap();
        assert_eq!(state.cells.len(), CELL_COUNT);
        assert!(state.cells.iter().all(|cell| !cell));

        let mut context = BehaviorContext::new(120.0);
        for input in [
            DeviceInput::GridPress { x: 2, y: 3 },
            DeviceInput::GridRelease { x: 2, y: 3 },
            DeviceInput::EncoderTurn { delta: 1, id: None },
        ] {
            assert_eq!(on_input(state.clone(), input, &mut context), state);
        }
        assert_eq!(on_tick(state.clone(), &mut context), state);
    }

    #[test]
    fn none_render_and_serialization_match_contract() {
        let state = init(Value::Null).unwrap();
        let model = render_model(&state);
        assert_eq!(model.name, "none");
        assert_eq!(model.status_line, "Idle");
        assert!(model.cells.iter().all(|cell| !cell));
        assert!(model.trigger_types.is_none());

        let raw = serialize(&state).unwrap();
        assert_eq!(deserialize(raw).unwrap(), state);
    }
}
