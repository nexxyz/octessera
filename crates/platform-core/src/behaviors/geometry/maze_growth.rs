use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const WALL: u8 = 0;
const PATH: u8 = 1;
const FRONTIER: u8 = 2;
const WALKER: u8 = 3;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MazeGrowthState {
    pub cells: Vec<u8>,
    pub visited: Vec<u8>,
    pub ages: Vec<u8>,
    pub walkers: Vec<usize>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "carvePct")]
    pub carve_pct: u8,
    #[serde(rename = "collapseAge")]
    pub collapse_age: u8,
    #[serde(rename = "walkerCount")]
    pub walker_count: u8,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
}

#[derive(Default, Deserialize)]
struct Config {
    cells: Option<Vec<Value>>,
    visited: Option<Vec<Value>>,
    ages: Option<Vec<Value>>,
    walkers: Option<Vec<Value>>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "carvePct")]
    carve_pct: Option<Value>,
    #[serde(rename = "collapseAge")]
    collapse_age: Option<Value>,
    #[serde(rename = "walkerCount")]
    walker_count: Option<Value>,
}

pub fn maze_growth_init(config: Value) -> Result<MazeGrowthState, String> {
    let mut s = from_config(config);
    if s.cells.iter().all(|cell| *cell == WALL) && s.walkers.is_empty() {
        seed_default(&mut s);
    }
    s.trigger_types = triggers(&s.cells, &s.cells, &[]);
    Ok(s)
}
pub fn maze_growth_deserialize(data: Value) -> Result<MazeGrowthState, String> {
    let mut s = from_config(data);
    s.trigger_types = triggers(&s.cells, &s.cells, &[]);
    Ok(s)
}
pub fn maze_growth_serialize(state: &MazeGrowthState) -> Result<Value, String> {
    let mut s = state.clone();
    normalize(&mut s);
    serde_json::to_value(s).map_err(|e| e.to_string())
}
pub fn maze_growth_on_input(
    mut state: MazeGrowthState,
    input: DeviceInput,
    _: &mut BehaviorContext,
) -> MazeGrowthState {
    normalize(&mut state);
    let prev = state.cells.clone();
    let mut forced = Vec::new();
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let i = grid_index(x, y);
            state.cells[i] = if state.cells[i] == WALL { PATH } else { WALL };
            state.visited[i] = u8::from(state.cells[i] != WALL);
            state.ages[i] = 0;
            forced.push(i)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "restartMaze" =>
        {
            restart(&mut state, &mut forced)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "collapseMaze" =>
        {
            collapse_action(&mut state)
        }
        _ => return state,
    }
    state.trigger_types = triggers(&prev, &state.cells, &forced);
    state
}
pub fn maze_growth_on_tick(mut state: MazeGrowthState, _: &mut BehaviorContext) -> MazeGrowthState {
    normalize(&mut state);
    let prev = state.cells.clone();
    let prev_visited = state.visited.clone();
    let prev_walkers = state.walkers.clone();
    let mut forced = Vec::new();
    let mut reserved = [false; CELL_COUNT];
    for (i, cell) in prev.iter().enumerate().take(CELL_COUNT) {
        if *cell == PATH || *cell == FRONTIER {
            state.ages[i] = state.ages[i].saturating_add(1)
        }
    }
    for (i, cell) in prev.iter().enumerate().take(CELL_COUNT) {
        if *cell != FRONTIER {
            continue;
        }
        if let Some(dest) = wall_neighbor(&prev, &prev_visited, i, state.tick_counter) {
            if !reserved[dest] && hash_pct(state.tick_counter, i, 7) < u32::from(state.carve_pct) {
                reserved[dest] = true;
                state.cells[i] = PATH;
                state.cells[dest] = FRONTIER;
                state.visited[dest] = 1;
                state.ages[i] = 0;
                state.ages[dest] = 0;
                forced.push(dest)
            }
        } else {
            state.cells[i] = PATH;
            state.ages[i] = 0
        }
    }
    let mut new_walkers = Vec::new();
    let mut walker_reserved = [false; CELL_COUNT];
    for (wi, origin) in prev_walkers.into_iter().enumerate() {
        if origin >= CELL_COUNT {
            continue;
        }
        let dest = visible_neighbor(&state.cells, origin, state.tick_counter + wi as u64, |d| {
            !walker_reserved[d]
        })
        .unwrap_or(origin);
        state.cells[origin] = if state.cells[origin] == WALKER {
            PATH
        } else {
            state.cells[origin]
        };
        state.cells[dest] = WALKER;
        state.visited[dest] = 1;
        state.ages[dest] = 0;
        walker_reserved[dest] = true;
        new_walkers.push(dest);
        forced.push(dest)
    }
    state.walkers = new_walkers;
    for i in 0..CELL_COUNT {
        if state.cells[i] != WALKER
            && (state.cells[i] == PATH || state.cells[i] == FRONTIER)
            && state.ages[i] > state.collapse_age
            && hash_pct(state.tick_counter, i, 31) < u32::from(state.carve_pct)
        {
            state.cells[i] = WALL;
            state.visited[i] = 0;
            state.ages[i] = 0
        }
    }
    state.tick_counter = state.tick_counter.wrapping_add(1);
    state.trigger_types = triggers(&prev, &state.cells, &forced);
    state
}
pub fn maze_growth_render_model(state: &MazeGrowthState) -> BehaviorRenderModel {
    let p = state
        .cells
        .iter()
        .filter(|c| **c == PATH || **c == WALKER)
        .count();
    let f = state.cells.iter().filter(|c| **c == FRONTIER).count();
    BehaviorRenderModel {
        name: "maze growth".into(),
        status_line: format!("P:{p} F:{f}"),
        cells: state.cells.iter().map(|c| *c != WALL).collect(),
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::YELLOW,
            inactive: crate::palette::BLACK,
            stable: crate::palette::GRAY,
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}
pub fn maze_growth_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("carvePct", "Carve", 0, 100, 1),
        number_item("collapseAge", "Collapse Age", 1, 64, 1),
        number_item("walkerCount", "Walker Count", 1, 8, 1),
        action_item("restartMaze", "Restart Maze"),
        action_item("collapseMaze", "Collapse Maze"),
    ]
}

