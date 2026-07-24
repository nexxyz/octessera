use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const EMPTY: u8 = 0;
const TREE: u8 = 1;
const BURNING: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForestFireState {
    pub cells: Vec<u8>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "treeDensityPct")]
    pub tree_density_pct: u8,
    #[serde(rename = "growChancePct")]
    pub grow_chance_pct: u8,
    #[serde(rename = "spreadChancePct")]
    pub spread_chance_pct: u8,
    #[serde(rename = "reseedThresholdPct")]
    pub reseed_threshold_pct: u8,
    #[serde(rename = "lightningChancePerThousand")]
    pub lightning_chance_per_thousand: u8,
}

pub fn forest_fire_init(config: Value) -> Result<ForestFireState, String> {
    let (mut state, has_cells) = state_from_config(&config);
    if !has_cells {
        reseed_if_needed(&mut state);
    }
    normalize_current_triggers(&mut state);
    Ok(state)
}

pub fn forest_fire_deserialize(data: Value) -> Result<ForestFireState, String> {
    let (mut state, has_cells) = state_from_config(&data);
    if !has_cells {
        reseed_if_needed(&mut state);
    }
    normalize_current_triggers(&mut state);
    Ok(state)
}

fn state_from_config(config: &Value) -> (ForestFireState, bool) {
    let has_cells = array_field(config, "cells").is_some();
    let state = ForestFireState {
        cells: normalize_cells(array_field(config, "cells").unwrap_or_default()),
        trigger_types: normalize_triggers(None),
        tree_density_pct: number_field(field(config, "treeDensityPct"), 34, 0, 100),
        grow_chance_pct: number_field(field(config, "growChancePct"), 5, 0, 100),
        spread_chance_pct: number_field(field(config, "spreadChancePct"), 70, 0, 100),
        reseed_threshold_pct: number_field(field(config, "reseedThresholdPct"), 5, 0, 100),
        lightning_chance_per_thousand: number_field(
            field(config, "lightningChancePerThousand"),
            1,
            0,
            20,
        ),
    };
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

fn number_field(value: Option<Value>, default: u8, min: u8, max: u8) -> u8 {
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

fn normalize_cells(cells: Vec<Value>) -> Vec<u8> {
    let mut cells = cells
        .into_iter()
        .map(|cell| {
            cell.as_u64()
                .unwrap_or(u64::from(EMPTY))
                .min(u64::from(BURNING)) as u8
        })
        .collect::<Vec<_>>();
    cells.resize(CELL_COUNT, EMPTY);
    cells.truncate(CELL_COUNT);
    cells
}

fn normalize_triggers(triggers: Option<Vec<CellTriggerType>>) -> Vec<CellTriggerType> {
    let mut triggers = triggers.unwrap_or_else(|| vec![CellTriggerType::None; CELL_COUNT]);
    triggers.resize(CELL_COUNT, CellTriggerType::None);
    triggers.truncate(CELL_COUNT);
    triggers
}

fn burning_neighbors(cells: &[u8], x: usize, y: usize) -> usize {
    let mut count = 0;
    let min_y = y.saturating_sub(1);
    let max_y = (y + 1).min(GRID_HEIGHT - 1);
    let min_x = x.saturating_sub(1);
    let max_x = (x + 1).min(GRID_WIDTH - 1);
    for ny in min_y..=max_y {
        for nx in min_x..=max_x {
            if (nx != x || ny != y) && cells[grid_index(nx, ny)] == BURNING {
                count += 1;
            }
        }
    }
    count
}

pub fn forest_fire_on_input(
    state: ForestFireState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> ForestFireState {
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            ignite_at(state, x, y)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "igniteRandom" =>
        {
            let mut rng = rand::thread_rng();
            ignite_at(
                state,
                rng.gen_range(0..GRID_WIDTH),
                rng.gen_range(0..GRID_HEIGHT),
            )
        }
        _ => state,
    }
}

fn ignite_at(mut state: ForestFireState, x: usize, y: usize) -> ForestFireState {
    let previous = state.cells.clone();
    let index = grid_index(x, y);
    state.cells[index] = BURNING;
    state.trigger_types = triggers_from_forest_cells(&previous, &state.cells);
    state
}

pub fn forest_fire_on_tick(
    state: ForestFireState,
    _context: &mut BehaviorContext,
) -> ForestFireState {
    let mut rng = rand::thread_rng();
    let mut cells = vec![EMPTY; CELL_COUNT];
    let mut trigger_types = vec![CellTriggerType::None; CELL_COUNT];

    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            let previous = state.cells[index];
            let next = match previous {
                BURNING => EMPTY,
                TREE => {
                    let catches = burning_neighbors(&state.cells, x, y) > 0
                        && rng.gen_range(0..100) < state.spread_chance_pct;
                    let lightning =
                        rng.gen_range(0..1000) < u16::from(state.lightning_chance_per_thousand);
                    if catches || lightning {
                        BURNING
                    } else {
                        TREE
                    }
                }
                _ => {
                    if rng.gen_range(0..100) < state.grow_chance_pct {
                        TREE
                    } else {
                        EMPTY
                    }
                }
            };
            cells[index] = next;
            trigger_types[index] = trigger_for(previous, next);
        }
    }

    let mut next = ForestFireState {
        cells,
        trigger_types,
        ..state
    };
    let before_reseed = next.cells.clone();
    reseed_if_needed(&mut next);
    if next.cells != before_reseed {
        next.trigger_types = triggers_from_forest_cells(&state.cells, &next.cells);
    }
    let before_relief = next.cells.clone();
    relieve_full_frame(&mut next);
    if next.cells != before_relief {
        next.trigger_types = triggers_from_forest_cells(&state.cells, &next.cells);
    }
    next
}

