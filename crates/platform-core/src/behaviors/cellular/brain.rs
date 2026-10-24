use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::config_items::{action_item, number_item};
use crate::behaviors::native_impl::grid_transitions::{trigger_types_from_cells, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrainState {
    pub cells: Vec<u8>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub generation: usize,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "fireThreshold")]
    pub fire_threshold: usize,
    #[serde(rename = "randomSeedCells")]
    pub random_seed_cells: usize,
    #[serde(rename = "seedInterval")]
    pub seed_interval: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(rename = "tickCounter", default, skip_serializing, skip_deserializing)]
    pub tick_counter: usize,
}

pub fn brain_init(config: Value) -> Result<BrainState, String> {
    Ok(from_value(&config))
}

fn brain_neighbors(cells: &[u8], x: usize, y: usize) -> usize {
    let mut count = 0;
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = ((x as isize + dx + GRID_WIDTH as isize) % GRID_WIDTH as isize) as usize;
            let ny = ((y as isize + dy + GRID_HEIGHT as isize) % GRID_HEIGHT as isize) as usize;
            if cells[grid_index(nx, ny)] == 1 {
                count += 1;
            }
        }
    }
    count
}

pub fn brain_on_input(
    state: BrainState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> BrainState {
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "seedRandom" =>
        {
            let mut next = state.clone();
            if next.random_seed_cells == 0 {
                return state;
            }
            let previous = next.cells.clone();
            seed_random_cells(&mut next.cells, next.random_seed_cells);
            let previous = previous.iter().map(|cell| *cell == 1).collect::<Vec<_>>();
            let current = next.cells.iter().map(|cell| *cell == 1).collect::<Vec<_>>();
            next.trigger_types = trigger_types_from_cells(&previous, &current);
            next
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let mut next = state.clone();
            let index = grid_index(x, y);
            next.cells[index] = if next.cells[index] == 0 { 1 } else { 0 };
            let previous = state
                .cells
                .iter()
                .map(|cell| *cell == 1)
                .collect::<Vec<_>>();
            let current = next.cells.iter().map(|cell| *cell == 1).collect::<Vec<_>>();
            next.trigger_types = trigger_types_from_cells(&previous, &current);
            next
        }
        _ => state,
    }
}

pub fn brain_on_tick(state: BrainState, _context: &mut BehaviorContext) -> BrainState {
    let mut cells = vec![0; CELL_COUNT];
    let mut trigger_types = vec![CellTriggerType::None; CELL_COUNT];
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            let cell = state.cells[index];
            let neighbors = brain_neighbors(&state.cells, x, y);
            if cell == 0 && neighbors == state.fire_threshold {
                cells[index] = 1;
                trigger_types[index] = CellTriggerType::Activate;
            } else if cell == 1 {
                cells[index] = 2;
                trigger_types[index] = CellTriggerType::Deactivate;
            }
        }
    }
    let tick_counter = state.tick_counter.saturating_add(1);
    if state.seed_interval > 0
        && state.random_seed_cells > 0
        && state.tick_counter % state.seed_interval == state.spawn_step % state.seed_interval
    {
        let step = tick_counter / state.seed_interval;
        seed_scheduled_cells(
            &mut cells,
            &mut trigger_types,
            state.random_seed_cells,
            step,
        );
    }
    BrainState {
        cells,
        trigger_types,
        generation: state.generation.saturating_add(1),
        tick_counter,
        ..state
    }
}

pub fn brain_deserialize(data: Value) -> Result<BrainState, String> {
    Ok(from_value(&data))
}

fn from_value(data: &Value) -> BrainState {
    let mut state = BrainState {
        cells: normalize_cells(array_field(data, "cells")),
        generation: 0,
        trigger_types: normalize_triggers(array_field(data, "triggerTypes")),
        fire_threshold: number_field(data, "fireThreshold", 2, 1, 4),
        random_seed_cells: number_field(data, "randomSeedCells", 2, 0, 20),
        seed_interval: number_field(data, "seedInterval", 2, 0, 30),
        spawn_step: number_field(data, "spawnStep", 0, 0, 63),
        tick_counter: 0,
    };
    normalize(&mut state);
    state
}

