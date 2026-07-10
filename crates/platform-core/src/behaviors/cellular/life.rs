use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorConfigItemType, BehaviorContext,
    BehaviorRenderModel, CellTriggerType, DeviceInput,
};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const CELL_COUNT: usize = GRID_WIDTH * GRID_HEIGHT;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifeState {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<bool>,
    #[serde(default, skip_serializing)]
    pub generation: usize,
    #[serde(rename = "randomCellsPerTick")]
    pub random_cells_per_tick: usize,
    #[serde(rename = "randomTickInterval")]
    pub random_tick_interval: usize,
    #[serde(rename = "gliderSpawnInterval", default)]
    pub glider_spawn_interval: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(rename = "tickCounter", default, skip_serializing)]
    pub tick_counter: usize,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifeConfig {
    #[serde(rename = "randomCellsPerTick")]
    pub random_cells_per_tick: Option<usize>,
    #[serde(rename = "randomTickInterval")]
    pub random_tick_interval: Option<usize>,
    #[serde(rename = "gliderSpawnInterval")]
    pub glider_spawn_interval: Option<usize>,
    #[serde(rename = "spawnStep")]
    pub spawn_step: Option<usize>,
}

pub fn init(config: Value) -> Result<LifeState, String> {
    let config: LifeConfig = serde_json::from_value(config).unwrap_or_default();
    Ok(LifeState {
        width: GRID_WIDTH,
        height: GRID_HEIGHT,
        cells: vec![false; CELL_COUNT],
        generation: 0,
        random_cells_per_tick: config.random_cells_per_tick.unwrap_or(0),
        random_tick_interval: config.random_tick_interval.unwrap_or(1),
        glider_spawn_interval: config.glider_spawn_interval.unwrap_or(0),
        spawn_step: config.spawn_step.unwrap_or(0).min(63),
        tick_counter: 0,
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
    })
}

pub fn on_input(state: LifeState, input: DeviceInput, _context: &mut BehaviorContext) -> LifeState {
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "spawnRandom" =>
        {
            let mut next = state.clone();
            let mut rng = rand::thread_rng();
            for _ in 0..5 {
                let x = rng.gen_range(0..GRID_WIDTH);
                let y = rng.gen_range(0..GRID_HEIGHT);
                let index = grid_index(x, y);
                if !next.cells[index] {
                    next.cells[index] = true;
                    next.trigger_types[index] = CellTriggerType::Activate;
                }
            }
            next
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "spawnGlider" =>
        {
            let mut next = state.clone();
            spawn_glider(&mut next.cells, &mut next.trigger_types, 0, 0);
            next
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let mut next = state.clone();
            let index = grid_index(x, y);
            next.cells[index] = !next.cells[index];
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

    let next_tick_counter = state.tick_counter + 1;
    if state.random_cells_per_tick > 0
        && state.random_tick_interval > 0
        && (next_tick_counter - 1) % state.random_tick_interval
            == state.spawn_step % state.random_tick_interval
    {
        let mut rng = rand::thread_rng();
        for _ in 0..state.random_cells_per_tick {
            let x = rng.gen_range(0..GRID_WIDTH);
            let y = rng.gen_range(0..GRID_HEIGHT);
            let index = grid_index(x, y);
            if !next_cells[index] {
                next_cells[index] = true;
                trigger_types[index] = CellTriggerType::Activate;
            }
        }
    }

    if state.glider_spawn_interval > 0
        && (next_tick_counter - 1) % state.glider_spawn_interval
            == state.spawn_step % state.glider_spawn_interval
    {
        spawn_glider(&mut next_cells, &mut trigger_types, 0, 0);
    }

    LifeState {
        width: state.width,
        height: state.height,
        cells: next_cells,
        generation: state.generation + 1,
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
        palette: Default::default(),
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
    for (dx, dy) in [(1, 0), (2, 1), (0, 2), (1, 2), (2, 2)] {
        let x = origin_x + dx;
        let y = origin_y + dy;
        if x < GRID_WIDTH && y < GRID_HEIGHT {
            let index = grid_index(x, y);
            if !cells[index] {
                trigger_types[index] = CellTriggerType::Activate;
            }
            cells[index] = true;
        }
    }
}

pub fn serialize(state: &LifeState) -> Result<Value, String> {
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn deserialize(data: Value) -> Result<LifeState, String> {
    serde_json::from_value(data).map_err(|error| error.to_string())
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
