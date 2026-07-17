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
const LEADER: u8 = 1;
const FLASH: u8 = 2;
const EDGES: &[&str] = &["north", "east", "south", "west"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaderPos {
    pub x: usize,
    pub y: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LightningState {
    pub cells: Vec<u8>,
    pub ages: Vec<u8>,
    pub leaders: Vec<LeaderPos>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "branchChancePct")]
    pub branch_chance_pct: u8,
    #[serde(rename = "jitterChancePct")]
    pub jitter_chance_pct: u8,
    #[serde(rename = "decayTicks")]
    pub decay_ticks: u8,
    #[serde(rename = "leaderLimit")]
    pub leader_limit: u8,
    #[serde(rename = "targetEdge")]
    pub target_edge: String,
}

#[derive(Default, Deserialize)]
struct Config {
    cells: Option<Vec<Value>>,
    ages: Option<Vec<Value>>,
    leaders: Option<Vec<LeaderPos>>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "branchChancePct")]
    branch_chance_pct: Option<Value>,
    #[serde(rename = "jitterChancePct")]
    jitter_chance_pct: Option<Value>,
    #[serde(rename = "decayTicks")]
    decay_ticks: Option<Value>,
    #[serde(rename = "leaderLimit")]
    leader_limit: Option<Value>,
    #[serde(rename = "targetEdge")]
    target_edge: Option<String>,
}

pub fn lightning_init(config: Value) -> Result<LightningState, String> {
    let config: Config = serde_json::from_value(config).unwrap_or_default();
    let mut state = LightningState {
        cells: normalize_cells(config.cells.unwrap_or_default()),
        ages: normalize_ages(config.ages.unwrap_or_default()),
        leaders: config.leaders.unwrap_or_default(),
        trigger_types: normalize_triggers(config.trigger_types),
        branch_chance_pct: number(config.branch_chance_pct, 25, 100),
        jitter_chance_pct: number(config.jitter_chance_pct, 20, 100),
        decay_ticks: number(config.decay_ticks, 4, 16).max(1),
        leader_limit: number(config.leader_limit, 3, 8).max(1),
        target_edge: normalize_edge(config.target_edge),
    };
    normalize_state(&mut state);
    if !visible_any(&state.cells) {
        start_strike(&mut state);
    }
    let empty = [false; CELL_COUNT];
    state.trigger_types = triggers(&empty, &visible(&state.cells), false);
    Ok(state)
}

pub fn lightning_deserialize(data: Value) -> Result<LightningState, String> {
    let mut state = lightning_init(data)?;
    state.trigger_types = visible(&state.cells)
        .into_iter()
        .map(|v| {
            if v {
                CellTriggerType::Stable
            } else {
                CellTriggerType::None
            }
        })
        .collect();
    Ok(state)
}

pub fn lightning_serialize(state: &LightningState) -> Result<Value, String> {
    let mut state = state.clone();
    normalize_state(&mut state);
    serde_json::to_value(state).map_err(|e| e.to_string())
}

pub fn lightning_on_input(
    mut state: LightningState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> LightningState {
    normalize_state(&mut state);
    let previous = visible(&state.cells);
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "strikeNow" =>
        {
            start_strike(&mut state)
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            clear(&mut state);
            add_leader(&mut state, x, y);
        }
        _ => return state,
    }
    state.trigger_types = triggers(&previous, &visible(&state.cells), false);
    state
}

pub fn lightning_on_tick(
    mut state: LightningState,
    _context: &mut BehaviorContext,
) -> LightningState {
    normalize_state(&mut state);
    let previous = visible(&state.cells);
    let mut flash = false;
    if state.cells.contains(&FLASH) {
        for i in 0..CELL_COUNT {
            if state.cells[i] == FLASH {
                state.ages[i] = state.ages[i].saturating_add(1);
            }
        }
        if state
            .cells
            .iter()
            .enumerate()
            .any(|(i, c)| *c == FLASH && state.ages[i] >= state.decay_ticks)
        {
            clear(&mut state);
            start_strike(&mut state);
        }
    } else {
        let snapshot = state.leaders.clone();
        advance_leaders(&mut state, &snapshot);
        branch_leaders(&mut state, &snapshot);
        if state
            .leaders
            .iter()
            .any(|p| at_target(p.x, p.y, &state.target_edge))
        {
            enter_flash(&mut state);
            flash = true;
        }
    }
    state.trigger_types = triggers(&previous, &visible(&state.cells), flash);
    state
}