fn normalize(state: &mut BrainState) {
    state.cells = normalize_cells(Some(
        state.cells.iter().map(|cell| Value::from(*cell)).collect(),
    ));
    state.trigger_types = normalize_triggers(Some(
        state
            .trigger_types
            .iter()
            .map(|trigger| serde_json::to_value(trigger).unwrap_or(Value::Null))
            .collect(),
    ));
    state.fire_threshold = state.fire_threshold.clamp(1, 4);
    state.random_seed_cells = state.random_seed_cells.clamp(0, 20);
    state.seed_interval = state.seed_interval.clamp(0, 30);
    state.spawn_step = state.spawn_step.clamp(0, 63);
}

fn array_field(data: &Value, key: &str) -> Option<Vec<Value>> {
    data.as_object().and_then(|object| {
        object
            .get(key)
            .map(|value| value.as_array().cloned().unwrap_or_default())
    })
}

fn number_field(data: &Value, key: &str, default: usize, min: usize, max: usize) -> usize {
    let Some(value) = data.as_object().and_then(|object| object.get(key)) else {
        return default;
    };
    if let Some(value) = value.as_i64() {
        return value.clamp(min as i64, max as i64) as usize;
    }
    value
        .as_u64()
        .map(|value| value.clamp(min as u64, max as u64) as usize)
        .unwrap_or(default)
}

fn normalize_cells(cells: Option<Vec<Value>>) -> Vec<u8> {
    let mut cells = cells
        .unwrap_or_default()
        .into_iter()
        .map(|cell| cell.as_u64().unwrap_or(0).min(2) as u8)
        .collect::<Vec<_>>();
    cells.resize(CELL_COUNT, 0);
    cells.truncate(CELL_COUNT);
    cells
}

fn normalize_triggers(triggers: Option<Vec<Value>>) -> Vec<CellTriggerType> {
    let mut triggers = triggers
        .unwrap_or_default()
        .into_iter()
        .map(|trigger| serde_json::from_value(trigger).unwrap_or(CellTriggerType::None))
        .collect::<Vec<_>>();
    triggers.resize(CELL_COUNT, CellTriggerType::None);
    triggers.truncate(CELL_COUNT);
    triggers
}

fn seed_random_cells(cells: &mut [u8], count: usize) {
    let mut available = cells
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (*cell == 0).then_some(index))
        .collect::<Vec<_>>();
    available.shuffle(&mut crate::behavior_random::rng());
    for index in available.into_iter().take(count) {
        cells[index] = 1;
    }
}

fn seed_scheduled_cells(
    cells: &mut [u8],
    trigger_types: &mut [CellTriggerType],
    count: usize,
    step: usize,
) {
    let start = step % CELL_COUNT;
    let mut seeded = 0;
    for offset in 0..CELL_COUNT {
        let index = (start + offset) % CELL_COUNT;
        if cells[index] == 0 {
            cells[index] = 1;
            trigger_types[index] = CellTriggerType::Activate;
            seeded += 1;
            if seeded == count {
                break;
            }
        }
    }
}

pub fn brain_render_model(state: &BrainState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "brain".into(),
        status_line: format!("Gen {}", state.generation),
        cells: state.cells.iter().map(|cell| *cell == 1).collect(),
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::WHITE,
            inactive: crate::palette::BLACK,
            stable: crate::palette::BLUE,
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn brain_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("fireThreshold", "Fire Threshold", 1, 4, 1),
        number_item("seedInterval", "Seed Interval", 0, 30, 1),
        number_item("spawnStep", "Spawn Step", 0, 63, 1),
        number_item("randomSeedCells", "Spawn Count", 0, 20, 1),
        action_item("seedRandom", "Seed Random"),
    ]
}

#[cfg(test)]
mod tests;
