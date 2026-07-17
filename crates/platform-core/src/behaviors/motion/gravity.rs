use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, enum_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const EMPTY: u8 = 0;
const SAND: u8 = 1;
const DIRS: &[&str] = &["down", "left", "up", "right"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GravityState {
    pub cells: Vec<u8>,
    pub age: Vec<u8>,
    #[serde(rename = "gravityDir")]
    pub gravity_dir: String,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "spawnRatePct")]
    pub spawn_rate_pct: u8,
    #[serde(rename = "slideChancePct")]
    pub slide_chance_pct: u8,
    #[serde(rename = "settleAge")]
    pub settle_age: u8,
}

#[derive(Default, Deserialize)]
struct Config {
    cells: Option<Vec<Value>>,
    age: Option<Vec<Value>>,
    #[serde(rename = "gravityDir")]
    gravity_dir: Option<String>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "spawnRatePct")]
    spawn_rate_pct: Option<Value>,
    #[serde(rename = "slideChancePct")]
    slide_chance_pct: Option<Value>,
    #[serde(rename = "settleAge")]
    settle_age: Option<Value>,
}

pub fn gravity_init(config: Value) -> Result<GravityState, String> {
    let mut state = from_config(config);
    state.trigger_types = triggers(&state.cells, &state.cells);
    Ok(state)
}

pub fn gravity_deserialize(data: Value) -> Result<GravityState, String> {
    let mut state = from_config(data);
    state.trigger_types = triggers(&state.cells, &state.cells);
    Ok(state)
}

