use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::config_items::{action_item, number_item};
use crate::behaviors::native_impl::grid_transitions::{trigger_types_from_cells, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AntAgent {
    pub x: usize,
    pub y: usize,
    pub dir: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AntState {
    pub ants: Vec<AntAgent>,
    pub cells: Vec<bool>,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "maxAnts")]
    pub max_ants: usize,
    #[serde(rename = "autoSpawnInterval")]
    pub auto_spawn_interval: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(rename = "tickCounter", default, skip_serializing, skip_deserializing)]
    pub tick_counter: usize,
}

pub fn ant_init(config: Value) -> Result<AntState, String> {
    Ok(from_value(&config))
}

fn random_ant() -> AntAgent {
    let mut rng = crate::behavior_random::rng();
    AntAgent {
        x: rng.gen_range(0..GRID_WIDTH),
        y: rng.gen_range(0..GRID_HEIGHT),
        dir: 0,
    }
}

fn ant_cells_with_agents(state: &AntState) -> Vec<bool> {
    let mut cells = state.cells.clone();
    for ant in &state.ants {
        cells[grid_index(ant.x, ant.y)] = true;
    }
    cells
}

pub fn ant_on_input(
    state: AntState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> AntState {
    let previous = ant_cells_with_agents(&state);
    let mut next = state.clone();
    if next.ants.len() >= next.max_ants {
        return state;
    }
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "spawnAnt" =>
        {
            next.ants.push(random_ant())
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            next.ants.push(AntAgent { x, y, dir: 0 })
        }
        _ => return state,
    }
    let cells = ant_cells_with_agents(&next);
    next.trigger_types = trigger_types_from_cells(&previous, &cells);
    next
}

pub fn ant_on_tick(state: AntState, _context: &mut BehaviorContext) -> AntState {
    let previous = state.cells.clone();
    let mut cells = state.cells.clone();
    let ants = state
        .ants
        .iter()
        .map(|ant| {
            let index = grid_index(ant.x, ant.y);
            let new_dir = if state.cells[index] {
                (ant.dir + 3) % 4
            } else {
                (ant.dir + 1) % 4
            };
            match new_dir {
                0 => AntAgent {
                    x: ant.x,
                    y: (ant.y + GRID_HEIGHT - 1) % GRID_HEIGHT,
                    dir: new_dir,
                },
                1 => AntAgent {
                    x: (ant.x + 1) % GRID_WIDTH,
                    y: ant.y,
                    dir: new_dir,
                },
                2 => AntAgent {
                    x: ant.x,
                    y: (ant.y + 1) % GRID_HEIGHT,
                    dir: new_dir,
                },
                _ => AntAgent {
                    x: (ant.x + GRID_WIDTH - 1) % GRID_WIDTH,
                    y: ant.y,
                    dir: new_dir,
                },
            }
        })
        .collect::<Vec<_>>();
    for ant in &state.ants {
        let index = grid_index(ant.x, ant.y);
        cells[index] = !cells[index];
    }
    let tick_counter = state.tick_counter.saturating_add(1);
    let mut ants = ants;
    if state.auto_spawn_interval > 0
        && state.tick_counter % state.auto_spawn_interval
            == state.spawn_step % state.auto_spawn_interval
        && ants.len() < state.max_ants
    {
        ants.push(scheduled_ant(tick_counter / state.auto_spawn_interval));
    }
    let trigger_types = trigger_types_from_cells(&previous, &cells);
    AntState {
        ants,
        cells,
        trigger_types,
        tick_counter,
        ..state
    }
}

pub fn ant_deserialize(data: Value) -> Result<AntState, String> {
    Ok(from_value(&data))
}

fn from_value(data: &Value) -> AntState {
    let max_ants = number_field(data, "maxAnts", 50, 1, 100);
    let has_saved_world = field_present(data, "ants") || field_present(data, "cells");
    let mut state = AntState {
        ants: if has_saved_world {
            normalize_ants(array_field(data, "ants"), max_ants)
        } else {
            vec![AntAgent { x: 3, y: 3, dir: 0 }]
        },
        cells: normalize_cells(array_field(data, "cells")),
        trigger_types: normalize_triggers(array_field(data, "triggerTypes")),
        max_ants,
        auto_spawn_interval: number_field(data, "autoSpawnInterval", 16, 0, 20),
        spawn_step: number_field(data, "spawnStep", 5, 0, 63),
        tick_counter: 0,
    };
    normalize(&mut state);
    state
}

fn normalize(state: &mut AntState) {
    state.cells.resize(CELL_COUNT, false);
    state.cells.truncate(CELL_COUNT);
    state
        .trigger_types
        .resize(CELL_COUNT, CellTriggerType::None);
    state.trigger_types.truncate(CELL_COUNT);
    state.max_ants = state.max_ants.clamp(1, 100);
    state.auto_spawn_interval = state.auto_spawn_interval.clamp(0, 20);
    state.spawn_step = state.spawn_step.clamp(0, 63);
    for ant in &mut state.ants {
        ant.x %= GRID_WIDTH;
        ant.y %= GRID_HEIGHT;
        ant.dir %= 4;
    }
    state.ants.truncate(state.max_ants);
}

fn field_present(data: &Value, key: &str) -> bool {
    data.as_object()
        .map(|object| object.contains_key(key))
        .unwrap_or(false)
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

fn normalize_ants(ants: Option<Vec<Value>>, max_ants: usize) -> Vec<AntAgent> {
    ants.unwrap_or_default()
        .into_iter()
        .filter_map(|ant| {
            let object = ant.as_object()?;
            Some(AntAgent {
                x: coordinate(object.get("x"), GRID_WIDTH),
                y: coordinate(object.get("y"), GRID_HEIGHT),
                dir: direction(object.get("dir")),
            })
        })
        .take(max_ants)
        .collect()
}

fn normalize_cells(cells: Option<Vec<Value>>) -> Vec<bool> {
    let mut cells = cells
        .unwrap_or_default()
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

fn coordinate(value: Option<&Value>, size: usize) -> usize {
    value
        .and_then(Value::as_u64)
        .map(|value| (value % size as u64) as usize)
        .unwrap_or(0)
}

fn direction(value: Option<&Value>) -> u8 {
    value
        .and_then(Value::as_u64)
        .map(|value| (value % 4) as u8)
        .unwrap_or(0)
}

fn scheduled_ant(step: usize) -> AntAgent {
    let starts = [
        AntAgent { x: 1, y: 1, dir: 0 },
        AntAgent { x: 6, y: 2, dir: 1 },
        AntAgent { x: 2, y: 6, dir: 2 },
        AntAgent { x: 5, y: 5, dir: 3 },
    ];
    starts[step % starts.len()].clone()
}

pub fn ant_render_model(state: &AntState) -> BehaviorRenderModel {
    let cells = ant_cells_with_agents(state);
    BehaviorRenderModel {
        name: "ant".into(),
        status_line: format!(
            "{} ant{}",
            state.ants.len(),
            if state.ants.len() == 1 { "" } else { "s" }
        ),
        cells,
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::BLACK,
            inactive: [80, 48, 24],
            stable: crate::palette::BLACK,
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn ant_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("maxAnts", "Max Ants", 1, 100, 1),
        number_item("autoSpawnInterval", "Spawn Interval", 0, 20, 1),
        number_item("spawnStep", "Spawn Step", 0, 63, 1),
        action_item("spawnAnt", "Spawn Ant"),
    ]
}

#[cfg(test)]
mod tests;
