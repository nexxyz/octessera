use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, DeviceInput,
    TileTriggerType,
};
use crate::behaviors::native_impl::config_items::{action_item, number_item};
use crate::behaviors::native_impl::grid_transitions::TILE_COUNT;
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A world where no cell has moved on for this many ticks is frozen, so it starts afresh.
const STALL_TICKS: u8 = 8;
/// The state a resting cell moves into; its tiles light, so the waves show as moving fronts.
const FRONT: u8 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CyclicState {
    pub cells: Vec<u8>,
    pub ages: Vec<u8>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<TileTriggerType>,
    pub states: u8,
    pub threshold: u8,
    pub range: u8,
}

pub fn cyclic_init(config: Value) -> Result<CyclicState, String> {
    let (mut state, has_cells) = state_from_config(config);
    if !has_cells {
        seed_cycle(&mut state);
    }
    state.trigger_types = triggers(&state.cells, &state.cells);
    Ok(state)
}

pub fn cyclic_deserialize(data: Value) -> Result<CyclicState, String> {
    let (mut state, has_cells) = state_from_config(data);
    if !has_cells {
        seed_cycle(&mut state);
    }
    state.trigger_types = triggers(&state.cells, &state.cells);
    Ok(state)
}

pub fn cyclic_serialize(state: &CyclicState) -> Result<Value, String> {
    let mut state = state.clone();
    normalize(&mut state);
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn cyclic_on_input(
    mut state: CyclicState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> CyclicState {
    normalize(&mut state);
    let previous = state.cells.clone();
    let pressed = match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let index = grid_index(x, y);
            state.cells[index] = FRONT;
            state.ages[index] = 0;
            Some(index)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "seedCycle" =>
        {
            seed_cycle(&mut state);
            None
        }
        _ => return state,
    };
    state.trigger_types = triggers(&previous, &state.cells);
    if let Some(index) = pressed {
        state.trigger_types[index] = TileTriggerType::Activate;
    }
    state
}

pub fn cyclic_on_tick(mut state: CyclicState, _context: &mut BehaviorContext) -> CyclicState {
    let previous = state.cells.clone();
    let mut next = state.cells.clone();
    let mut ages = state.ages.clone();
    let mut rng = crate::behavior_random::rng();
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            let target = (previous[index] + 1) % state.states;
            let support = count_neighbors(&previous, x, y, target, state.range);
            if support >= state.threshold || rng.gen_range(0..state.threshold) < support {
                next[index] = target;
                ages[index] = 0;
            } else {
                ages[index] = ages[index].saturating_add(1);
            }
        }
    }
    state.cells = next;
    state.ages = ages;
    if state.ages.iter().all(|age| *age >= STALL_TICKS) {
        seed_cycle(&mut state);
    }
    state.trigger_types = triggers(&previous, &state.cells);
    state
}

pub fn cyclic_render_model(state: &CyclicState) -> BehaviorRenderModel {
    let active = state.cells.iter().filter(|cell| **cell == FRONT).count();
    BehaviorRenderModel {
        name: "cyclic".into(),
        status_line: format!("S:{} T:{} A:{active}", state.states, state.threshold),
        tiles: state.cells.iter().map(|cell| *cell == FRONT).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [255, 120, 220],
            inactive: crate::palette::BLACK,
            stable: [80, 180, 255],
        },
        trigger_types: Some(state.trigger_types.clone()),
        tile_colors: None,
    }
}

pub fn cyclic_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("states", "States", 3, 8, 1),
        number_item("threshold", "Threshold", 1, 8, 1),
        number_item("range", "Range", 1, 2, 1),
        action_item("seedCycle", "Seed Cycle"),
    ]
}

fn state_from_config(config: Value) -> (CyclicState, bool) {
    let has_cells = array_field(&config, "cells").is_some();
    let states = number(field(&config, "states"), 4, 3, 8);
    let mut state = CyclicState {
        cells: normalize_cells(array_field(&config, "cells").unwrap_or_default(), states),
        ages: normalize_ages(array_field(&config, "ages").unwrap_or_default()),
        trigger_types: normalize_triggers(None),
        states,
        threshold: number(field(&config, "threshold"), 1, 1, 8),
        range: number(field(&config, "range"), 1, 1, 2),
    };
    normalize(&mut state);
    (state, has_cells)
}

