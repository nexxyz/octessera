use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, enum_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const EMPTY: u8 = 0;
const MAX_PHASE: u8 = 4;
const SYMMETRIES: &[&str] = &["cross", "diagonal", "snowflake"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalGrowthState {
    pub cells: Vec<u8>,
    pub ages: Vec<u16>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "growthChancePct")]
    pub growth_chance_pct: u8,
    #[serde(rename = "seedInterval")]
    pub seed_interval: u8,
    #[serde(rename = "seedStep")]
    pub seed_step: u8,
    #[serde(rename = "cellLife")]
    pub cell_life: u16,
    pub symmetry: String,
    #[serde(rename = "tickCounter", skip_serializing)]
    pub tick_counter: u64,
}

#[derive(Default, Deserialize)]
struct CrystalGrowthConfig {
    cells: Option<Vec<Value>>,
    ages: Option<Vec<Value>>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "growthChancePct")]
    growth_chance_pct: Option<Value>,
    #[serde(rename = "seedInterval")]
    seed_interval: Option<Value>,
    #[serde(rename = "seedStep")]
    seed_step: Option<Value>,
    #[serde(rename = "cellLife")]
    cell_life: Option<Value>,
    symmetry: Option<String>,
    #[serde(rename = "tickCounter")]
    tick_counter: Option<Value>,
}

pub fn crystal_growth_init(config: Value) -> Result<CrystalGrowthState, String> {
    let config: CrystalGrowthConfig = serde_json::from_value(config).unwrap_or_default();
    let mut state = CrystalGrowthState {
        cells: normalize_cells(config.cells.unwrap_or_default()),
        ages: normalize_ages(config.ages.unwrap_or_default()),
        trigger_types: normalize_triggers(config.trigger_types),
        growth_chance_pct: number_u8(config.growth_chance_pct, 45, 100),
        seed_interval: number_u8(config.seed_interval, 16, 64),
        seed_step: number_u8(config.seed_step, 0, 63),
        cell_life: number_u16(config.cell_life, 128, 256),
        symmetry: normalize_symmetry(config.symmetry),
        tick_counter: config.tick_counter.and_then(|v| v.as_u64()).unwrap_or(0),
    };
    normalize_state(&mut state);
    force_reseed_if_empty(&mut state, &[]);
    let empty = [false; CELL_COUNT];
    state.trigger_types = triggers_from_visible(&empty, &visible(&state.cells));
    Ok(state)
}

pub fn crystal_growth_on_input(
    mut state: CrystalGrowthState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> CrystalGrowthState {
    normalize_state(&mut state);
    let previous = visible(&state.cells);
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            seed_at(&mut state, grid_index(x, y));
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "seedCrystal" =>
        {
            seed_random(&mut state);
        }
        _ => return state,
    }
    state.trigger_types = triggers_from_visible(&previous, &visible(&state.cells));
    state
}

pub fn crystal_growth_on_tick(
    mut state: CrystalGrowthState,
    _context: &mut BehaviorContext,
) -> CrystalGrowthState {
    normalize_state(&mut state);
    let previous = visible(&state.cells);
    age_and_dissolve(&mut state);
    let surviving = visible(&state.cells);
    grow_candidates(&mut state, &surviving);
    scheduled_seed(&mut state);
    force_reseed_if_empty(&mut state, &previous);
    state.tick_counter = state.tick_counter.saturating_add(1);
    state.trigger_types = triggers_from_visible(&previous, &visible(&state.cells));
    state
}

