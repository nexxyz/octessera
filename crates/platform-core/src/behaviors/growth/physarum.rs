use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_AGENTS: usize = 32;
const UNIT: i16 = 16;
const WORLD_MAX_X: i16 = (GRID_WIDTH as i16 - 1) * UNIT + 15;
const WORLD_MAX_Y: i16 = (GRID_HEIGHT as i16 - 1) * UNIT + 15;
const VISIBLE: u8 = 12;
const ACTIVATE: u8 = 80;
const DX: [i16; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
const DY: [i16; 8] = [0, 1, 1, 1, 0, -1, -1, -1];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysarumState {
    pub x: Vec<i16>,
    pub y: Vec<i16>,
    pub heading: Vec<u8>,
    #[serde(rename = "activeCount")]
    pub active_count: usize,
    pub trail: Vec<u8>,
    pub food: Vec<u8>,
    #[serde(rename = "foodPattern")]
    pub food_pattern: u8,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "agentCount")]
    pub agent_count: usize,
    #[serde(rename = "senseDistance")]
    pub sense_distance: u8,
    #[serde(rename = "turnBiasPct")]
    pub turn_bias_pct: u8,
    #[serde(rename = "depositAmount")]
    pub deposit_amount: u8,
    #[serde(rename = "evaporationPct")]
    pub evaporation_pct: u8,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
}

#[derive(Default, Deserialize)]
struct Config {
    x: Option<Vec<Value>>,
    y: Option<Vec<Value>>,
    heading: Option<Vec<Value>>,
    #[serde(rename = "activeCount")]
    active_count: Option<Value>,
    trail: Option<Vec<Value>>,
    food: Option<Vec<Value>>,
    #[serde(rename = "foodPattern")]
    food_pattern: Option<Value>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "agentCount")]
    agent_count: Option<Value>,
    #[serde(rename = "senseDistance")]
    sense_distance: Option<Value>,
    #[serde(rename = "turnBiasPct")]
    turn_bias_pct: Option<Value>,
    #[serde(rename = "depositAmount")]
    deposit_amount: Option<Value>,
    #[serde(rename = "evaporationPct")]
    evaporation_pct: Option<Value>,
}

pub fn physarum_init(config: Value) -> Result<PhysarumState, String> {
    let mut s = from_config(config);
    seed_if_empty(&mut s);
    s.trigger_types = triggers(&render(&s), &s.trail, &render(&s), &s.trail, &[]);
    Ok(s)
}
pub fn physarum_deserialize(data: Value) -> Result<PhysarumState, String> {
    let mut s = from_config(data);
    s.trigger_types = triggers(&render(&s), &s.trail, &render(&s), &s.trail, &[]);
    Ok(s)
}
pub fn physarum_serialize(state: &PhysarumState) -> Result<Value, String> {
    let mut s = state.clone();
    normalize(&mut s);
    serde_json::to_value(s).map_err(|e| e.to_string())
}

pub fn physarum_on_input(
    mut state: PhysarumState,
    input: DeviceInput,
    _: &mut BehaviorContext,
) -> PhysarumState {
    normalize(&mut state);
    let pv = render(&state);
    let pt = state.trail.clone();
    let forced = match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let i = grid_index(x, y);
            state.food[i] = 1 - state.food[i];
            vec![]
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "relocateFood" =>
        {
            state.food_pattern = (state.food_pattern + 1) % 4;
            apply_food(&mut state);
            vec![]
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "seedSlime" =>
        {
            state.trail.fill(0);
            seed_agents(&mut state);
            agent_cells(&state)
        }
        _ => return state,
    };
    state.trigger_types = triggers(&pv, &pt, &render(&state), &state.trail, &forced);
    state
}

