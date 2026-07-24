use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{
    action_item, number_item, trigger_types_from_cells, CELL_COUNT,
};
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
    available.shuffle(&mut rand::thread_rng());
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
mod tests {
    use super::*;

    #[test]
    fn state_machine_cycles_firing_refractory_dead_with_threshold() {
        let mut state =
            brain_init(serde_json::json!({ "fireThreshold": 1, "randomSeedCells": 0 })).unwrap();
        state.cells[grid_index(1, 1)] = 1;
        let mut context = BehaviorContext::new(120.0);

        let next = brain_on_tick(state, &mut context);
        assert_eq!(next.cells[grid_index(0, 0)], 1);
        let next = brain_on_tick(next, &mut context);
        assert_eq!(next.cells[grid_index(0, 0)], 2);
        let next = brain_on_tick(next, &mut context);
        assert_eq!(next.cells[grid_index(0, 0)], 0);
    }

    #[test]
    fn fire_threshold_prevents_insufficient_neighbors() {
        let mut state =
            brain_init(serde_json::json!({ "fireThreshold": 3, "randomSeedCells": 0 })).unwrap();
        state.cells[grid_index(1, 1)] = 1;
        state.cells[grid_index(0, 1)] = 1;

        let next = brain_on_tick(state, &mut BehaviorContext::new(120.0));
        assert_eq!(next.cells[grid_index(0, 0)], 0);
    }

    #[test]
    fn render_input_and_config_menu_match_legacy_contract() {
        let state = brain_init(Value::Null).unwrap();
        let mut context = BehaviorContext::new(120.0);
        let toggled = brain_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
        assert_eq!(toggled.cells[grid_index(2, 3)], 1);
        assert_eq!(
            toggled.trigger_types[grid_index(2, 3)],
            CellTriggerType::Activate
        );
        let toggled = brain_on_input(toggled, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
        assert_eq!(toggled.cells[grid_index(2, 3)], 0);
        assert_eq!(
            toggled.trigger_types[grid_index(2, 3)],
            CellTriggerType::Deactivate
        );

        let mut state =
            brain_init(serde_json::json!({ "fireThreshold": 1, "randomSeedCells": 0 })).unwrap();
        state.cells[grid_index(5, 5)] = 1;
        let next = brain_on_tick(state, &mut context);
        let model = brain_render_model(&next);
        assert_eq!(model.cells.len(), CELL_COUNT);
        assert!(model.cells.iter().any(|cell| *cell));
        assert_eq!(model.trigger_types.as_ref().unwrap().len(), CELL_COUNT);

        let menu = brain_config_menu();
        assert_eq!(menu.len(), 5);
        assert_eq!(menu[0].key, "fireThreshold");
        assert_eq!(menu[1].key, "seedInterval");
        assert_eq!(menu[2].key, "spawnStep");
        assert_eq!(menu[3].key, "randomSeedCells");
        assert_eq!(menu[4].key, "seedRandom");
    }

    #[test]
    fn default_run_avoids_consecutive_empty_or_full_frames() {
        let mut state = brain_init(Value::Null).unwrap();
        let mut context = BehaviorContext::new(120.0);
        let mut empty_run = 0;
        let mut full_run = 0;

        for _ in 0..300 {
            state = brain_on_tick(state, &mut context);
            let model = brain_render_model(&state);
            empty_run = if model.cells.iter().all(|cell| !*cell) {
                empty_run + 1
            } else {
                0
            };
            full_run = if model.cells.iter().all(|cell| *cell) {
                full_run + 1
            } else {
                0
            };
            assert!(empty_run <= 1);
            assert!(full_run <= 1);
        }
    }

    #[test]
    fn malformed_saved_state_normalizes_vectors_parameters_and_counters() {
        let state = brain_deserialize(serde_json::json!({
            "cells": [1, 2, 9, "bad"],
            "triggerTypes": ["activate", "bad"],
            "fireThreshold": -1,
            "randomSeedCells": -1,
            "seedInterval": 999,
            "spawnStep": 999,
            "generation": u64::MAX,
            "tickCounter": u64::MAX
        }))
        .unwrap();

        assert_eq!(state.cells.len(), CELL_COUNT);
        assert_eq!(state.cells[..4], [1, 2, 2, 0]);
        assert_eq!(state.trigger_types.len(), CELL_COUNT);
        assert_eq!(state.trigger_types[0], CellTriggerType::Activate);
        assert_eq!(state.trigger_types[1], CellTriggerType::None);
        assert_eq!(state.fire_threshold, 1);
        assert_eq!(state.random_seed_cells, 0);
        assert_eq!(state.seed_interval, 30);
        assert_eq!(state.spawn_step, 63);
        assert_eq!(state.generation, 0);
        assert_eq!(state.tick_counter, 0);

        let next = brain_on_tick(state, &mut BehaviorContext::new(120.0));
        assert_eq!(next.cells.len(), CELL_COUNT);
        assert_eq!(next.trigger_types.len(), CELL_COUNT);
    }

    #[test]
    fn manual_and_scheduled_seeding_use_exact_configured_counts() {
        let mut context = BehaviorContext::new(120.0);
        let state = brain_init(serde_json::json!({
            "cells": [],
            "randomSeedCells": 4,
            "seedInterval": 0
        }))
        .unwrap();
        let seeded = brain_on_input(
            state,
            DeviceInput::BehaviorAction(BehaviorActionInput {
                action_type: "seedRandom".into(),
            }),
            &mut context,
        );
        assert_eq!(seeded.cells.iter().filter(|cell| **cell == 1).count(), 4);

        let zero = brain_init(serde_json::json!({
            "cells": [],
            "randomSeedCells": 0
        }))
        .unwrap();
        let zero = brain_on_input(
            zero.clone(),
            DeviceInput::BehaviorAction(BehaviorActionInput {
                action_type: "seedRandom".into(),
            }),
            &mut context,
        );
        assert_eq!(
            zero,
            brain_init(serde_json::json!({
                "cells": [],
                "randomSeedCells": 0
            }))
            .unwrap()
        );

        let scheduled = brain_init(serde_json::json!({
            "cells": [],
            "randomSeedCells": 20,
            "seedInterval": 1,
            "spawnStep": 0
        }))
        .unwrap();
        let scheduled = brain_on_tick(scheduled, &mut context);
        assert_eq!(
            scheduled.cells.iter().filter(|cell| **cell == 1).count(),
            20
        );
    }
}