pub fn crystal_growth_render_model(state: &CrystalGrowthState) -> BehaviorRenderModel {
    let count = state.cells.iter().filter(|cell| **cell != EMPTY).count();
    BehaviorRenderModel {
        name: "crystal growth".into(),
        status_line: format!("Cr:{count}"),
        cells: visible(&state.cells),
        palette: crate::BehaviorRenderPalette {
            active: [220, 255, 255],
            inactive: crate::palette::BLACK,
            stable: [40, 180, 255],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn crystal_growth_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("growthChancePct", "Growth Chance", 0, 100, 1),
        number_item("seedInterval", "Seed Interval", 0, 64, 1),
        number_item("seedStep", "Seed Step", 0, 63, 1),
        number_item("cellLife", "Cell Life", 0, 256, 1),
        enum_item("symmetry", "Symmetry", SYMMETRIES),
        action_item("seedCrystal", "Seed Crystal"),
    ]
}

pub fn crystal_growth_deserialize(data: Value) -> Result<CrystalGrowthState, String> {
    let mut state = crystal_growth_init(data)?;
    state.trigger_types = visible(&state.cells)
        .into_iter()
        .map(|cell| {
            if cell {
                CellTriggerType::Stable
            } else {
                CellTriggerType::None
            }
        })
        .collect();
    Ok(state)
}

pub fn crystal_growth_serialize(state: &CrystalGrowthState) -> Result<Value, String> {
    let mut state = state.clone();
    normalize_state(&mut state);
    serde_json::to_value(state).map_err(|error| error.to_string())
}

fn normalize_state(state: &mut CrystalGrowthState) {
    state.cells = normalize_cells(state.cells.iter().map(|cell| Value::from(*cell)).collect());
    state.ages = normalize_ages(state.ages.iter().map(|age| Value::from(*age)).collect());
    state.trigger_types = normalize_triggers(Some(state.trigger_types.clone()));
    state.growth_chance_pct = state.growth_chance_pct.min(100);
    state.seed_interval = state.seed_interval.min(64);
    state.seed_step = state.seed_step.min(63);
    state.cell_life = state.cell_life.min(256);
    state.symmetry = normalize_symmetry(Some(state.symmetry.clone()));
    for index in 0..CELL_COUNT {
        if state.cells[index] == EMPTY {
            state.ages[index] = 0;
        }
    }
}

fn number_u8(value: Option<Value>, default: u8, max: u8) -> u8 {
    value
        .and_then(|v| v.as_u64())
        .map(|v| v.min(max.into()) as u8)
        .unwrap_or(default)
}

fn number_u16(value: Option<Value>, default: u16, max: u16) -> u16 {
    value
        .and_then(|v| v.as_u64())
        .map(|v| v.min(max.into()) as u16)
        .unwrap_or(default)
}

fn normalize_cells(cells: Vec<Value>) -> Vec<u8> {
    let mut cells = cells
        .into_iter()
        .map(|cell| cell.as_u64().unwrap_or(0).min(MAX_PHASE.into()) as u8)
        .collect::<Vec<_>>();
    cells.resize(CELL_COUNT, EMPTY);
    cells.truncate(CELL_COUNT);
    cells
}

fn normalize_ages(ages: Vec<Value>) -> Vec<u16> {
    let mut ages = ages
        .into_iter()
        .map(|age| age.as_u64().unwrap_or(0).min(u16::MAX.into()) as u16)
        .collect::<Vec<_>>();
    ages.resize(CELL_COUNT, 0);
    ages.truncate(CELL_COUNT);
    ages
}

fn normalize_triggers(triggers: Option<Vec<CellTriggerType>>) -> Vec<CellTriggerType> {
    let mut triggers = triggers.unwrap_or_default();
    triggers.resize(CELL_COUNT, CellTriggerType::None);
    triggers.truncate(CELL_COUNT);
    triggers
}

fn normalize_symmetry(symmetry: Option<String>) -> String {
    let symmetry = symmetry.unwrap_or_else(|| "cross".into());
    if SYMMETRIES.contains(&symmetry.as_str()) {
        symmetry
    } else {
        "cross".into()
    }
}

fn visible(cells: &[u8]) -> Vec<bool> {
    cells.iter().map(|cell| *cell != EMPTY).collect()
}

fn triggers_from_visible(previous: &[bool], next: &[bool]) -> Vec<CellTriggerType> {
    (0..CELL_COUNT)
        .map(|index| match (previous[index], next[index]) {
            (false, true) => CellTriggerType::Activate,
            (true, false) => CellTriggerType::Deactivate,
            (true, true) => CellTriggerType::Stable,
            (false, false) => CellTriggerType::None,
        })
        .collect()
}

fn seed_at(state: &mut CrystalGrowthState, index: usize) {
    if state.cells[index] == EMPTY {
        state.cells[index] = 1;
    }
    state.ages[index] = 0;
}

fn seed_random(state: &mut CrystalGrowthState) {
    seed_random_avoiding(state, &[]);
}

fn seed_random_avoiding(state: &mut CrystalGrowthState, avoid: &[bool]) {
    let mut rng = rand::thread_rng();
    let mut empty = state
        .cells
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| {
            (*cell == EMPTY && !avoid.get(index).copied().unwrap_or(false)).then_some(index)
        })
        .collect::<Vec<_>>();
    if empty.is_empty() {
        empty = state
            .cells
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| (*cell == EMPTY).then_some(index))
            .collect();
    }
    if empty.is_empty() {
        seed_at(state, rng.gen_range(0..CELL_COUNT));
    } else {
        seed_at(state, empty[rng.gen_range(0..empty.len())]);
    }
}

