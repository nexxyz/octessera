use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorConfigItemType, BehaviorContext,
    BehaviorRenderModel, CellTriggerType, DeviceInput,
};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const CELL_COUNT: usize = GRID_WIDTH * GRID_HEIGHT;

fn trigger_types_from_cells(previous: &[bool], next: &[bool]) -> Vec<CellTriggerType> {
    (0..CELL_COUNT)
        .map(|index| match (previous[index], next[index]) {
            (false, true) => CellTriggerType::Activate,
            (true, false) => CellTriggerType::Deactivate,
            (true, true) | (false, false) => CellTriggerType::None,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifeState {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<bool>,
    #[serde(default, skip_serializing, skip_deserializing)]
    pub generation: usize,
    #[serde(rename = "randomCellsPerTick")]
    pub random_cells_per_tick: usize,
    #[serde(rename = "randomTickInterval")]
    pub random_tick_interval: usize,
    #[serde(rename = "gliderSpawnInterval", default)]
    pub glider_spawn_interval: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(rename = "tickCounter", default, skip_serializing, skip_deserializing)]
    pub tick_counter: usize,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
}

pub fn init(config: Value) -> Result<LifeState, String> {
    Ok(from_value(&config))
}

fn seed_glider(cells: &mut [bool], origin_x: usize, origin_y: usize) {
    for (dx, dy) in [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)] {
        let x = origin_x + dx;
        let y = origin_y + dy;
        if x < GRID_WIDTH && y < GRID_HEIGHT {
            cells[grid_index(x, y)] = true;
        }
    }
}

fn seed_blinker(cells: &mut [bool]) {
    for (x, y) in [(4, 5), (5, 5), (6, 5)] {
        cells[grid_index(x, y)] = true;
    }
}

pub fn on_input(state: LifeState, input: DeviceInput, _context: &mut BehaviorContext) -> LifeState {
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "spawnRandom" =>
        {
            let mut next = state.clone();
            spawn_random_cells(&mut next.cells, next.random_cells_per_tick);
            next.trigger_types = trigger_types_from_cells(&state.cells, &next.cells);
            next
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "spawnGlider" =>
        {
            let mut next = state.clone();
            spawn_glider(&mut next.cells, &mut next.trigger_types, 0, 0);
            next.trigger_types = trigger_types_from_cells(&state.cells, &next.cells);
            next
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let mut next = state.clone();
            let index = grid_index(x, y);
            next.cells[index] = !next.cells[index];
            next.trigger_types = vec![CellTriggerType::None; CELL_COUNT];
            next
        }
        _ => state,
    }
}

pub fn on_tick(state: LifeState, _context: &mut BehaviorContext) -> LifeState {
    let mut next_cells = state.cells.clone();
    let mut trigger_types = vec![CellTriggerType::None; CELL_COUNT];

    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            let alive = state.cells[index];
            let neighbors = count_neighbors(&state.cells, x, y);
            let next_alive = if alive {
                neighbors == 2 || neighbors == 3
            } else {
                neighbors == 3
            };
            next_cells[index] = next_alive;
            trigger_types[index] = match (alive, next_alive) {
                (false, true) => CellTriggerType::Activate,
                (true, false) => CellTriggerType::Deactivate,
                (true, true) => CellTriggerType::Stable,
                (false, false) => CellTriggerType::None,
            };
        }
    }

    let next_tick_counter = state.tick_counter.saturating_add(1);
    if state.random_cells_per_tick > 0
        && state.random_tick_interval > 0
        && state.tick_counter % state.random_tick_interval
            == state.spawn_step % state.random_tick_interval
    {
        let previous = next_cells.clone();
        spawn_random_cells(&mut next_cells, state.random_cells_per_tick);
        for index in 0..CELL_COUNT {
            if !previous[index] && next_cells[index] {
                trigger_types[index] = CellTriggerType::Activate;
            }
        }
    }

    if state.glider_spawn_interval > 0
        && state.tick_counter % state.glider_spawn_interval
            == state.spawn_step % state.glider_spawn_interval
    {
        spawn_glider(&mut next_cells, &mut trigger_types, 0, 0);
    }

    LifeState {
        width: state.width,
        height: state.height,
        cells: next_cells,
        generation: state.generation.saturating_add(1),
        random_cells_per_tick: state.random_cells_per_tick,
        random_tick_interval: state.random_tick_interval,
        glider_spawn_interval: state.glider_spawn_interval,
        spawn_step: state.spawn_step,
        tick_counter: next_tick_counter,
        trigger_types,
    }
}

