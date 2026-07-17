use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const CLEAR: u8 = 0;
const STRESS: u8 = 1;
const CRACK: u8 = 2;
const TIP: u8 = 3;
const STRESS_VISIBLE: u8 = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CracksState {
    pub cells: Vec<u8>,
    pub stress: Vec<u8>,
    #[serde(rename = "pendingShatter")]
    pub pending_shatter: bool,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "stressPct")]
    pub stress_pct: u8,
    #[serde(rename = "branchPct")]
    pub branch_pct: u8,
    #[serde(rename = "propagationPct")]
    pub propagation_pct: u8,
    #[serde(rename = "shatterThreshold")]
    pub shatter_threshold: u8,
}

#[derive(Default, Deserialize)]
struct Config {
    cells: Option<Vec<Value>>,
    stress: Option<Vec<Value>>,
    #[serde(rename = "pendingShatter")]
    pending_shatter: Option<Value>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "stressPct")]
    stress_pct: Option<Value>,
    #[serde(rename = "branchPct")]
    branch_pct: Option<Value>,
    #[serde(rename = "propagationPct")]
    propagation_pct: Option<Value>,
    #[serde(rename = "shatterThreshold")]
    shatter_threshold: Option<Value>,
}

pub fn cracks_init(config: Value) -> Result<CracksState, String> {
    let mut s = from_config(config);
    s.trigger_types = triggers(&s.cells, &s.stress, &s.cells, &s.stress, &[]);
    Ok(s)
}
pub fn cracks_deserialize(data: Value) -> Result<CracksState, String> {
    let mut s = from_config(data);
    s.trigger_types = triggers(&s.cells, &s.stress, &s.cells, &s.stress, &[]);
    Ok(s)
}
pub fn cracks_serialize(state: &CracksState) -> Result<Value, String> {
    let mut s = state.clone();
    normalize(&mut s);
    serde_json::to_value(s).map_err(|e| e.to_string())
}

pub fn cracks_on_input(
    mut state: CracksState,
    input: DeviceInput,
    _: &mut BehaviorContext,
) -> CracksState {
    normalize(&mut state);
    let pc = state.cells.clone();
    let ps = state.stress.clone();
    let forced = match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            impact(&mut state, x, y)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "impact" =>
        {
            impact(&mut state, GRID_WIDTH / 2, GRID_HEIGHT / 2)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "replacePane" =>
        {
            replace(&mut state);
            vec![]
        }
        _ => return state,
    };
    state.trigger_types = triggers(&pc, &ps, &state.cells, &state.stress, &forced);
    state
}

pub fn cracks_on_tick(mut state: CracksState, _: &mut BehaviorContext) -> CracksState {
    normalize(&mut state);
    let pc = state.cells.clone();
    let ps = state.stress.clone();
    let mut forced = Vec::new();
    if state.pending_shatter {
        replace(&mut state);
        state.pending_shatter = false;
        state.trigger_types = triggers(&pc, &ps, &state.cells, &state.stress, &[]);
        return state;
    }
    let tips = (0..CELL_COUNT)
        .filter(|i| state.cells[*i] == TIP)
        .collect::<Vec<_>>();
    for tip in tips {
        if hash(state.tick_counter, tip) % 100 >= u64::from(state.propagation_pct) {
            continue;
        }
        let Some(dest) = best_neighbor(&state, tip, &[]) else {
            continue;
        };
        state.cells[tip] = CRACK;
        state.cells[dest] = TIP;
        state.stress[dest] = 0;
        forced.push(dest);
        if hash(state.tick_counter + 1, tip) % 100 < u64::from(state.branch_pct) {
            if let Some(b) = best_neighbor(&state, tip, &[dest]) {
                state.cells[b] = TIP;
                state.stress[b] = 0;
                forced.push(b);
            }
        }
    }
    for i in 0..CELL_COUNT {
        if state.cells[i] <= STRESS
            && hash(state.tick_counter, i) % 100 < u64::from(state.stress_pct)
        {
            state.stress[i] = state.stress[i].saturating_add(8);
            if state.cells[i] == CLEAR && state.stress[i] >= STRESS_VISIBLE {
                state.cells[i] = STRESS;
            }
        }
    }
    if crack_count(&state) >= usize::from(state.shatter_threshold) || connects_edges(&state) {
        state.pending_shatter = true;
    }
    state.tick_counter = state.tick_counter.wrapping_add(1);
    state.trigger_types = triggers(&pc, &ps, &state.cells, &state.stress, &forced);
    state
}

pub fn cracks_render_model(state: &CracksState) -> BehaviorRenderModel {
    let cracks = state.cells.iter().filter(|c| **c == CRACK).count();
    let tips = state.cells.iter().filter(|c| **c == TIP).count();
    BehaviorRenderModel {
        name: "cracks".into(),
        status_line: format!("C:{cracks} T:{tips}"),
        cells: visible_cells(&state.cells, &state.stress),
        palette: crate::BehaviorRenderPalette {
            active: [255, 245, 210],
            inactive: crate::palette::BLACK,
            stable: [120, 160, 180],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}
pub fn cracks_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("stressPct", "Stress", 0, 100, 1),
        number_item("branchPct", "Branch", 0, 100, 1),
        number_item("propagationPct", "Propagation", 0, 100, 1),
        number_item("shatterThreshold", "Shatter Threshold", 1, 64, 1),
        action_item("impact", "Impact"),
        action_item("replacePane", "Replace Pane"),
    ]
}

