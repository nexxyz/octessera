use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_DENSITY: u8 = 3;
const DEFAULT_BIRTH_CHANCE_PCT: u8 = 70;
const DEFAULT_FADE_CHANCE_PCT: u8 = 35;
const DEFAULT_STAR_LIFE: u8 = 8;
const DEFAULT_CLUSTER_BIAS_PCT: u8 = 40;
const DEFAULT_SEED: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwinkleState {
    pub cells: Vec<bool>,
    pub ages: Vec<u8>,
    #[serde(rename = "triggerTypes", default, skip_serializing, skip_deserializing)]
    pub trigger_types: Vec<CellTriggerType>,
    pub density: u8,
    #[serde(rename = "birthChancePct")]
    pub birth_chance_pct: u8,
    #[serde(rename = "fadeChancePct")]
    pub fade_chance_pct: u8,
    #[serde(rename = "starLife")]
    pub star_life: u8,
    #[serde(rename = "clusterBiasPct")]
    pub cluster_bias_pct: u8,
    pub seed: u16,
    #[serde(rename = "rngCounter")]
    pub rng_counter: u64,
}

pub fn twinkle_init(config: Value) -> Result<TwinkleState, String> {
    let (mut state, has_cells, removed) = state_from_value(&config);
    if !has_cells {
        reseed_default(&mut state);
        state.trigger_types = current_triggers(&state.cells);
    } else {
        state.trigger_types = current_triggers(&state.cells);
        for index in removed {
            state.trigger_types[index] = CellTriggerType::Deactivate;
        }
    }
    Ok(state)
}

pub fn twinkle_deserialize(data: Value) -> Result<TwinkleState, String> {
    let (mut state, has_cells, removed) = state_from_value(&data);
    if !has_cells {
        reseed_default(&mut state);
    }
    state.trigger_types = current_triggers(&state.cells);
    for index in removed {
        state.trigger_types[index] = CellTriggerType::Deactivate;
    }
    Ok(state)
}

