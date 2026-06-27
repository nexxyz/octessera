use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorConfigItemType, BehaviorContext,
    BehaviorRenderModel, CellTriggerType, DeviceInput,
};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const CELL_COUNT: usize = GRID_WIDTH * GRID_HEIGHT;
const GLIDER_OFFSETS: [(usize, usize); 5] = [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliderState {
    pub cells: Vec<bool>,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "spawnInterval")]
    pub spawn_interval: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(rename = "tickCounter", default)]
    pub tick_counter: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliderConfig {
    #[serde(rename = "spawnInterval")]
    pub spawn_interval: Option<usize>,
}

pub fn init(config: Value) -> Result<GliderState, String> {
    let config: GliderConfig = serde_json::from_value(config).unwrap_or_default();
    Ok(GliderState {
        cells: vec![false; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        spawn_interval: config.spawn_interval.unwrap_or(8),
        spawn_step: 0,
        tick_counter: 0,
    })
}

pub fn on_input(
    state: GliderState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> GliderState {
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "spawnGlider" =>
        {
            let mut next = state.clone();
            spawn_glider_random(&mut next.cells);
            next
        }
        DeviceInput::GridPress { x, y } => {
            let mut next = state.clone();
            let origin_x = x.min(GRID_WIDTH - 3);
            let origin_y = y.min(GRID_HEIGHT - 3);
            spawn_glider(&mut next.cells, origin_x, origin_y);
            next
        }
        _ => state,
    }
}

pub fn on_tick(state: GliderState, _context: &mut BehaviorContext) -> GliderState {
    let mut cells = state.cells.clone();
    let tick_counter = state.tick_counter + 1;
    if state.spawn_interval > 0
        && (tick_counter - 1) % state.spawn_interval == state.spawn_step % state.spawn_interval
    {
        spawn_glider_random(&mut cells);
    }

    let mut next = vec![false; CELL_COUNT];
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            let mut neighbors = 0;
            for offset_y in -1isize..=1 {
                for offset_x in -1isize..=1 {
                    if offset_x == 0 && offset_y == 0 {
                        continue;
                    }
                    let next_x = ((x as isize + offset_x + GRID_WIDTH as isize)
                        % GRID_WIDTH as isize) as usize;
                    let next_y = ((y as isize + offset_y + GRID_HEIGHT as isize)
                        % GRID_HEIGHT as isize) as usize;
                    if cells[grid_index(next_x, next_y)] {
                        neighbors += 1;
                    }
                }
            }
            next[index] = if cells[index] {
                neighbors == 2 || neighbors == 3
            } else {
                neighbors == 3
            };
        }
    }

    let mut trigger_types = vec![CellTriggerType::None; CELL_COUNT];
    for index in 0..CELL_COUNT {
        trigger_types[index] = match (state.cells[index], next[index]) {
            (true, true) => CellTriggerType::Stable,
            (false, true) => CellTriggerType::Activate,
            (true, false) => CellTriggerType::Deactivate,
            (false, false) => CellTriggerType::None,
        };
    }

    GliderState {
        cells: next,
        trigger_types,
        spawn_interval: state.spawn_interval,
        spawn_step: state.spawn_step,
        tick_counter,
    }
}

pub fn render_model(state: &GliderState) -> BehaviorRenderModel {
    let count = state.cells.iter().filter(|cell| **cell).count();
    BehaviorRenderModel {
        name: "glider".into(),
        status_line: format!("Cells: {count}"),
        cells: state.cells.clone(),
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn config_menu(_state: &GliderState) -> Vec<BehaviorConfigItem> {
    vec![
        BehaviorConfigItem {
            key: "spawnInterval".into(),
            label: "Spawn Interval".into(),
            item_type: BehaviorConfigItemType::Number,
            min: Some(0),
            max: Some(30),
            step: Some(1),
            options: None,
        },
        BehaviorConfigItem {
            key: "spawnStep".into(),
            label: "Spawn Step".into(),
            item_type: BehaviorConfigItemType::Number,
            min: Some(0),
            max: Some(63),
            step: Some(1),
            options: None,
        },
        BehaviorConfigItem {
            key: "spawnGlider".into(),
            label: "Spawn Glider".into(),
            item_type: BehaviorConfigItemType::Action,
            min: None,
            max: None,
            step: None,
            options: None,
        },
    ]
}

pub fn serialize(state: &GliderState) -> Result<Value, String> {
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn serialize_persistent(state: &GliderState) -> Result<Value, String> {
    Ok(json!({
        "cells": &state.cells,
        "triggerTypes": &state.trigger_types,
        "spawnInterval": state.spawn_interval,
        "spawnStep": state.spawn_step,
    }))
}

pub fn deserialize(data: Value) -> Result<GliderState, String> {
    serde_json::from_value(data).map_err(|error| error.to_string())
}

fn spawn_glider_random(cells: &mut [bool]) {
    let mut rng = rand::thread_rng();
    let origin_x = rng.gen_range(0..(GRID_WIDTH - 2));
    let origin_y = rng.gen_range(0..(GRID_HEIGHT - 2));
    spawn_glider(cells, origin_x, origin_y);
}

fn spawn_glider(cells: &mut [bool], origin_x: usize, origin_y: usize) {
    for (offset_x, offset_y) in GLIDER_OFFSETS {
        cells[grid_index(origin_x + offset_x, origin_y + offset_y)] = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_press_spawns_glider_shape() {
        let state = init(Value::Null).unwrap();
        let mut context = BehaviorContext::new(120.0);
        let next = on_input(state, DeviceInput::GridPress { x: 0, y: 0 }, &mut context);
        for (x, y) in GLIDER_OFFSETS {
            assert!(next.cells[grid_index(x, y)]);
        }
    }

    #[test]
    fn block_is_stable_and_spawn_interval_zero_disables_spawning() {
        let mut state = init(serde_json::json!({ "spawnInterval": 0 })).unwrap();
        for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
            state.cells[grid_index(x, y)] = true;
        }
        let next = on_tick(state.clone(), &mut BehaviorContext::new(120.0));
        assert_eq!(next.cells, state.cells);
        assert_eq!(
            next.trigger_types[grid_index(2, 2)],
            CellTriggerType::Stable
        );
    }

    #[test]
    fn render_config_and_serialization_match_contract() {
        let mut state = init(Value::Null).unwrap();
        state.cells[grid_index(1, 1)] = true;
        let model = render_model(&state);
        assert_eq!(model.name, "glider");
        assert!(model.status_line.contains("Cells:"));
        assert_eq!(model.trigger_types.as_ref().unwrap().len(), CELL_COUNT);

        let menu = config_menu(&state);
        assert_eq!(
            menu.iter()
                .map(|item| item.key.as_str())
                .collect::<Vec<_>>(),
            vec!["spawnInterval", "spawnStep", "spawnGlider"]
        );

        let raw = serialize(&state).unwrap();
        assert_eq!(deserialize(raw).unwrap(), state);
    }

    #[test]
    fn spawn_interval_and_spawn_step_control_auto_spawn() {
        let mut context = BehaviorContext::new(120.0);
        let state = init(serde_json::json!({ "spawnInterval": 2 })).unwrap();
        let first = on_tick(state.clone(), &mut context);
        assert!(first.cells.iter().any(|cell| *cell));

        let mut delayed = state;
        delayed.spawn_step = 1;
        let first = on_tick(delayed, &mut context);
        assert!(first.cells.iter().all(|cell| !cell));
        let second = on_tick(first, &mut context);
        assert!(second.cells.iter().any(|cell| *cell));
    }
}