fn from_config(v: Value) -> CracksState {
    let c: Config = serde_json::from_value(v).unwrap_or_default();
    let mut s = CracksState {
        cells: norm_cells(c.cells.unwrap_or_default()),
        stress: norm_u8(c.stress.unwrap_or_default()),
        pending_shatter: c
            .pending_shatter
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        tick_counter: 0,
        trigger_types: norm_triggers(c.trigger_types),
        stress_pct: num(c.stress_pct, 20, 100),
        branch_pct: num(c.branch_pct, 18, 100),
        propagation_pct: num(c.propagation_pct, 65, 100),
        shatter_threshold: num(c.shatter_threshold, 24, 64).max(1),
    };
    normalize(&mut s);
    s
}
fn normalize(s: &mut CracksState) {
    s.cells = norm_cells(s.cells.iter().map(|v| Value::from(*v)).collect());
    s.stress = norm_u8(s.stress.iter().map(|v| Value::from(*v)).collect());
    s.trigger_types = norm_triggers(Some(s.trigger_types.clone()));
    s.stress_pct = s.stress_pct.min(100);
    s.branch_pct = s.branch_pct.min(100);
    s.propagation_pct = s.propagation_pct.min(100);
    s.shatter_threshold = s.shatter_threshold.clamp(1, 64)
}
fn norm_cells(v: Vec<Value>) -> Vec<u8> {
    let mut o = v
        .into_iter()
        .map(|v| v.as_u64().unwrap_or(0).min(3) as u8)
        .collect::<Vec<_>>();
    o.resize(CELL_COUNT, CLEAR);
    o.truncate(CELL_COUNT);
    o
}
fn norm_u8(v: Vec<Value>) -> Vec<u8> {
    let mut o = v
        .into_iter()
        .map(|v| v.as_u64().unwrap_or(0).min(255) as u8)
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
fn num(v: Option<Value>, d: u8, max: u8) -> u8 {
    v.and_then(|v| v.as_u64())
        .map(|v| v.min(max.into()) as u8)
        .unwrap_or(d)
}
fn impact(s: &mut CracksState, x: usize, y: usize) -> Vec<usize> {
    let idx = grid_index(x, y);
    s.cells[idx] = TIP;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
                if nx < GRID_WIDTH && ny < GRID_HEIGHT {
                    s.stress[grid_index(nx, ny)] = s.stress[grid_index(nx, ny)].saturating_add(96);
                }
            }
        }
    }
    vec![idx]
}
fn replace(s: &mut CracksState) {
    s.cells.fill(CLEAR);
    s.stress.fill(0)
}
fn visible_cells(c: &[u8], st: &[u8]) -> Vec<bool> {
    (0..CELL_COUNT)
        .map(|i| c[i] == CRACK || c[i] == TIP || st[i] >= STRESS_VISIBLE)
        .collect()
}
fn triggers(pc: &[u8], ps: &[u8], nc: &[u8], ns: &[u8], forced: &[usize]) -> Vec<CellTriggerType> {
    let pv = visible_cells(pc, ps);
    let nv = visible_cells(nc, ns);
    let mut t = (0..CELL_COUNT)
        .map(|i| {
            if pv[i] && !nv[i] {
                CellTriggerType::Deactivate
            } else if !pv[i] && nv[i] && (nc[i] == CRACK || nc[i] == TIP) {
                CellTriggerType::Activate
            } else if nv[i] {
                CellTriggerType::Stable
            } else {
                CellTriggerType::None
            }
        })
        .collect::<Vec<_>>();
    for i in forced {
        t[*i] = CellTriggerType::Activate;
    }
    t
}
fn neighbors(i: usize) -> Vec<usize> {
    let x = i % GRID_WIDTH;
    let y = i / GRID_WIDTH;
    [
        (0, 1),
        (1, 0),
        (0, -1),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, -1),
        (-1, 1),
    ]
    .into_iter()
    .filter_map(|(dx, dy)| {
        let nx = x.checked_add_signed(dx)?;
        let ny = y.checked_add_signed(dy)?;
        (nx < GRID_WIDTH && ny < GRID_HEIGHT).then_some(grid_index(nx, ny))
    })
    .collect()
}
fn best_neighbor(s: &CracksState, i: usize, skip: &[usize]) -> Option<usize> {
    let mut best = None;
    let mut best_stress = 0;
    for neighbor in neighbors(i)
        .into_iter()
        .filter(|n| !skip.contains(n) && s.cells[*n] <= STRESS)
    {
        if best.is_none() || s.stress[neighbor] > best_stress {
            best = Some(neighbor);
            best_stress = s.stress[neighbor];
        }
    }
    best
}
fn hash(t: u64, i: usize) -> u64 {
    t.wrapping_mul(1_103_515_245).wrapping_add(i as u64 * 97)
}
fn crack_count(s: &CracksState) -> usize {
    s.cells
        .iter()
        .filter(|c| **c == CRACK || **c == TIP)
        .count()
}
fn connects_edges(s: &CracksState) -> bool {
    let mut visited = [false; CELL_COUNT];
    for start in 0..CELL_COUNT {
        if visited[start] || s.cells[start] < CRACK {
            continue;
        }
        let mut stack = vec![start];
        visited[start] = true;
        let mut top = false;
        let mut bottom = false;
        let mut left = false;
        let mut right = false;
        while let Some(index) = stack.pop() {
            let x = index % GRID_WIDTH;
            let y = index / GRID_WIDTH;
            top |= y == GRID_HEIGHT - 1;
            bottom |= y == 0;
            left |= x == 0;
            right |= x == GRID_WIDTH - 1;
            for neighbor in neighbors(index) {
                if !visited[neighbor] && s.cells[neighbor] >= CRACK {
                    visited[neighbor] = true;
                    stack.push(neighbor);
                }
            }
        }
        if (top && bottom) || (left && right) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests;