pub fn twinkle_serialize(state: &TwinkleState) -> Result<Value, String> {
    let mut state = state.clone();
    let _ = normalize(&mut state);
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn twinkle_on_input(
    mut state: TwinkleState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> TwinkleState {
    let _ = normalize(&mut state);
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let previous = state.cells.clone();
            manual_press(&mut state, grid_index(x, y));
            state.trigger_types = transition_triggers(&previous, &state.cells);
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "reseedStars" =>
        {
            let previous = state.cells.clone();
            state.rng_counter = 0;
            reseed_default(&mut state);
            state.trigger_types = transition_triggers(&previous, &state.cells);
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "clearStars" =>
        {
            let previous = state.cells.clone();
            state.cells.fill(false);
            state.ages.fill(0);
            state.trigger_types = transition_triggers(&previous, &state.cells);
        }
        _ => {}
    }
    state
}

pub fn twinkle_on_tick(mut state: TwinkleState, _context: &mut BehaviorContext) -> TwinkleState {
    let previous = state.cells.clone();
    let previously_empty = previous.iter().map(|cell| !*cell).collect::<Vec<_>>();
    for (cell, age) in state.cells.iter().zip(state.ages.iter_mut()) {
        if *cell {
            *age = age.saturating_add(1);
        } else {
            *age = 0;
        }
    }

    let eligible_deaths = (0..CELL_COUNT)
        .filter(|index| state.cells[*index] && state.ages[*index] >= state.star_life)
        .collect::<Vec<_>>();
    let fade_chance_pct = state.fade_chance_pct;
    if !eligible_deaths.is_empty() && chance(&mut state, fade_chance_pct) {
        let index = choose_index(&mut state, &eligible_deaths);
        state.cells[index] = false;
        state.ages[index] = 0;
    }

    let live_count = state.cells.iter().filter(|cell| **cell).count();
    let birth_chance_pct = state.birth_chance_pct;
    if live_count < usize::from(state.density) && chance(&mut state, birth_chance_pct) {
        if let Some(index) = choose_birth(&mut state, &previous, &previously_empty) {
            state.cells[index] = true;
            state.ages[index] = 0;
        }
    }

    state.trigger_types = transition_triggers(&previous, &state.cells);
    state
}

pub fn twinkle_render_model(state: &TwinkleState) -> BehaviorRenderModel {
    let stars = state.cells.iter().filter(|cell| **cell).count();
    BehaviorRenderModel {
        name: "twinkle".into(),
        status_line: format!("S:{stars}/{}", state.density),
        cells: state.cells.clone(),
        palette: crate::BehaviorRenderPalette {
            active: [80, 170, 255],
            inactive: crate::palette::BLACK,
            stable: [18, 48, 82],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn twinkle_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("density", "Density", 1, 5, 1),
        number_item("birthChancePct", "Birth Chance", 0, 100, 1),
        number_item("fadeChancePct", "Fade Chance", 0, 100, 1),
        number_item("starLife", "Star Life", 1, 32, 1),
        number_item("clusterBiasPct", "Cluster Bias", 0, 100, 1),
        number_item("seed", "Seed", 0, 65535, 1),
        action_item("reseedStars", "Reseed Stars"),
        action_item("clearStars", "Clear Stars"),
    ]
}

fn state_from_value(value: &Value) -> (TwinkleState, bool, Vec<usize>) {
    let has_cells = value
        .as_object()
        .and_then(|object| object.get("cells"))
        .is_some_and(Value::is_array);
    let state = TwinkleState {
        cells: normalize_cells(array_field(value, "cells")),
        ages: normalize_ages(array_field(value, "ages")),
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        density: number_field(value, "density", u64::from(DEFAULT_DENSITY), 1, 5) as u8,
        birth_chance_pct: number_field(
            value,
            "birthChancePct",
            u64::from(DEFAULT_BIRTH_CHANCE_PCT),
            0,
            100,
        ) as u8,
        fade_chance_pct: number_field(
            value,
            "fadeChancePct",
            u64::from(DEFAULT_FADE_CHANCE_PCT),
            0,
            100,
        ) as u8,
        star_life: number_field(value, "starLife", u64::from(DEFAULT_STAR_LIFE), 1, 32) as u8,
        cluster_bias_pct: number_field(
            value,
            "clusterBiasPct",
            u64::from(DEFAULT_CLUSTER_BIAS_PCT),
            0,
            100,
        ) as u8,
        seed: number_field(
            value,
            "seed",
            u64::from(DEFAULT_SEED),
            0,
            u64::from(u16::MAX),
        ) as u16,
        rng_counter: number_field(value, "rngCounter", 0, 0, u64::MAX),
    };
    let mut state = state;
    let removed = normalize(&mut state);
    (state, has_cells, removed)
}

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_object().and_then(|object| object.get(key))
}

fn array_field(value: &Value, key: &str) -> Option<Vec<Value>> {
    field(value, key).and_then(|value| value.as_array().cloned())
}

fn number_field(value: &Value, key: &str, default: u64, min: u64, max: u64) -> u64 {
    let Some(value) = field(value, key) else {
        return default;
    };
    if let Some(value) = value.as_i64() {
        return if value < 0 {
            min
        } else {
            (value as u64).clamp(min, max)
        };
    }
    value
        .as_u64()
        .map(|value| value.clamp(min, max))
        .unwrap_or(default)
}

fn normalize(state: &mut TwinkleState) -> Vec<usize> {
    state.density = state.density.clamp(1, 5);
    state.birth_chance_pct = state.birth_chance_pct.min(100);
    state.fade_chance_pct = state.fade_chance_pct.min(100);
    state.star_life = state.star_life.clamp(1, 32);
    state.cluster_bias_pct = state.cluster_bias_pct.min(100);
    normalize_vectors(state);
    let mut active = 0;
    let mut removed = Vec::new();
    for index in 0..CELL_COUNT {
        if state.cells[index] {
            if active == usize::from(state.density) {
                state.cells[index] = false;
                state.ages[index] = 0;
                removed.push(index);
            } else {
                active += 1;
            }
        } else {
            state.ages[index] = 0;
        }
    }
    removed
}

fn normalize_vectors(state: &mut TwinkleState) {
    state.cells.resize(CELL_COUNT, false);
    state.cells.truncate(CELL_COUNT);
    state.ages.resize(CELL_COUNT, 0);
    state.ages.truncate(CELL_COUNT);
    state
        .trigger_types
        .resize(CELL_COUNT, CellTriggerType::None);
    state.trigger_types.truncate(CELL_COUNT);
}

fn normalize_cells(values: Option<Vec<Value>>) -> Vec<bool> {
    let mut cells = values
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.as_bool().unwrap_or(false))
        .collect::<Vec<_>>();
    cells.resize(CELL_COUNT, false);
    cells.truncate(CELL_COUNT);
    cells
}

fn normalize_ages(values: Option<Vec<Value>>) -> Vec<u8> {
    let mut ages = values
        .unwrap_or_default()
        .into_iter()
        .map(|value| {
            value
                .as_i64()
                .map(|value| value.clamp(0, i64::from(u8::MAX)) as u8)
                .or_else(|| {
                    value
                        .as_u64()
                        .map(|value| value.min(u64::from(u8::MAX)) as u8)
                })
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    ages.resize(CELL_COUNT, 0);
    ages.truncate(CELL_COUNT);
    ages
}

fn current_triggers(cells: &[bool]) -> Vec<CellTriggerType> {
    cells
        .iter()
        .map(|cell| {
            if *cell {
                CellTriggerType::Stable
            } else {
                CellTriggerType::None
            }
        })
        .collect()
}

fn transition_triggers(previous: &[bool], next: &[bool]) -> Vec<CellTriggerType> {
    (0..CELL_COUNT)
        .map(|index| match (previous[index], next[index]) {
            (false, true) => CellTriggerType::Activate,
            (true, false) => CellTriggerType::Deactivate,
            (true, true) => CellTriggerType::Stable,
            (false, false) => CellTriggerType::None,
        })
        .collect()
}

fn manual_press(state: &mut TwinkleState, index: usize) {
    if state.cells[index] {
        state.cells[index] = false;
        state.ages[index] = 0;
    } else if state.cells.iter().filter(|cell| **cell).count() < usize::from(state.density) {
        state.cells[index] = true;
        state.ages[index] = 0;
    } else if let Some(replaced) = state.cells.iter().position(|cell| *cell) {
        state.cells[replaced] = false;
        state.ages[replaced] = 0;
        state.cells[index] = true;
        state.ages[index] = 0;
    }
}

fn reseed_default(state: &mut TwinkleState) {
    state.cells.fill(false);
    state.ages.fill(0);
    let mut used = [false; CELL_COUNT];
    let mut seeded = 0;
    let mut attempt = 0u64;
    while seeded < usize::from(state.density) {
        let index =
            (splitmix64(u64::from(state.seed).wrapping_add(attempt)) % CELL_COUNT as u64) as usize;
        attempt = attempt.wrapping_add(1);
        if !used[index] {
            used[index] = true;
            state.cells[index] = true;
            seeded += 1;
        }
    }
}

fn choose_birth(
    state: &mut TwinkleState,
    previous: &[bool],
    previously_empty: &[bool],
) -> Option<usize> {
    let global = empty_indices(previously_empty);
    if global.is_empty() {
        return None;
    }
    let cluster_bias_pct = state.cluster_bias_pct;
    if chance(state, cluster_bias_pct) {
        let clustered = clustered_empty_indices(previous);
        if !clustered.is_empty() {
            return Some(choose_index(state, &clustered));
        }
    }
    Some(choose_index(state, &global))
}

fn empty_indices(empty: &[bool]) -> Vec<usize> {
    empty
        .iter()
        .enumerate()
        .filter_map(|(index, empty)| (*empty).then_some(index))
        .collect()
}

fn clustered_empty_indices(previous: &[bool]) -> Vec<usize> {
    let mut clustered = Vec::new();
    for index in 0..CELL_COUNT {
        if previous[index] {
            let x = index % GRID_WIDTH;
            let y = index / GRID_WIDTH;
            for dy in -1isize..=1 {
                for dx in -1isize..=1 {
                    let Some(nx) = x.checked_add_signed(dx) else {
                        continue;
                    };
                    let Some(ny) = y.checked_add_signed(dy) else {
                        continue;
                    };
                    if nx >= GRID_WIDTH || ny >= GRID_HEIGHT {
                        continue;
                    }
                    let candidate = grid_index(nx, ny);
                    if !previous[candidate] && !clustered.contains(&candidate) {
                        clustered.push(candidate);
                    }
                }
            }
        }
    }
    clustered.sort_unstable();
    clustered
}

fn choose_index(state: &mut TwinkleState, values: &[usize]) -> usize {
    let offset = (next_random(state) % values.len() as u64) as usize;
    values[offset]
}

fn chance(state: &mut TwinkleState, percentage: u8) -> bool {
    next_random(state) % 100 < u64::from(percentage)
}

fn next_random(state: &mut TwinkleState) -> u64 {
    let counter = state.rng_counter;
    state.rng_counter = counter.wrapping_add(1);
    splitmix64((u64::from(state.seed) << 32) ^ counter)
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[cfg(test)]
#[path = "twinkle_tests.rs"]
mod tests;
