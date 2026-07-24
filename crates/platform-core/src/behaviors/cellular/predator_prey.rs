use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod simulation;

use simulation::{
    reseed_extinct, starter, triggers, triggers_from_cells, triggers_from_cells_forced,
    triggers_with_deactivations, ActorStep, StepBuffers,
};

pub(super) const EMPTY: u8 = 0;
pub(super) const GRASS: u8 = 1;
pub(super) const HERBIVORE: u8 = 2;
pub(super) const PREDATOR: u8 = 3;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredatorPreyState {
    pub cells: Vec<u8>,
    pub energy: Vec<u8>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "grassGrowChancePct")]
    pub grass_grow_chance_pct: u8,
    #[serde(rename = "herbivoreReproducePct")]
    pub herbivore_reproduce_pct: u8,
    #[serde(rename = "predatorReproducePct")]
    pub predator_reproduce_pct: u8,
    #[serde(rename = "starveTicks")]
    pub starve_ticks: u8,
}

#[derive(Default, Deserialize)]
struct Config {
    cells: Option<Value>,
    energy: Option<Value>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Value>,
    #[serde(rename = "grassGrowChancePct")]
    grass_grow_chance_pct: Option<Value>,
    #[serde(rename = "herbivoreReproducePct")]
    herbivore_reproduce_pct: Option<Value>,
    #[serde(rename = "predatorReproducePct")]
    predator_reproduce_pct: Option<Value>,
    #[serde(rename = "starveTicks")]
    starve_ticks: Option<Value>,
}

pub fn predator_prey_init(config: Value) -> Result<PredatorPreyState, String> {
    let config: Config = serde_json::from_value(config).unwrap_or_default();
    let has_cells = config.cells.as_ref().is_some_and(|value| value.is_array());
    let mut state = PredatorPreyState {
        cells: norm_cells(config.cells.map(array_values).unwrap_or_default()),
        energy: norm_energy(config.energy.map(array_values).unwrap_or_default()),
        trigger_types: norm_triggers(parse_triggers(config.trigger_types)),
        grass_grow_chance_pct: num(config.grass_grow_chance_pct, 15, 100),
        herbivore_reproduce_pct: num(config.herbivore_reproduce_pct, 15, 100),
        predator_reproduce_pct: num(config.predator_reproduce_pct, 8, 100),
        starve_ticks: num(config.starve_ticks, 8, 32).max(1),
    };
    normalize(&mut state);
    if !has_cells {
        starter(&mut state);
    }
    state.trigger_types = triggers(&state.cells, &state.cells, &[], &[]);
    Ok(state)
}

pub fn predator_prey_deserialize(data: Value) -> Result<PredatorPreyState, String> {
    let config: Config = serde_json::from_value(data).unwrap_or_default();
    let has_cells = config.cells.as_ref().is_some_and(|value| value.is_array());
    let mut state = PredatorPreyState {
        cells: norm_cells(config.cells.map(array_values).unwrap_or_default()),
        energy: norm_energy(config.energy.map(array_values).unwrap_or_default()),
        trigger_types: norm_triggers(parse_triggers(config.trigger_types)),
        grass_grow_chance_pct: num(config.grass_grow_chance_pct, 15, 100),
        herbivore_reproduce_pct: num(config.herbivore_reproduce_pct, 15, 100),
        predator_reproduce_pct: num(config.predator_reproduce_pct, 8, 100),
        starve_ticks: num(config.starve_ticks, 8, 32).max(1),
    };
    normalize(&mut state);
    if !has_cells {
        starter(&mut state);
    }
    state.trigger_types = state
        .cells
        .iter()
        .map(|c| {
            if *c == EMPTY {
                CellTriggerType::None
            } else {
                CellTriggerType::Stable
            }
        })
        .collect();
    Ok(state)
}

pub fn predator_prey_serialize(state: &PredatorPreyState) -> Result<Value, String> {
    let mut state = state.clone();
    normalize(&mut state);
    serde_json::to_value(state).map_err(|e| e.to_string())
}