pub fn lightning_render_model(state: &LightningState) -> BehaviorRenderModel {
    let leaders = state.leaders.len();
    let flash = state.cells.iter().filter(|c| **c == FLASH).count();
    BehaviorRenderModel {
        name: "lightning".into(),
        status_line: format!("L:{leaders} F:{flash}"),
        cells: visible(&state.cells),
        palette: crate::BehaviorRenderPalette {
            active: [255, 255, 180],
            inactive: crate::palette::BLACK,
            stable: [80, 180, 255],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn lightning_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("branchChancePct", "Branch Chance", 0, 100, 1),
        number_item("jitterChancePct", "Jitter Chance", 0, 100, 1),
        number_item("decayTicks", "Decay Ticks", 1, 16, 1),
        number_item("leaderLimit", "Leader Limit", 1, 8, 1),
        enum_item("targetEdge", "Target Edge", EDGES),
        action_item("strikeNow", "Strike Now"),
    ]
}

fn normalize_state(s: &mut LightningState) {
    s.cells = normalize_cells(s.cells.iter().map(|c| Value::from(*c)).collect());
    s.ages = normalize_ages(s.ages.iter().map(|a| Value::from(*a)).collect());
    s.trigger_types = normalize_triggers(Some(s.trigger_types.clone()));
    s.branch_chance_pct = s.branch_chance_pct.min(100);
    s.jitter_chance_pct = s.jitter_chance_pct.min(100);
    s.decay_ticks = s.decay_ticks.clamp(1, 16);
    s.leader_limit = s.leader_limit.clamp(1, 8);
    s.target_edge = normalize_edge(Some(s.target_edge.clone()));
    s.leaders.retain(|p| p.x < GRID_WIDTH && p.y < GRID_HEIGHT);
    s.leaders.truncate(s.leader_limit as usize);
    if s.cells.contains(&FLASH) {
        s.leaders.clear();
        for cell in &mut s.cells {
            if *cell == LEADER {
                *cell = FLASH;
            }
        }
        return;
    }
    let limit = s.leader_limit as usize;
    for index in 0..CELL_COUNT {
        if s.cells[index] != LEADER {
            continue;
        }
        let x = index % GRID_WIDTH;
        let y = index / GRID_WIDTH;
        if !s
            .leaders
            .iter()
            .any(|leader| leader.x == x && leader.y == y)
        {
            if s.leaders.len() < limit {
                s.leaders.push(LeaderPos { x, y });
            } else {
                s.cells[index] = EMPTY;
                s.ages[index] = 0;
            }
        }
    }
    let leaders = s.leaders.clone();
    s.cells
        .iter_mut()
        .zip(s.ages.iter_mut())
        .enumerate()
        .for_each(|(index, (cell, age))| {
            if *cell == LEADER
                && !leaders
                    .iter()
                    .any(|leader| grid_index(leader.x, leader.y) == index)
            {
                *cell = EMPTY;
                *age = 0;
            }
        });
    for leader in leaders {
        s.cells[grid_index(leader.x, leader.y)] = LEADER;
    }
}
fn number(v: Option<Value>, d: u8, m: u8) -> u8 {
    v.and_then(|v| v.as_u64())
        .map(|v| v.min(m.into()) as u8)
        .unwrap_or(d)
}
fn normalize_cells(v: Vec<Value>) -> Vec<u8> {
    let mut out = v
        .into_iter()
        .map(|v| v.as_u64().unwrap_or(0).min(2) as u8)
        .collect::<Vec<_>>();
    out.resize(CELL_COUNT, EMPTY);
    out.truncate(CELL_COUNT);
    out
}
fn normalize_ages(v: Vec<Value>) -> Vec<u8> {
    let mut out = v
        .into_iter()
        .map(|v| v.as_u64().unwrap_or(0).min(u8::MAX.into()) as u8)
        .collect::<Vec<_>>();
    out.resize(CELL_COUNT, 0);
    out.truncate(CELL_COUNT);
    out
}
fn normalize_triggers(v: Option<Vec<CellTriggerType>>) -> Vec<CellTriggerType> {
    let mut out = v.unwrap_or_default();
    out.resize(CELL_COUNT, CellTriggerType::None);
    out.truncate(CELL_COUNT);
    out
}
fn normalize_edge(v: Option<String>) -> String {
    let edge = v.unwrap_or_else(|| "south".into());
    if EDGES.contains(&edge.as_str()) {
        edge
    } else {
        "south".into()
    }
}
fn visible(cells: &[u8]) -> Vec<bool> {
    cells.iter().map(|c| *c != EMPTY).collect()
}
fn visible_any(cells: &[u8]) -> bool {
    cells.iter().any(|c| *c != EMPTY)
}
fn triggers(prev: &[bool], next: &[bool], flash: bool) -> Vec<CellTriggerType> {
    (0..CELL_COUNT)
        .map(|i| {
            if flash && next[i] {
                CellTriggerType::Activate
            } else {
                match (prev[i], next[i]) {
                    (false, true) => CellTriggerType::Activate,
                    (true, false) => CellTriggerType::Deactivate,
                    (true, true) => CellTriggerType::Stable,
                    _ => CellTriggerType::None,
                }
            }
        })
        .collect()
}
fn clear(s: &mut LightningState) {
    s.cells.fill(EMPTY);
    s.ages.fill(0);
    s.leaders.clear();
}
fn add_leader(s: &mut LightningState, x: usize, y: usize) {
    if s.leaders.len() < s.leader_limit as usize {
        s.leaders.push(LeaderPos { x, y });
    }
    s.cells[grid_index(x, y)] = LEADER;
}
fn start_strike(s: &mut LightningState) {
    clear(s);
    let mut rng = rand::thread_rng();
    let (x, y) = match s.target_edge.as_str() {
        "north" => (rng.gen_range(0..GRID_WIDTH), 0),
        "east" => (0, rng.gen_range(0..GRID_HEIGHT)),
        "west" => (GRID_WIDTH - 1, rng.gen_range(0..GRID_HEIGHT)),
        _ => (rng.gen_range(0..GRID_WIDTH), GRID_HEIGHT - 1),
    };
    add_leader(s, x, y);
}
fn at_target(x: usize, y: usize, edge: &str) -> bool {
    match edge {
        "north" => y == GRID_HEIGHT - 1,
        "east" => x == GRID_WIDTH - 1,
        "west" => x == 0,
        _ => y == 0,
    }
}
fn step_toward(x: usize, y: usize, edge: &str, jitter: bool) -> (usize, usize) {
    let mut rng = rand::thread_rng();
    let lateral = if jitter {
        rng.gen_range(0..3) as isize - 1
    } else {
        0
    };
    let (dx, dy) = match edge {
        "north" => (lateral, 1),
        "east" => (1, lateral),
        "west" => (-1, lateral),
        _ => (lateral, -1),
    };
    (
        x.saturating_add_signed(dx).min(GRID_WIDTH - 1),
        y.saturating_add_signed(dy).min(GRID_HEIGHT - 1),
    )
}
fn advance_leaders(s: &mut LightningState, snapshot: &[LeaderPos]) {
    s.leaders.clear();
    for p in snapshot.iter().take(s.leader_limit as usize) {
        let jitter = rand::thread_rng().gen_range(0..100) < s.jitter_chance_pct;
        let (x, y) = step_toward(p.x, p.y, &s.target_edge, jitter);
        add_leader(s, x, y);
    }
}
fn branch_leaders(s: &mut LightningState, snapshot: &[LeaderPos]) {
    for p in snapshot {
        if s.leaders.len() >= s.leader_limit as usize {
            break;
        }
        if rand::thread_rng().gen_range(0..100) < s.branch_chance_pct {
            let (x, y) = step_toward(p.x, p.y, &s.target_edge, true);
            add_leader(s, x, y);
        }
    }
}
fn enter_flash(s: &mut LightningState) {
    for i in 0..CELL_COUNT {
        if s.cells[i] != EMPTY {
            s.cells[i] = FLASH;
            s.ages[i] = 0;
        }
    }
    s.leaders.clear();
}

#[cfg(test)]
mod tests;