pub fn render_model(state: &LifeState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "game of life".into(),
        status_line: format!("Gen {}", state.generation),
        cells: state.cells.clone(),
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::BEHAVIOR_PRIMARY_MAGENTA,
            inactive: crate::palette::BEHAVIOR_DIM_GREEN,
            stable: crate::palette::BEHAVIOR_PRIMARY_YELLOW,
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn config_menu(_state: &LifeState) -> Vec<BehaviorConfigItem> {
    vec![
        BehaviorConfigItem {
            key: "randomCellsPerTick".into(),
            label: "Spawn Count".into(),
            item_type: BehaviorConfigItemType::Number,
            min: Some(0),
            max: Some(20),
            step: Some(1),
            options: None,
        },
        BehaviorConfigItem {
            key: "randomTickInterval".into(),
            label: "Spawn Interval".into(),
            item_type: BehaviorConfigItemType::Number,
            min: Some(1),
            max: Some(20),
            step: Some(1),
            options: None,
        },
        BehaviorConfigItem {
            key: "gliderSpawnInterval".into(),
            label: "Glider Interval".into(),
            item_type: BehaviorConfigItemType::Number,
            min: Some(0),
            max: Some(20),
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
            key: "spawnRandom".into(),
            label: "Spawn Random".into(),
            item_type: BehaviorConfigItemType::Action,
            min: None,
            max: None,
            step: None,
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

fn spawn_glider(
    cells: &mut [bool],
    trigger_types: &mut [CellTriggerType],
    origin_x: usize,
    origin_y: usize,
) {
    let previous = cells.to_vec();
    seed_glider(cells, origin_x, origin_y);
    for index in 0..CELL_COUNT {
        if !previous[index] && cells[index] {
            trigger_types[index] = CellTriggerType::Activate;
        }
    }
}

pub fn serialize(state: &LifeState) -> Result<Value, String> {
    let mut state = state.clone();
    normalize(&mut state);
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn deserialize(data: Value) -> Result<LifeState, String> {
    Ok(from_value(&data))
}

fn from_value(data: &Value) -> LifeState {
    let cells = array_field(data, "cells")
        .map(normalize_cells)
        .unwrap_or_else(|| {
            let mut cells = vec![false; CELL_COUNT];
            seed_blinker(&mut cells);
            cells
        });
    let mut state = LifeState {
        width: GRID_WIDTH,
        height: GRID_HEIGHT,
        cells,
        generation: 0,
        random_cells_per_tick: number_field(data, "randomCellsPerTick", 0, 0, 20),
        random_tick_interval: number_field(data, "randomTickInterval", 1, 1, 20),
        glider_spawn_interval: number_field(data, "gliderSpawnInterval", 0, 0, 20),
        spawn_step: number_field(data, "spawnStep", 0, 0, 63),
        tick_counter: 0,
        trigger_types: normalize_triggers(array_field(data, "triggerTypes")),
    };
    normalize(&mut state);
    state
}

fn normalize(state: &mut LifeState) {
    state.width = GRID_WIDTH;
    state.height = GRID_HEIGHT;
    state.cells.resize(CELL_COUNT, false);
    state.cells.truncate(CELL_COUNT);
    state
        .trigger_types
        .resize(CELL_COUNT, CellTriggerType::None);
    state.trigger_types.truncate(CELL_COUNT);
    state.random_cells_per_tick = state.random_cells_per_tick.clamp(0, 20);
    state.random_tick_interval = state.random_tick_interval.clamp(1, 20);
    state.glider_spawn_interval = state.glider_spawn_interval.clamp(0, 20);
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

fn normalize_cells(cells: Vec<Value>) -> Vec<bool> {
    let mut cells = cells
        .into_iter()
        .map(|cell| cell.as_bool().unwrap_or(false))
        .collect::<Vec<_>>();
    cells.resize(CELL_COUNT, false);
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

fn spawn_random_cells(cells: &mut [bool], count: usize) {
    let mut available = cells
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (!*cell).then_some(index))
        .collect::<Vec<_>>();
    available.shuffle(&mut rand::thread_rng());
    for index in available.into_iter().take(count) {
        cells[index] = true;
    }
}

fn count_neighbors(cells: &[bool], x: usize, y: usize) -> usize {
    let mut count = 0;
    for offset_y in -1isize..=1 {
        for offset_x in -1isize..=1 {
            if offset_x == 0 && offset_y == 0 {
                continue;
            }
            let next_x = x as isize + offset_x;
            let next_y = y as isize + offset_y;
            if next_x < 0
                || next_x >= GRID_WIDTH as isize
                || next_y < 0
                || next_y >= GRID_HEIGHT as isize
            {
                continue;
            }
            if cells[grid_index(next_x as usize, next_y as usize)] {
                count += 1;
            }
        }
    }
    count
}

#[cfg(test)]
#[path = "life_tests.rs"]
mod life_tests;