pub fn physarum_on_tick(mut state: PhysarumState, _: &mut BehaviorContext) -> PhysarumState {
    normalize(&mut state);
    let pv = render(&state);
    let pt = state.trail.clone();
    let previous_agents = agent_occupancy(&state);
    let trail_snapshot = state.trail.clone();
    let food_snapshot = state.food.clone();
    let ox = state.x.clone();
    let oy = state.y.clone();
    let oh = state.heading.clone();
    for i in 0..state.active_count {
        let h = oh[i];
        let ahead = sample(&state, &trail_snapshot, &food_snapshot, ox[i], oy[i], h);
        let left = sample(
            &state,
            &trail_snapshot,
            &food_snapshot,
            ox[i],
            oy[i],
            (h + 1) % 8,
        );
        let right = sample(
            &state,
            &trail_snapshot,
            &food_snapshot,
            ox[i],
            oy[i],
            (h + 7) % 8,
        );
        state.heading[i] = if should_turn(&state, i, left, ahead) && left >= right {
            (h + 1) % 8
        } else if should_turn(&state, i, right, ahead) && right > left {
            (h + 7) % 8
        } else {
            h
        };
        state.x[i] += DX[state.heading[i] as usize] * UNIT;
        state.y[i] += DY[state.heading[i] as usize] * UNIT;
        reflect(&mut state, i);
        let c = cell(state.x[i], state.y[i]);
        state.trail[c] = state.trail[c].saturating_add(state.deposit_amount);
    }
    for v in &mut state.trail {
        *v = (*v as u16 * (100 - u16::from(state.evaporation_pct)) / 100) as u8;
    }
    state.tick_counter = state.tick_counter.wrapping_add(1);
    drift_food(&mut state);
    let entered = agent_entries(&previous_agents, &agent_occupancy(&state));
    state.trigger_types = triggers(&pv, &pt, &render(&state), &state.trail, &entered);
    state
}

pub fn physarum_render_model(state: &PhysarumState) -> BehaviorRenderModel {
    let cells = render(state);
    let t = state.trail.iter().filter(|v| **v >= VISIBLE).count();
    BehaviorRenderModel {
        name: "physarum".into(),
        status_line: format!("A:{} T:{t}", state.active_count),
        cells,
        palette: crate::BehaviorRenderPalette {
            active: [240, 255, 140],
            inactive: crate::palette::BLACK,
            stable: [160, 180, 70],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}
pub fn physarum_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("agentCount", "Agent Count", 1, 32, 1),
        number_item("senseDistance", "Sense Distance", 1, 3, 1),
        number_item("turnBiasPct", "Turn Bias", 0, 100, 1),
        number_item("depositAmount", "Deposit Amount", 1, 64, 1),
        number_item("evaporationPct", "Evaporation", 0, 100, 1),
        action_item("relocateFood", "Relocate Food"),
        action_item("seedSlime", "Seed Slime"),
    ]
}