fn field(config: &Value, key: &str) -> Option<Value> {
    config.as_object()?.get(key).cloned()
}

fn array_field(config: &Value, key: &str) -> Option<Vec<Value>> {
    config.as_object().and_then(|object| {
        object
            .get(key)
            .map(|value| value.as_array().cloned().unwrap_or_default())
    })
}

fn normalize(state: &mut CyclicState) {
    state.states = state.states.clamp(3, 8);
    state.threshold = state.threshold.clamp(1, 8);
    state.range = state.range.clamp(1, 2);
    state.cells = normalize_cells(
        state.cells.iter().map(|cell| Value::from(*cell)).collect(),
        state.states,
    );
    state.ages = normalize_ages(state.ages.iter().map(|age| Value::from(*age)).collect());
    state.trigger_types = normalize_triggers(Some(state.trigger_types.clone()));
}

fn normalize_cells(cells: Vec<Value>, states: u8) -> Vec<u8> {
    let mut cells = cells
        .into_iter()
        .map(|cell| (cell.as_u64().unwrap_or(0).min(255) as u8) % states)
        .collect::<Vec<_>>();
    cells.resize(TILE_COUNT, 0);
    cells.truncate(TILE_COUNT);
    cells
}

fn normalize_ages(ages: Vec<Value>) -> Vec<u8> {
    let mut ages = ages
        .into_iter()
        .map(|age| age.as_u64().unwrap_or(0).min(u8::MAX.into()) as u8)
        .collect::<Vec<_>>();
    ages.resize(TILE_COUNT, 0);
    ages.truncate(TILE_COUNT);
    ages
}

fn normalize_triggers(triggers: Option<Vec<TileTriggerType>>) -> Vec<TileTriggerType> {
    let mut triggers = triggers.unwrap_or_default();
    triggers.resize(TILE_COUNT, TileTriggerType::None);
    triggers.truncate(TILE_COUNT);
    triggers
}

fn number(value: Option<Value>, default: u8, min: u8, max: u8) -> u8 {
    match value {
        Some(value) => value
            .as_i64()
            .map(|value| value.clamp(i64::from(min), i64::from(max)) as u8)
            .or_else(|| {
                value
                    .as_u64()
                    .map(|value| value.clamp(u64::from(min), u64::from(max)) as u8)
            })
            .unwrap_or(default),
        None => default,
    }
}

/// Scatters every state across the grid: a fresh mix the waves can organize into spirals.
fn seed_cycle(state: &mut CyclicState) {
    let mut rng = crate::behavior_random::rng();
    for (cell, age) in state.cells.iter_mut().zip(&mut state.ages) {
        *cell = rng.gen_range(0..state.states);
        *age = 0;
    }
}

fn count_neighbors(cells: &[u8], x: usize, y: usize, target: u8, range: u8) -> u8 {
    let mut count = 0;
    let range = isize::from(range);
    for dy in -range..=range {
        for dx in -range..=range {
            if dx == 0 && dy == 0 {
                continue;
            }
            if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
                if nx < GRID_WIDTH && ny < GRID_HEIGHT && cells[grid_index(nx, ny)] == target {
                    count += 1;
                }
            }
        }
    }
    count
}

fn triggers(previous: &[u8], next: &[u8]) -> Vec<TileTriggerType> {
    (0..TILE_COUNT)
        .map(
            |index| match (previous[index] == FRONT, next[index] == FRONT) {
                (false, true) => TileTriggerType::Activate,
                (true, false) => TileTriggerType::Deactivate,
                (true, true) => TileTriggerType::Stable,
                (false, false) => TileTriggerType::None,
            },
        )
        .collect()
}

#[cfg(test)]
mod tests;