pub fn predator_prey_on_input(
    mut state: PredatorPreyState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> PredatorPreyState {
    normalize(&mut state);
    let prev = state.cells.clone();
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let i = grid_index(x, y);
            state.cells[i] = (state.cells[i] + 1) % 4;
            state.energy[i] = if state.cells[i] >= HERBIVORE {
                state.starve_ticks
            } else {
                0
            };
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "reseedEcosystem" =>
        {
            let forced = starter(&mut state);
            state.trigger_types = triggers_from_cells_forced(&prev, &state.cells, &forced);
            return state;
        }
        _ => return state,
    }
    state.trigger_types = triggers_from_cells(&prev, &state.cells);
    state
}

pub fn predator_prey_on_tick(
    mut state: PredatorPreyState,
    _context: &mut BehaviorContext,
) -> PredatorPreyState {
    let prev = state.cells.clone();
    let prev_energy = state.energy.clone();
    let mut next = vec![EMPTY; CELL_COUNT];
    let mut energy = vec![0; CELL_COUNT];
    let mut reserved = vec![false; CELL_COUNT];
    let mut eaten = vec![false; CELL_COUNT];
    let mut bursts = Vec::new();
    let mut force_activate = Vec::new();
    for i in 0..CELL_COUNT {
        if prev[i] == GRASS {
            next[i] = GRASS;
        }
    }
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let i = grid_index(x, y);
            if prev[i] == PREDATOR {
                let mut buffers = StepBuffers {
                    next: &mut next,
                    energy: &mut energy,
                    reserved: &mut reserved,
                };
                ActorStep::Predator.act(
                    i,
                    x,
                    y,
                    &prev,
                    &prev_energy,
                    &mut buffers,
                    &mut eaten,
                    &mut bursts,
                    &mut force_activate,
                    &state,
                );
            }
        }
    }
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let i = grid_index(x, y);
            if prev[i] == HERBIVORE && !eaten[i] {
                let mut buffers = StepBuffers {
                    next: &mut next,
                    energy: &mut energy,
                    reserved: &mut reserved,
                };
                ActorStep::Herbivore.act(
                    i,
                    x,
                    y,
                    &prev,
                    &prev_energy,
                    &mut buffers,
                    &mut eaten,
                    &mut bursts,
                    &mut force_activate,
                    &state,
                );
            }
        }
    }
    let mut rng = rand::thread_rng();
    for cell in next.iter_mut().take(CELL_COUNT) {
        if *cell == EMPTY && rng.gen_range(0..100) < state.grass_grow_chance_pct {
            *cell = GRASS;
        }
    }
    let nudged = nudge_static_visibility(&prev, &mut next, &mut energy);
    let relieved = relieve_full_grid(&mut next, &mut energy);
    let mut force_deactivate = relieved;
    if let Some(index) = nudged {
        force_deactivate.push(index);
    }
    state.cells = next;
    state.energy = energy;
    force_activate.extend(reseed_extinct(&mut state));
    state.trigger_types = triggers_with_deactivations(
        &prev,
        &state.cells,
        &bursts,
        &force_activate,
        &force_deactivate,
    );
    state
}

fn relieve_full_grid(cells: &mut [u8], energy: &mut [u8]) -> Vec<usize> {
    let empty_count = cells.iter().filter(|cell| **cell == EMPTY).count();
    if empty_count >= 3 {
        return Vec::new();
    }
    let target_relief = 4usize.saturating_sub(empty_count);
    let mut relieved = cells
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (*cell == GRASS).then_some(index))
        .take(target_relief)
        .collect::<Vec<_>>();
    if relieved.is_empty() {
        relieved = (0..CELL_COUNT).step_by(17).take(target_relief).collect();
    }
    for index in &relieved {
        cells[*index] = EMPTY;
        energy[*index] = 0;
    }
    relieved
}

