use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const EMPTY: u8 = 0;
const STEM: u8 = 1;
const TIP: u8 = 2;
const LEAF: u8 = 3;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VinesState {
    pub cells: Vec<u8>,
    pub energy: Vec<u8>,
    pub ages: Vec<u8>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "growthPct")]
    pub growth_pct: u8,
    #[serde(rename = "branchPct")]
    pub branch_pct: u8,
    #[serde(rename = "pruneAge")]
    pub prune_age: u8,
    #[serde(rename = "lightBiasPct")]
    pub light_bias_pct: u8,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
}

#[derive(Default, Deserialize)]
struct Config {
    cells: Option<Vec<Value>>,
    energy: Option<Vec<Value>>,
    ages: Option<Vec<Value>>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "growthPct")]
    growth_pct: Option<Value>,
    #[serde(rename = "branchPct")]
    branch_pct: Option<Value>,
    #[serde(rename = "pruneAge")]
    prune_age: Option<Value>,
    #[serde(rename = "lightBiasPct")]
    light_bias_pct: Option<Value>,
}

pub fn vines_init(config: Value) -> Result<VinesState, String> {
    let mut s = from_config(config);
    if s.cells.iter().all(|cell| *cell == EMPTY) {
        for (x, y) in [(3, 0), (4, 0), (3, 1)] {
            let index = grid_index(x, y);
            plant(&mut s, index);
        }
    }
    s.trigger_types = triggers(&s.cells, &s.cells);
    Ok(s)
}
pub fn vines_deserialize(data: Value) -> Result<VinesState, String> {
    let mut s = from_config(data);
    s.trigger_types = triggers(&s.cells, &s.cells);
    Ok(s)
}
pub fn vines_serialize(state: &VinesState) -> Result<Value, String> {
    let mut s = state.clone();
    normalize(&mut s);
    serde_json::to_value(s).map_err(|e| e.to_string())
}
pub fn vines_on_input(
    mut state: VinesState,
    input: DeviceInput,
    _: &mut BehaviorContext,
) -> VinesState {
    normalize(&mut state);
    let prev = state.cells.clone();
    let mut forced = Vec::new();
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let index = grid_index(x, y);
            plant(&mut state, index);
            forced.push(index);
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "plantSeed" =>
        {
            for (x, y) in [(3, 0), (4, 0), (3, 1)] {
                let index = grid_index(x, y);
                plant(&mut state, index);
                forced.push(index);
            }
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "pruneVines" =>
        {
            prune_action(&mut state)
        }
        _ => return state,
    }
    state.trigger_types = triggers_with_forced(&prev, &state.cells, &forced);
    state
}
pub fn vines_on_tick(mut state: VinesState, _: &mut BehaviorContext) -> VinesState {
    normalize(&mut state);
    let prev = state.cells.clone();
    let prev_energy = state.energy.clone();
    let prev_ages = state.ages.clone();
    let mut reserved = [false; CELL_COUNT];
    let mut forced = Vec::new();
    for i in 0..CELL_COUNT {
        if prev[i] != EMPTY {
            state.ages[i] = prev_ages[i].saturating_add(1)
        }
    }
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let i = grid_index(x, y);
            if prev[i] != TIP {
                continue;
            }
            let candidates = candidates(
                &prev,
                x,
                y,
                u32::from(prev_energy[i]),
                u32::from(state.light_bias_pct),
            );
            if candidates.is_empty() {
                continue;
            }
            if hash_pct(state.tick_counter, i, 11) < u32::from(state.growth_pct) {
                let dest = first_unreserved(&candidates, &reserved);
                if let Some(dest) = dest {
                    reserved[dest] = true;
                    state.cells[i] = STEM;
                    state.ages[i] = 0;
                    plant_with_energy(&mut state, dest, 200);
                    if hash_pct(state.tick_counter, i, 29) < u32::from(state.branch_pct) {
                        if let Some(branch) = first_unreserved(&candidates, &reserved) {
                            reserved[branch] = true;
                            plant_with_energy(&mut state, branch, 200);
                        }
                    }
                }
            }
        }
    }
    for i in 0..CELL_COUNT {
        if state.cells[i] == STEM && state.ages[i] >= state.prune_age {
            state.cells[i] = LEAF;
            state.energy[i] /= 2;
            state.ages[i] = 0;
            forced.push(i);
        } else if state.cells[i] == LEAF
            && state.ages[i] >= state.prune_age / 2
            && hash_pct(state.tick_counter, i, 47) < u32::from(state.growth_pct)
        {
            state.cells[i] = EMPTY;
            state.energy[i] = 0;
            state.ages[i] = 0
        }
    }
    state.tick_counter = state.tick_counter.wrapping_add(1);
    state.trigger_types = triggers_with_forced(&prev, &state.cells, &forced);
    state
}
pub fn vines_render_model(state: &VinesState) -> BehaviorRenderModel {
    let visible = state.cells.iter().filter(|c| **c != EMPTY).count();
    let tips = state.cells.iter().filter(|c| **c == TIP).count();
    BehaviorRenderModel {
        name: "vines".into(),
        status_line: format!("V:{visible} T:{tips}"),
        cells: state.cells.iter().map(|c| *c != EMPTY).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [170, 255, 100],
            inactive: crate::palette::BLACK,
            stable: [40, 160, 60],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}
pub fn vines_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("growthPct", "Growth", 0, 100, 1),
        number_item("branchPct", "Branch", 0, 100, 1),
        number_item("pruneAge", "Prune Age", 1, 64, 1),
        number_item("lightBiasPct", "Light Bias", 0, 100, 1),
        action_item("plantSeed", "Plant Seed"),
        action_item("pruneVines", "Prune Vines"),
    ]
}