fn force_reseed_if_empty(state: &mut CrystalGrowthState, avoid: &[bool]) {
    if state.cells.iter().all(|cell| *cell == EMPTY) {
        seed_random_avoiding(state, avoid);
    }
}

fn scheduled_seed(state: &mut CrystalGrowthState) {
    if state.seed_interval > 0
        && state.tick_counter % u64::from(state.seed_interval)
            == u64::from(state.seed_step % state.seed_interval)
    {
        seed_random(state);
    }
}

fn age_and_dissolve(state: &mut CrystalGrowthState) {
    if state.cell_life == 0 {
        return;
    }
    for index in 0..CELL_COUNT {
        if state.cells[index] == EMPTY {
            continue;
        }
        state.ages[index] = state.ages[index].saturating_add(1);
        if state.ages[index] >= state.cell_life {
            state.cells[index] = EMPTY;
            state.ages[index] = 0;
        }
    }
}

fn grow_candidates(state: &mut CrystalGrowthState, surviving: &[bool]) {
    let mut rng = rand::thread_rng();
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            if state.cells[index] != EMPTY || !has_neighbor(surviving, x, y, &state.symmetry) {
                continue;
            }
            if rng.gen_range(0..100) < state.growth_chance_pct {
                seed_at(state, index);
            }
        }
    }
}

fn has_neighbor(cells: &[bool], x: usize, y: usize, symmetry: &str) -> bool {
    for sy in 0..GRID_HEIGHT {
        for sx in 0..GRID_WIDTH {
            if cells[grid_index(sx, sy)] && has_neighbor_cell(sx, sy, x, y, symmetry) {
                return true;
            }
        }
    }
    false
}

fn has_neighbor_cell(x: usize, y: usize, target_x: usize, target_y: usize, symmetry: &str) -> bool {
    let offsets: &[(isize, isize)] = match symmetry {
        "diagonal" => &[(-1, -1), (1, -1), (-1, 1), (1, 1)],
        "snowflake" if (x + y).is_multiple_of(2) => {
            &[(-1, 0), (1, 0), (0, -1), (0, 1), (-1, 1), (1, -1)]
        }
        "snowflake" => &[(-1, 0), (1, 0), (0, -1), (0, 1), (1, 1), (-1, -1)],
        _ => &[(-1, 0), (1, 0), (0, -1), (0, 1)],
    };
    offsets.iter().any(|(dx, dy)| {
        let Some(nx) = x.checked_add_signed(*dx) else {
            return false;
        };
        let Some(ny) = y.checked_add_signed(*dy) else {
            return false;
        };
        nx == target_x && ny == target_y && nx < GRID_WIDTH && ny < GRID_HEIGHT
    })
}

#[cfg(test)]
mod tests;