fn nudge_static_visibility(previous: &[u8], cells: &mut [u8], energy: &mut [u8]) -> Option<usize> {
    if !previous
        .iter()
        .zip(cells.iter())
        .all(|(previous, next)| (*previous != EMPTY) == (*next != EMPTY))
    {
        return None;
    }
    if let Some(index) = cells.iter().position(|cell| *cell == EMPTY) {
        cells[index] = GRASS;
        return None;
    }
    if let Some(index) = cells.iter().position(|cell| *cell == GRASS) {
        cells[index] = EMPTY;
        energy[index] = 0;
        return Some(index);
    }
    None
}

pub fn predator_prey_render_model(state: &PredatorPreyState) -> BehaviorRenderModel {
    let g = state.cells.iter().filter(|c| **c == GRASS).count();
    let h = state.cells.iter().filter(|c| **c == HERBIVORE).count();
    let p = state.cells.iter().filter(|c| **c == PREDATOR).count();
    BehaviorRenderModel {
        name: "predator prey".into(),
        status_line: format!("G:{g} H:{h} P:{p}"),
        cells: state.cells.iter().map(|c| *c != EMPTY).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [255, 180, 80],
            inactive: crate::palette::BLACK,
            stable: crate::palette::GREEN,
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn predator_prey_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("grassGrowChancePct", "Grass Grow", 0, 100, 1),
        number_item("herbivoreReproducePct", "Herbivore Repro", 0, 100, 1),
        number_item("predatorReproducePct", "Predator Repro", 0, 100, 1),
        number_item("starveTicks", "Starve Ticks", 1, 32, 1),
        action_item("reseedEcosystem", "Reseed Ecosystem"),
    ]
}

fn normalize(s: &mut PredatorPreyState) {
    s.cells = norm_cells(s.cells.iter().map(|c| Value::from(*c)).collect());
    s.energy = norm_energy(s.energy.iter().map(|e| Value::from(*e)).collect());
    s.trigger_types = norm_triggers(Some(s.trigger_types.clone()));
    s.grass_grow_chance_pct = s.grass_grow_chance_pct.min(100);
    s.herbivore_reproduce_pct = s.herbivore_reproduce_pct.min(100);
    s.predator_reproduce_pct = s.predator_reproduce_pct.min(100);
    s.starve_ticks = s.starve_ticks.clamp(1, 32);
    for i in 0..CELL_COUNT {
        if s.cells[i] < HERBIVORE {
            s.energy[i] = 0
        } else {
            s.energy[i] = s.energy[i].clamp(1, s.starve_ticks)
        }
    }
}
fn num(v: Option<Value>, d: u8, m: u8) -> u8 {
    v.and_then(|v| v.as_u64())
        .map(|v| v.min(m.into()) as u8)
        .unwrap_or(d)
}
fn array_values(value: Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}
fn parse_triggers(value: Option<Value>) -> Option<Vec<CellTriggerType>> {
    value.and_then(|value| {
        value
            .as_array()?
            .iter()
            .map(|value| serde_json::from_value(value.clone()).ok())
            .collect()
    })
}
fn norm_cells(v: Vec<Value>) -> Vec<u8> {
    let mut o = v
        .into_iter()
        .map(|v| v.as_u64().unwrap_or(0).min(3) as u8)
        .collect::<Vec<_>>();
    o.resize(CELL_COUNT, EMPTY);
    o.truncate(CELL_COUNT);
    o
}
fn norm_energy(v: Vec<Value>) -> Vec<u8> {
    let mut o = v
        .into_iter()
        .map(|v| v.as_u64().unwrap_or(0).min(u8::MAX.into()) as u8)
        .collect::<Vec<_>>();
    o.resize(CELL_COUNT, 0);
    o.truncate(CELL_COUNT);
    o
}
fn norm_triggers(v: Option<Vec<CellTriggerType>>) -> Vec<CellTriggerType> {
    let mut o = v.unwrap_or_default();
    o.resize(CELL_COUNT, CellTriggerType::None);
    o.truncate(CELL_COUNT);
    o
}

#[cfg(test)]
mod tests;