fn trigger_for(previous: u8, next: u8) -> CellTriggerType {
    match (previous, next) {
        (_, TREE) => CellTriggerType::Stable,
        (BURNING, BURNING) => CellTriggerType::None,
        (_, BURNING) => CellTriggerType::Activate,
        (TREE | BURNING, EMPTY) => CellTriggerType::Deactivate,
        _ => CellTriggerType::None,
    }
}

fn reseed_if_needed(state: &mut ForestFireState) {
    let mut live = state.cells.iter().filter(|cell| **cell != EMPTY).count();
    if live * 100 >= CELL_COUNT * state.reseed_threshold_pct as usize {
        return;
    }
    let target = (CELL_COUNT * state.tree_density_pct as usize / 100).max(1);
    let mut empty = state
        .cells
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (*cell == EMPTY).then_some(index))
        .collect::<Vec<_>>();
    let mut rng = rand::thread_rng();
    while live < target && !empty.is_empty() {
        let position = rng.gen_range(0..empty.len());
        let index = empty.swap_remove(position);
        state.cells[index] = TREE;
        live += 1;
    }
}

fn relieve_full_frame(state: &mut ForestFireState) {
    if state.reseed_threshold_pct == 0 || state.cells.contains(&EMPTY) {
        return;
    }
    for index in (0..CELL_COUNT).step_by(17).take(4) {
        state.cells[index] = EMPTY;
    }
}

fn normalize_current_triggers(state: &mut ForestFireState) {
    state.trigger_types = state
        .cells
        .iter()
        .map(|cell| match *cell {
            BURNING => CellTriggerType::Activate,
            TREE => CellTriggerType::Stable,
            _ => CellTriggerType::None,
        })
        .collect();
}

fn triggers_from_forest_cells(previous: &[u8], next: &[u8]) -> Vec<CellTriggerType> {
    (0..CELL_COUNT)
        .map(|index| trigger_for(*previous.get(index).unwrap_or(&EMPTY), next[index]))
        .collect()
}

pub fn forest_fire_render_model(state: &ForestFireState) -> BehaviorRenderModel {
    let trees = state.cells.iter().filter(|cell| **cell == TREE).count();
    let fires = state.cells.iter().filter(|cell| **cell == BURNING).count();
    BehaviorRenderModel {
        name: "forest fire".into(),
        status_line: format!("T:{trees} F:{fires}"),
        cells: state.cells.iter().map(|cell| *cell != EMPTY).collect(),
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::YELLOW,
            inactive: crate::palette::BLACK,
            stable: crate::palette::GREEN,
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn forest_fire_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("treeDensityPct", "Tree Density", 0, 100, 1),
        number_item("growChancePct", "Grow Chance", 0, 100, 1),
        number_item("spreadChancePct", "Spread Chance", 0, 100, 1),
        number_item("reseedThresholdPct", "Reseed Threshold", 0, 100, 1),
        number_item("lightningChancePerThousand", "Lightning", 0, 20, 1),
        action_item("igniteRandom", "Ignite Random"),
    ]
}

#[cfg(test)]
#[path = "forest_fire_tests.rs"]
mod tests;