fn from_config(v: Value) -> VinesState {
    let c: Config = serde_json::from_value(v).unwrap_or_default();
    let mut s = VinesState {
        cells: norm(c.cells, 0, 3),
        energy: norm(c.energy, 0, 255),
        ages: norm(c.ages, 0, 255),
        trigger_types: norm_triggers(c.trigger_types),
        growth_pct: num(c.growth_pct, 55, 100),
        branch_pct: num(c.branch_pct, 18, 100),
        prune_age: num(c.prune_age, 24, 64).max(1),
        light_bias_pct: num(c.light_bias_pct, 50, 100),
        tick_counter: 0,
    };
    normalize(&mut s);
    s
}
fn normalize(s: &mut VinesState) {
    s.cells = norm(
        Some(s.cells.iter().map(|v| Value::from(*v)).collect()),
        0,
        3,
    );
    s.energy = norm(
        Some(s.energy.iter().map(|v| Value::from(*v)).collect()),
        0,
        255,
    );
    s.ages = norm(
        Some(s.ages.iter().map(|v| Value::from(*v)).collect()),
        0,
        255,
    );
    s.trigger_types = norm_triggers(Some(s.trigger_types.clone()));
    s.growth_pct = s.growth_pct.min(100);
    s.branch_pct = s.branch_pct.min(100);
    s.prune_age = s.prune_age.clamp(1, 64);
    s.light_bias_pct = s.light_bias_pct.min(100)
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
fn plant(s: &mut VinesState, i: usize) {
    plant_with_energy(s, i, 220)
}
fn plant_with_energy(s: &mut VinesState, i: usize, e: u8) {
    s.cells[i] = TIP;
    s.energy[i] = e;
    s.ages[i] = 0
}
fn prune_action(s: &mut VinesState) {
    let mut rows = (0..CELL_COUNT)
        .filter(|i| s.cells[*i] != EMPTY && s.cells[*i] != TIP)
        .map(|i| (s.ages[i], i))
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    for (_, i) in rows.into_iter().take(8) {
        s.cells[i] = EMPTY;
        s.energy[i] = 0;
        s.ages[i] = 0
    }
}
fn candidates(cells: &[u8], x: usize, y: usize, source_energy: u32, light_bias: u32) -> Vec<usize> {
    let mut out = Vec::new();
    for (dx, dy) in [
        (0, 1),
        (1, 1),
        (-1, 1),
        (1, 0),
        (-1, 0),
        (1, -1),
        (-1, -1),
        (0, -1),
    ] {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
            if nx < GRID_WIDTH && ny < GRID_HEIGHT {
                let i = grid_index(nx, ny);
                if cells[i] == EMPTY {
                    out.push(i)
                }
            }
        }
    }
    out.sort_by(|a, b| {
        score(cells, *b, source_energy, light_bias).cmp(&score(
            cells,
            *a,
            source_energy,
            light_bias,
        ))
    });
    out
}
fn score(cells: &[u8], i: usize, source_energy: u32, light_bias: u32) -> u32 {
    let x = i % GRID_WIDTH;
    let y = i / GRID_WIDTH;
    empty_neighbors(cells, x, y) * 10 + y as u32 * light_bias / 10 + source_energy / 8
}
fn empty_neighbors(cells: &[u8], x: usize, y: usize) -> u32 {
    let mut c = 0;
    for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
            if nx < GRID_WIDTH && ny < GRID_HEIGHT && cells[grid_index(nx, ny)] == EMPTY {
                c += 1
            }
        }
    }
    c
}
fn first_unreserved(candidates: &[usize], reserved: &[bool; CELL_COUNT]) -> Option<usize> {
    candidates.iter().copied().find(|i| !reserved[*i])
}
fn hash_pct(tick: u64, index: usize, salt: u64) -> u32 {
    let mut x = tick
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(index as u64 * 0x85EB_CA6B)
        .wrapping_add(salt * 0xC2B2_AE35);
    x ^= x >> 16;
    ((x.wrapping_mul(0x27D4_EB2D) >> 24) % 100) as u32
}
fn triggers(p: &[u8], n: &[u8]) -> Vec<CellTriggerType> {
    (0..CELL_COUNT)
        .map(|i| {
            if p[i] == EMPTY && n[i] != EMPTY {
                CellTriggerType::Activate
            } else if p[i] != EMPTY && n[i] == EMPTY {
                CellTriggerType::Deactivate
            } else if n[i] != EMPTY {
                CellTriggerType::Stable
            } else {
                CellTriggerType::None
            }
        })
        .collect()
}
fn triggers_with_forced(p: &[u8], n: &[u8], forced: &[usize]) -> Vec<CellTriggerType> {
    let mut trigger_types = triggers(p, n);
    for index in forced {
        trigger_types[*index] = CellTriggerType::Activate;
    }
    trigger_types
}

#[cfg(test)]
mod tests;