fn from_config(v: Value) -> MazeGrowthState {
    let c: Config = serde_json::from_value(v).unwrap_or_default();
    let walker_count = num(c.walker_count, 2, 8).max(1);
    let mut s = MazeGrowthState {
        cells: norm(c.cells, 0, 3),
        visited: norm(c.visited, 0, 1),
        ages: norm(c.ages, 0, 255),
        walkers: norm_walkers(c.walkers, walker_count),
        trigger_types: norm_triggers(c.trigger_types),
        carve_pct: num(c.carve_pct, 100, 100),
        collapse_age: num(c.collapse_age, 32, 64).max(1),
        walker_count,
        tick_counter: 0,
    };
    normalize(&mut s);
    s
}
fn normalize(s: &mut MazeGrowthState) {
    s.cells = norm(
        Some(s.cells.iter().map(|v| Value::from(*v)).collect()),
        0,
        3,
    );
    s.visited = norm(
        Some(s.visited.iter().map(|v| Value::from(*v)).collect()),
        0,
        1,
    );
    s.ages = norm(
        Some(s.ages.iter().map(|v| Value::from(*v)).collect()),
        0,
        255,
    );
    s.carve_pct = s.carve_pct.min(100);
    s.collapse_age = s.collapse_age.clamp(1, 64);
    s.walker_count = s.walker_count.clamp(1, 8);
    s.walkers = clean_walkers(&s.walkers, &mut s.cells, usize::from(s.walker_count));
    s.trigger_types = norm_triggers(Some(s.trigger_types.clone()))
}
fn norm(v: Option<Vec<Value>>, d: u8, max: u8) -> Vec<u8> {
    let mut o = v
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.as_u64().unwrap_or(u64::from(d)).min(u64::from(max)) as u8)
        .collect::<Vec<_>>();
    o.resize(CELL_COUNT, d);
    o.truncate(CELL_COUNT);
    o
}
fn norm_triggers(v: Option<Vec<CellTriggerType>>) -> Vec<CellTriggerType> {
    let mut o = v.unwrap_or_default();
    o.resize(CELL_COUNT, CellTriggerType::None);
    o.truncate(CELL_COUNT);
    o
}
fn num(v: Option<Value>, d: u8, max: u8) -> u8 {
    v.and_then(|v| v.as_u64())
        .map(|v| v.min(max.into()) as u8)
        .unwrap_or(d)
}
fn norm_walkers(v: Option<Vec<Value>>, count: u8) -> Vec<usize> {
    v.unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_u64().map(|n| n as usize))
        .take(count.into())
        .collect()
}
fn clean_walkers(w: &[usize], cells: &mut [u8], count: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for i in w.iter().copied() {
        if i < CELL_COUNT && !out.contains(&i) && cells[i] != WALL {
            cells[i] = WALKER;
            out.push(i)
        }
    }
    for (i, cell) in cells.iter_mut().enumerate().take(CELL_COUNT) {
        if *cell == WALKER && !out.contains(&i) {
            if out.len() < count {
                out.push(i);
            } else {
                *cell = PATH;
            }
        }
    }
    out.truncate(count);
    for (i, cell) in cells.iter_mut().enumerate().take(CELL_COUNT) {
        if *cell == WALKER && !out.contains(&i) {
            *cell = PATH;
        }
    }
    out
}
fn restart(s: &mut MazeGrowthState, forced: &mut Vec<usize>) {
    s.cells.fill(WALL);
    s.visited.fill(0);
    s.ages.fill(0);
    s.walkers.clear();
    seed_default(s);
    forced.extend(s.walkers.iter().copied());
    forced.push(grid_index(3, 3));
}
fn seed_default(s: &mut MazeGrowthState) {
    for i in [grid_index(3, 4), grid_index(4, 3)] {
        s.cells[i] = if i == grid_index(3, 4) {
            PATH
        } else {
            FRONTIER
        };
        s.visited[i] = 1;
        s.ages[i] = 0;
    }
    for i in [grid_index(3, 3), grid_index(4, 4)]
        .into_iter()
        .take(s.walker_count.into())
    {
        s.cells[i] = WALKER;
        s.visited[i] = 1;
        s.ages[i] = 0;
        s.walkers.push(i);
    }
}
fn collapse_action(s: &mut MazeGrowthState) {
    let mut rows = (0..CELL_COUNT)
        .filter(|i| s.cells[*i] == PATH || s.cells[*i] == FRONTIER)
        .map(|i| (s.ages[i], i))
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    for (_, i) in rows.into_iter().take(8) {
        if !s.walkers.contains(&i) {
            s.cells[i] = WALL;
            s.visited[i] = 0;
            s.ages[i] = 0
        }
    }
}
fn neigh(i: usize, rot: u64) -> impl Iterator<Item = usize> {
    let x = i % GRID_WIDTH;
    let y = i / GRID_WIDTH;
    let dirs = [(0, 1), (1, 0), (0, -1), (-1, 0)];
    (0..4)
        .filter_map(move |n| {
            let (dx, dy) = dirs[(n + rot as usize) % 4];
            Some((x.checked_add_signed(dx)?, y.checked_add_signed(dy)?))
        })
        .filter(|(x, y)| *x < GRID_WIDTH && *y < GRID_HEIGHT)
        .map(|(x, y)| grid_index(x, y))
}
fn wall_neighbor(cells: &[u8], visited: &[u8], i: usize, tick: u64) -> Option<usize> {
    neigh(i, tick + i as u64).find(|n| cells[*n] == WALL && visited[*n] == 0)
}
fn visible_neighbor(
    cells: &[u8],
    i: usize,
    tick: u64,
    available: impl Fn(usize) -> bool,
) -> Option<usize> {
    neigh(i, tick + i as u64).find(|n| cells[*n] != WALL && available(*n))
}
fn hash_pct(tick: u64, index: usize, salt: u64) -> u32 {
    let mut x = tick
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(index as u64 * 0x85EB_CA6B)
        .wrapping_add(salt * 0xC2B2_AE35);
    x ^= x >> 16;
    ((x.wrapping_mul(0x27D4_EB2D) >> 24) % 100) as u32
}
fn triggers(p: &[u8], n: &[u8], forced: &[usize]) -> Vec<CellTriggerType> {
    let mut t = (0..CELL_COUNT)
        .map(|i| {
            if p[i] == WALL && n[i] != WALL {
                CellTriggerType::Activate
            } else if p[i] != WALL && n[i] == WALL {
                CellTriggerType::Deactivate
            } else if n[i] != WALL {
                CellTriggerType::Stable
            } else {
                CellTriggerType::None
            }
        })
        .collect::<Vec<_>>();
    for i in forced {
        t[*i] = if n[*i] == WALL {
            CellTriggerType::Deactivate
        } else {
            CellTriggerType::Activate
        }
    }
    t
}

#[cfg(test)]
mod tests;