fn from_config(v: Value) -> PhysarumState {
    let c: Config = serde_json::from_value(v).unwrap_or_default();
    let ac = num(c.agent_count.or(c.active_count), 20, 32) as usize;
    let mut s = PhysarumState {
        x: norm_i(c.x, 0, 0, WORLD_MAX_X),
        y: norm_i(c.y, 0, 0, WORLD_MAX_Y),
        heading: norm_h(c.heading),
        active_count: ac,
        trail: norm_u8(c.trail),
        food: norm_food(c.food),
        food_pattern: num(c.food_pattern, 0, 3),
        trigger_types: norm_triggers(c.trigger_types),
        agent_count: ac,
        sense_distance: num(c.sense_distance, 1, 3).max(1),
        turn_bias_pct: num(c.turn_bias_pct, 55, 100),
        deposit_amount: num(c.deposit_amount, 18, 64).max(1),
        evaporation_pct: num(c.evaporation_pct, 18, 100),
        tick_counter: 0,
    };
    normalize(&mut s);
    s
}
fn normalize(s: &mut PhysarumState) {
    s.x = norm_i(
        Some(s.x.iter().map(|v| Value::from(*v)).collect()),
        0,
        0,
        WORLD_MAX_X,
    );
    s.y = norm_i(
        Some(s.y.iter().map(|v| Value::from(*v)).collect()),
        0,
        0,
        WORLD_MAX_Y,
    );
    s.heading = norm_h(Some(s.heading.iter().map(|v| Value::from(*v)).collect()));
    s.active_count = s.active_count.clamp(1, MAX_AGENTS);
    s.agent_count = s.active_count;
    s.trail = norm_u8(Some(s.trail.iter().map(|v| Value::from(*v)).collect()));
    s.food = norm_food(Some(s.food.iter().map(|v| Value::from(*v)).collect()));
    s.food_pattern = s.food_pattern.min(3);
    s.trigger_types = norm_triggers(Some(s.trigger_types.clone()));
    s.sense_distance = s.sense_distance.clamp(1, 3);
    s.turn_bias_pct = s.turn_bias_pct.min(100);
    s.deposit_amount = s.deposit_amount.clamp(1, 64);
    s.evaporation_pct = s.evaporation_pct.min(100);
    for i in s.active_count..MAX_AGENTS {
        s.x[i] = 0;
        s.y[i] = 0;
        s.heading[i] = 0
    }
}
fn norm_i(v: Option<Vec<Value>>, d: i16, min: i16, max: i16) -> Vec<i16> {
    let mut o = v
        .unwrap_or_default()
        .into_iter()
        .map(|v| {
            v.as_i64()
                .unwrap_or(i64::from(d))
                .clamp(i64::from(min), i64::from(max)) as i16
        })
        .collect::<Vec<_>>();
    o.resize(MAX_AGENTS, d);
    o.truncate(MAX_AGENTS);
    o
}
fn norm_h(v: Option<Vec<Value>>) -> Vec<u8> {
    let mut o = v
        .unwrap_or_default()
        .into_iter()
        .map(|v| (v.as_u64().unwrap_or(0) % 8) as u8)
        .collect::<Vec<_>>();
    o.resize(MAX_AGENTS, 0);
    o.truncate(MAX_AGENTS);
    o
}
fn norm_u8(v: Option<Vec<Value>>) -> Vec<u8> {
    let mut o = v
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.as_u64().unwrap_or(0).min(255) as u8)
        .collect::<Vec<_>>();
    o.resize(CELL_COUNT, 0);
    o.truncate(CELL_COUNT);
    o
}
fn norm_food(v: Option<Vec<Value>>) -> Vec<u8> {
    let mut o = v
        .unwrap_or_default()
        .into_iter()
        .map(|v| u8::from(v.as_u64().unwrap_or(0) > 0))
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
fn cell(x: i16, y: i16) -> usize {
    grid_index(
        (x / UNIT).clamp(0, GRID_WIDTH as i16 - 1) as usize,
        (y / UNIT).clamp(0, GRID_HEIGHT as i16 - 1) as usize,
    )
}
fn render(s: &PhysarumState) -> Vec<bool> {
    let mut r = (0..CELL_COUNT)
        .map(|i| s.trail[i] >= VISIBLE || s.food[i] > 0)
        .collect::<Vec<_>>();
    for i in 0..s.active_count {
        r[cell(s.x[i], s.y[i])] = true;
    }
    r
}
fn agent_cells(s: &PhysarumState) -> Vec<usize> {
    (0..s.active_count).map(|i| cell(s.x[i], s.y[i])).collect()
}
fn agent_occupancy(s: &PhysarumState) -> Vec<bool> {
    let mut occupied = vec![false; CELL_COUNT];
    for index in agent_cells(s) {
        occupied[index] = true;
    }
    occupied
}
fn agent_entries(previous: &[bool], next: &[bool]) -> Vec<usize> {
    (0..CELL_COUNT)
        .filter(|index| !previous[*index] && next[*index])
        .collect()
}
fn triggers(
    pv: &[bool],
    pt: &[u8],
    nv: &[bool],
    nt: &[u8],
    forced: &[usize],
) -> Vec<CellTriggerType> {
    let mut t = (0..CELL_COUNT)
        .map(|i| {
            if !pv[i] && nv[i] && nt[i] >= ACTIVATE {
                CellTriggerType::Activate
            } else if pv[i] && !nv[i] {
                CellTriggerType::Deactivate
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
    for i in 0..CELL_COUNT {
        if pt[i] < ACTIVATE && nt[i] >= ACTIVATE {
            t[i] = CellTriggerType::Activate;
        }
    }
    t
}
fn sample(s: &PhysarumState, trail: &[u8], food: &[u8], x: i16, y: i16, h: u8) -> u16 {
    let d = i16::from(s.sense_distance) * UNIT;
    let nx = (x + DX[h as usize] * d).clamp(0, WORLD_MAX_X);
    let ny = (y + DY[h as usize] * d).clamp(0, WORLD_MAX_Y);
    let c = cell(nx, ny);
    u16::from(trail[c]) + u16::from(food[c]) * 255
}
fn should_turn(s: &PhysarumState, index: usize, side: u16, ahead: u16) -> bool {
    if side <= ahead {
        return false;
    }
    if side - ahead >= 16 {
        return true;
    }
    hash(s.tick_counter, index) % 100 < u64::from(s.turn_bias_pct)
}
fn hash(tick: u64, index: usize) -> u64 {
    tick.wrapping_mul(1_103_515_245)
        .wrapping_add(index as u64 * 97)
}
fn reflect(s: &mut PhysarumState, i: usize) {
    if s.x[i] < 0 {
        s.x[i] = 0;
        s.heading[i] = (4 + 8 - s.heading[i]) % 8
    }
    if s.y[i] < 0 {
        s.y[i] = 0;
        s.heading[i] = (8 - s.heading[i]) % 8
    }
    if s.x[i] > WORLD_MAX_X {
        s.x[i] = WORLD_MAX_X;
        s.heading[i] = (4 + 8 - s.heading[i]) % 8
    }
    if s.y[i] > WORLD_MAX_Y {
        s.y[i] = WORLD_MAX_Y;
        s.heading[i] = (8 - s.heading[i]) % 8
    }
}
fn seed_if_empty(s: &mut PhysarumState) {
    if s.x.iter().take(s.active_count).all(|v| *v == 0)
        && s.y.iter().take(s.active_count).all(|v| *v == 0)
    {
        seed_agents(s);
        apply_food(s)
    }
}
fn seed_agents(s: &mut PhysarumState) {
    for i in 0..s.active_count {
        let col = (i % 5) as i16;
        let row = (i / 5) as i16;
        s.x[i] = 48 + col * 4;
        s.y[i] = 48 + row * 4;
        s.heading[i] = (i % 8) as u8
    }
}
fn apply_food(s: &mut PhysarumState) {
    s.food.fill(0);
    match s.food_pattern {
        0 => {
            for (x, y) in [(0, 0), (7, 0), (0, 7), (7, 7)] {
                s.food[grid_index(x, y)] = 1
            }
        }
        1 => {
            for (x, y) in [(3, 3), (4, 3), (3, 4), (4, 4)] {
                s.food[grid_index(x, y)] = 1
            }
        }
        2 => {
            for y in 0..GRID_HEIGHT {
                s.food[grid_index(0, y)] = 1;
                s.food[grid_index(7, y)] = 1
            }
        }
        _ => {
            for x in 0..GRID_WIDTH {
                s.food[grid_index(x, 0)] = 1;
                s.food[grid_index(x, 7)] = 1
            }
        }
    }
}
fn drift_food(s: &mut PhysarumState) {
    if s.food_pattern != 0 {
        return;
    }
    s.food.fill(0);
    let offset = (s.tick_counter as usize) % GRID_WIDTH;
    for (x, y) in [(offset, 0), (7 - offset, 7), (0, offset), (7, 7 - offset)] {
        s.food[grid_index(x, y)] = 1;
    }
}

#[cfg(test)]
mod tests;