pub fn gravity_serialize(state: &GravityState) -> Result<Value, String> {
    let mut state = state.clone();
    normalize(&mut state);
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn gravity_on_input(
    mut state: GravityState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> GravityState {
    normalize(&mut state);
    let previous = state.cells.clone();
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let i = grid_index(x, y);
            state.cells[i] = if state.cells[i] == SAND { EMPTY } else { SAND };
            state.age[i] = 0;
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "dropSand" =>
        {
            spawn_edge(&mut state, true)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "clearBottom" =>
        {
            clear_bottom(&mut state)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "invertGravity" =>
        {
            state.gravity_dir = next_dir(&state.gravity_dir).into()
        }
        _ => return state,
    }
    state.trigger_types = triggers(&previous, &state.cells);
    state
}

pub fn gravity_on_tick(mut state: GravityState, _context: &mut BehaviorContext) -> GravityState {
    normalize(&mut state);
    let previous = state.cells.clone();
    let mut next = state.cells.clone();
    let mut age = state.age.clone();
    let mut vacated = vec![false; CELL_COUNT];
    for i in scan_order(&state.gravity_dir) {
        if state.cells[i] != SAND || next[i] != SAND {
            continue;
        }
        let x = i % GRID_WIDTH;
        let y = i / GRID_WIDTH;
        if let Some(dest) = destination(&state, &next, x, y, i) {
            next[i] = EMPTY;
            age[i] = 0;
            vacated[i] = true;
            next[dest] = SAND;
            age[dest] = 0;
        } else {
            age[i] = age[i].saturating_add(1).min(state.settle_age);
        }
    }
    state.cells = next;
    state.age = age;
    spawn_edge_skipping(&mut state, false, &vacated);
    state.tick_counter = state.tick_counter.wrapping_add(1);
    state.trigger_types = triggers(&previous, &state.cells);
    state
}

pub fn gravity_render_model(state: &GravityState) -> BehaviorRenderModel {
    let sand = state.cells.iter().filter(|cell| **cell == SAND).count();
    let moving = state
        .trigger_types
        .iter()
        .filter(|t| **t == CellTriggerType::Activate)
        .count();
    BehaviorRenderModel {
        name: "gravity".into(),
        status_line: format!("G:{} M:{moving} S:{sand}", state.gravity_dir),
        cells: state.cells.iter().map(|cell| *cell == SAND).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [255, 220, 120],
            inactive: crate::palette::BLACK,
            stable: [180, 140, 60],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn gravity_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("spawnRatePct", "Spawn Rate", 0, 100, 1),
        number_item("slideChancePct", "Slide Chance", 0, 100, 1),
        number_item("settleAge", "Settle Age", 1, 32, 1),
        enum_item("gravityDir", "Gravity Dir", DIRS),
        action_item("dropSand", "Drop Sand"),
        action_item("clearBottom", "Clear Bottom"),
        action_item("invertGravity", "Invert Gravity"),
    ]
}

fn from_config(config: Value) -> GravityState {
    let config: Config = serde_json::from_value(config).unwrap_or_default();
    let mut state = GravityState {
        cells: norm_cells(config.cells.unwrap_or_default()),
        age: norm_age(config.age.unwrap_or_default()),
        gravity_dir: norm_dir(config.gravity_dir),
        tick_counter: 0,
        trigger_types: norm_triggers(config.trigger_types),
        spawn_rate_pct: num(config.spawn_rate_pct, 20, 100),
        slide_chance_pct: num(config.slide_chance_pct, 60, 100),
        settle_age: num(config.settle_age, 8, 32).max(1),
    };
    normalize(&mut state);
    state
}

fn normalize(state: &mut GravityState) {
    state.cells = norm_cells(state.cells.iter().map(|cell| Value::from(*cell)).collect());
    state.age = norm_age(state.age.iter().map(|age| Value::from(*age)).collect());
    state.gravity_dir = norm_dir(Some(state.gravity_dir.clone()));
    state.trigger_types = norm_triggers(Some(state.trigger_types.clone()));
    state.spawn_rate_pct = state.spawn_rate_pct.min(100);
    state.slide_chance_pct = state.slide_chance_pct.min(100);
    state.settle_age = state.settle_age.clamp(1, 32);
    for i in 0..CELL_COUNT {
        if state.cells[i] == EMPTY {
            state.age[i] = 0;
        }
    }
}

fn norm_cells(values: Vec<Value>) -> Vec<u8> {
    let mut out = values
        .into_iter()
        .map(|v| u8::from(v.as_u64().unwrap_or(0) > 0))
        .collect::<Vec<_>>();
    out.resize(CELL_COUNT, EMPTY);
    out.truncate(CELL_COUNT);
    out
}
fn norm_age(values: Vec<Value>) -> Vec<u8> {
    let mut out = values
        .into_iter()
        .map(|v| v.as_u64().unwrap_or(0).min(u8::MAX.into()) as u8)
        .collect::<Vec<_>>();
    out.resize(CELL_COUNT, 0);
    out.truncate(CELL_COUNT);
    out
}
fn norm_triggers(values: Option<Vec<CellTriggerType>>) -> Vec<CellTriggerType> {
    let mut out = values.unwrap_or_default();
    out.resize(CELL_COUNT, CellTriggerType::None);
    out.truncate(CELL_COUNT);
    out
}
fn norm_dir(value: Option<String>) -> String {
    let value = value.unwrap_or_else(|| "down".into());
    if DIRS.contains(&value.as_str()) {
        value
    } else {
        "down".into()
    }
}
fn num(value: Option<Value>, default: u8, max: u8) -> u8 {
    value
        .and_then(|v| v.as_u64())
        .map(|v| v.min(max.into()) as u8)
        .unwrap_or(default)
}
fn next_dir(dir: &str) -> &'static str {
    match dir {
        "down" => "left",
        "left" => "up",
        "up" => "right",
        _ => "down",
    }
}
fn vector(dir: &str) -> (isize, isize) {
    match dir {
        "left" => (-1, 0),
        "up" => (0, 1),
        "right" => (1, 0),
        _ => (0, -1),
    }
}
fn side_vectors(dir: &str) -> [(isize, isize); 2] {
    match dir {
        "left" | "right" => [(0, 1), (0, -1)],
        _ => [(-1, 0), (1, 0)],
    }
}
fn offset(x: usize, y: usize, dx: isize, dy: isize) -> Option<usize> {
    let nx = x.checked_add_signed(dx)?;
    let ny = y.checked_add_signed(dy)?;
    (nx < GRID_WIDTH && ny < GRID_HEIGHT).then_some(grid_index(nx, ny))
}

fn destination(
    state: &GravityState,
    next: &[u8],
    x: usize,
    y: usize,
    index: usize,
) -> Option<usize> {
    let (gx, gy) = vector(&state.gravity_dir);
    if let Some(dest) = offset(x, y, gx, gy).filter(|dest| next[*dest] == EMPTY) {
        return Some(dest);
    }
    if hash(state.tick_counter, index, &state.gravity_dir) % 100
        >= u64::from(state.slide_chance_pct)
    {
        return None;
    }
    let sides = side_vectors(&state.gravity_dir);
    let first = ((state.tick_counter + index as u64) & 1) as usize;
    for side in [sides[first], sides[1 - first]] {
        if let Some(dest) =
            offset(x, y, gx + side.0, gy + side.1).filter(|dest| next[*dest] == EMPTY)
        {
            return Some(dest);
        }
    }
    None
}

fn scan_order(dir: &str) -> Vec<usize> {
    let mut out = Vec::with_capacity(CELL_COUNT);
    let ys: Vec<usize> = if dir == "down" {
        (0..GRID_HEIGHT).collect()
    } else {
        (0..GRID_HEIGHT).rev().collect()
    };
    let xs: Vec<usize> = if dir == "left" {
        (0..GRID_WIDTH).collect()
    } else {
        (0..GRID_WIDTH).rev().collect()
    };
    for y in ys {
        for x in xs.iter().copied() {
            out.push(grid_index(x, y));
        }
    }
    out
}

fn spawn_edge(state: &mut GravityState, force: bool) {
    spawn_edge_skipping(state, force, &[]);
}

fn spawn_edge_skipping(state: &mut GravityState, force: bool, skip: &[bool]) {
    for index in source_edge(&state.gravity_dir) {
        if !skip.get(index).copied().unwrap_or(false)
            && state.cells[index] == EMPTY
            && (force
                || hash(state.tick_counter, index, &state.gravity_dir) % 100
                    < u64::from(state.spawn_rate_pct))
        {
            state.cells[index] = SAND;
            state.age[index] = 0;
        }
    }
}

fn clear_bottom(state: &mut GravityState) {
    for index in dest_edge(&state.gravity_dir) {
        state.cells[index] = EMPTY;
        state.age[index] = 0;
    }
}
fn source_edge(dir: &str) -> Vec<usize> {
    match dir {
        "down" => (0..GRID_WIDTH)
            .map(|x| grid_index(x, GRID_HEIGHT - 1))
            .collect(),
        "up" => (0..GRID_WIDTH).map(|x| grid_index(x, 0)).collect(),
        "left" => (0..GRID_HEIGHT)
            .map(|y| grid_index(GRID_WIDTH - 1, y))
            .collect(),
        _ => (0..GRID_HEIGHT).map(|y| grid_index(0, y)).collect(),
    }
}
fn dest_edge(dir: &str) -> Vec<usize> {
    match dir {
        "down" => (0..GRID_WIDTH).map(|x| grid_index(x, 0)).collect(),
        "up" => (0..GRID_WIDTH)
            .map(|x| grid_index(x, GRID_HEIGHT - 1))
            .collect(),
        "left" => (0..GRID_HEIGHT).map(|y| grid_index(0, y)).collect(),
        _ => (0..GRID_HEIGHT)
            .map(|y| grid_index(GRID_WIDTH - 1, y))
            .collect(),
    }
}
fn hash(tick: u64, index: usize, dir: &str) -> u64 {
    tick.wrapping_mul(1_103_515_245)
        .wrapping_add(index as u64 * 97)
        .wrapping_add(dir.as_bytes()[0] as u64)
}
fn triggers(prev: &[u8], next: &[u8]) -> Vec<CellTriggerType> {
    (0..CELL_COUNT)
        .map(|i| match (prev[i], next[i]) {
            (EMPTY, SAND) => CellTriggerType::Activate,
            (SAND, EMPTY) => CellTriggerType::Deactivate,
            (SAND, SAND) => CellTriggerType::Stable,
            _ => CellTriggerType::None,
        })
        .collect()
}

#[cfg(test)]
mod tests;
