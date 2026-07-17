use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, enum_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_PARTICLES: usize = 16;
const UNIT: i16 = 16;
const WORLD_MAX_X: i16 = (GRID_WIDTH as i16 - 1) * UNIT + 15;
const WORLD_MAX_Y: i16 = (GRID_HEIGHT as i16 - 1) * UNIT + 15;
const SPEED: i16 = 14;
const REPEL: &[&str] = &["off", "always"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrbitState {
    pub x: Vec<i16>,
    pub y: Vec<i16>,
    pub vx: Vec<i16>,
    pub vy: Vec<i16>,
    #[serde(rename = "attractorX")]
    pub attractor_x: i16,
    #[serde(rename = "attractorY")]
    pub attractor_y: i16,
    #[serde(rename = "attractorVx")]
    pub attractor_vx: i16,
    #[serde(rename = "attractorVy")]
    pub attractor_vy: i16,
    #[serde(rename = "activeCount")]
    pub active_count: usize,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "attractionPct")]
    pub attraction_pct: u8,
    #[serde(rename = "orbitPct")]
    pub orbit_pct: u8,
    #[serde(rename = "repelMode")]
    pub repel_mode: String,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
}

#[derive(Default, Deserialize)]
struct Config {
    x: Option<Vec<Value>>,
    y: Option<Vec<Value>>,
    vx: Option<Vec<Value>>,
    vy: Option<Vec<Value>>,
    #[serde(rename = "attractorX")]
    attractor_x: Option<Value>,
    #[serde(rename = "attractorY")]
    attractor_y: Option<Value>,
    #[serde(rename = "attractorVx")]
    attractor_vx: Option<Value>,
    #[serde(rename = "attractorVy")]
    attractor_vy: Option<Value>,
    #[serde(rename = "activeCount")]
    active_count: Option<Value>,
    #[serde(rename = "particleCount")]
    particle_count: Option<Value>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "attractionPct")]
    attraction_pct: Option<Value>,
    #[serde(rename = "orbitPct")]
    orbit_pct: Option<Value>,
    #[serde(rename = "repelMode")]
    repel_mode: Option<String>,
}

pub fn orbit_init(config: Value) -> Result<OrbitState, String> {
    let mut s = from_config(config);
    seed_if_empty(&mut s);
    s.trigger_types = triggers(&occupancy(&s), &occupancy(&s), None);
    Ok(s)
}
pub fn orbit_deserialize(data: Value) -> Result<OrbitState, String> {
    let mut s = from_config(data);
    s.trigger_types = triggers(&occupancy(&s), &occupancy(&s), None);
    Ok(s)
}
pub fn orbit_serialize(state: &OrbitState) -> Result<Value, String> {
    let mut s = state.clone();
    normalize(&mut s);
    serde_json::to_value(s).map_err(|e| e.to_string())
}

pub fn orbit_on_input(
    mut state: OrbitState,
    input: DeviceInput,
    _: &mut BehaviorContext,
) -> OrbitState {
    normalize(&mut state);
    let prev = occupancy(&state);
    let mut force = None;
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            state.attractor_x = x as i16 * UNIT;
            state.attractor_y = y as i16 * UNIT;
            force = Some(grid_index(x, y));
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "resetOrbit" =>
        {
            reset_orbit(&mut state);
            force = Some(attractor_cell(&state));
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "nudgeAttractor" =>
        {
            nudge(&mut state)
        }
        _ => return state,
    }
    state.trigger_types = triggers(&prev, &occupancy(&state), force);
    state
}

pub fn orbit_on_tick(mut state: OrbitState, _: &mut BehaviorContext) -> OrbitState {
    normalize(&mut state);
    let prev = occupancy(&state);
    let ox = state.x.clone();
    let oy = state.y.clone();
    for i in 0..state.active_count {
        let dx = state.attractor_x - ox[i];
        let dy = state.attractor_y - oy[i];
        let pullx = dx.signum() * i16::from(state.attraction_pct) / 20;
        let pully = dy.signum() * i16::from(state.attraction_pct) / 20;
        let tangx = -dy.signum() * i16::from(state.orbit_pct) / 20;
        let tangy = dx.signum() * i16::from(state.orbit_pct) / 20;
        let repel = if state.repel_mode == "always" { -1 } else { 1 };
        state.vx[i] = (state.vx[i] + pullx * repel + tangx).clamp(-SPEED, SPEED);
        state.vy[i] = (state.vy[i] + pully * repel + tangy).clamp(-SPEED, SPEED);
        if state.vx[i] == 0 && state.vy[i] == 0 {
            state.vx[i] = 1;
            state.vy[i] = 1;
        }
        state.x[i] += state.vx[i];
        state.y[i] += state.vy[i];
        reflect_particle(&mut state, i);
    }
    state.attractor_x += state.attractor_vx;
    state.attractor_y += state.attractor_vy;
    reflect_attractor(&mut state);
    state.tick_counter = state.tick_counter.wrapping_add(1);
    state.trigger_types = triggers(&prev, &occupancy(&state), None);
    state
}

pub fn orbit_render_model(state: &OrbitState) -> BehaviorRenderModel {
    let mut cells = occupancy(state);
    cells[attractor_cell(state)] = true;
    let occupied = cells.iter().filter(|v| **v).count();
    BehaviorRenderModel {
        name: "orbit".into(),
        status_line: format!("P:{} O:{occupied}", state.active_count),
        cells,
        palette: crate::BehaviorRenderPalette {
            active: [255, 210, 120],
            inactive: crate::palette::BLACK,
            stable: [140, 120, 255],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}
pub fn orbit_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("particleCount", "Particle Count", 1, 16, 1),
        number_item("attractionPct", "Attraction", 0, 100, 1),
        number_item("orbitPct", "Orbit", 0, 100, 1),
        enum_item("repelMode", "Repel Mode", REPEL),
        action_item("resetOrbit", "Reset Orbit"),
        action_item("nudgeAttractor", "Nudge Attractor"),
    ]
}

fn from_config(v: Value) -> OrbitState {
    let c: Config = serde_json::from_value(v).unwrap_or_default();
    let mut s = OrbitState {
        x: norm(c.x, 0, 0, WORLD_MAX_X),
        y: norm(c.y, 0, 0, WORLD_MAX_Y),
        vx: norm(c.vx, 1, -SPEED, SPEED),
        vy: norm(c.vy, 1, -SPEED, SPEED),
        attractor_x: val(c.attractor_x, GRID_WIDTH as i16 * 8, 0, WORLD_MAX_X),
        attractor_y: val(c.attractor_y, GRID_HEIGHT as i16 * 8, 0, WORLD_MAX_Y),
        attractor_vx: val(c.attractor_vx, 3, -SPEED, SPEED),
        attractor_vy: val(c.attractor_vy, 2, -SPEED, SPEED),
        active_count: num(c.particle_count.or(c.active_count), 8, 16) as usize,
        trigger_types: norm_triggers(c.trigger_types),
        attraction_pct: num(c.attraction_pct, 45, 100),
        orbit_pct: num(c.orbit_pct, 55, 100),
        repel_mode: repel(c.repel_mode),
        tick_counter: 0,
    };
    normalize(&mut s);
    s
}
fn normalize(s: &mut OrbitState) {
    s.x = norm(
        Some(s.x.iter().map(|v| Value::from(*v)).collect()),
        0,
        0,
        WORLD_MAX_X,
    );
    s.y = norm(
        Some(s.y.iter().map(|v| Value::from(*v)).collect()),
        0,
        0,
        WORLD_MAX_Y,
    );
    s.vx = norm(
        Some(s.vx.iter().map(|v| Value::from(*v)).collect()),
        1,
        -SPEED,
        SPEED,
    );
    s.vy = norm(
        Some(s.vy.iter().map(|v| Value::from(*v)).collect()),
        1,
        -SPEED,
        SPEED,
    );
    s.active_count = s.active_count.clamp(1, MAX_PARTICLES);
    s.attraction_pct = s.attraction_pct.min(100);
    s.orbit_pct = s.orbit_pct.min(100);
    s.repel_mode = repel(Some(s.repel_mode.clone()));
    s.trigger_types = norm_triggers(Some(s.trigger_types.clone()));
    for i in s.active_count..MAX_PARTICLES {
        s.x[i] = 0;
        s.y[i] = 0;
        s.vx[i] = 1;
        s.vy[i] = 1
    }
}
fn norm(v: Option<Vec<Value>>, d: i16, min: i16, max: i16) -> Vec<i16> {
    let mut o = v
        .unwrap_or_default()
        .into_iter()
        .map(|v| {
            v.as_i64()
                .unwrap_or(i64::from(d))
                .clamp(i64::from(min), i64::from(max)) as i16
        })
        .collect::<Vec<_>>();
    o.resize(MAX_PARTICLES, d);
    o.truncate(MAX_PARTICLES);
    o
}
fn val(v: Option<Value>, d: i16, min: i16, max: i16) -> i16 {
    v.and_then(|v| v.as_i64())
        .unwrap_or(i64::from(d))
        .clamp(i64::from(min), i64::from(max)) as i16
}
fn num(v: Option<Value>, d: u8, max: u8) -> u8 {
    v.and_then(|v| v.as_u64())
        .map(|v| v.clamp(1, u64::from(max)) as u8)
        .unwrap_or(d)
}
fn norm_triggers(v: Option<Vec<CellTriggerType>>) -> Vec<CellTriggerType> {
    let mut o = v.unwrap_or_default();
    o.resize(CELL_COUNT, CellTriggerType::None);
    o.truncate(CELL_COUNT);
    o
}
fn repel(v: Option<String>) -> String {
    let v = v.unwrap_or_else(|| "off".into());
    if REPEL.contains(&v.as_str()) {
        v
    } else {
        "off".into()
    }
}
fn cell(x: i16, y: i16) -> usize {
    grid_index(
        (x / UNIT).clamp(0, GRID_WIDTH as i16 - 1) as usize,
        (y / UNIT).clamp(0, GRID_HEIGHT as i16 - 1) as usize,
    )
}
fn attractor_cell(s: &OrbitState) -> usize {
    cell(s.attractor_x, s.attractor_y)
}
fn occupancy(s: &OrbitState) -> Vec<bool> {
    let mut o = vec![false; CELL_COUNT];
    for i in 0..s.active_count {
        o[cell(s.x[i], s.y[i])] = true;
    }
    o
}
fn triggers(p: &[bool], n: &[bool], force: Option<usize>) -> Vec<CellTriggerType> {
    let mut t = (0..CELL_COUNT)
        .map(|i| match (p[i], n[i]) {
            (false, true) => CellTriggerType::Activate,
            (true, false) => CellTriggerType::Deactivate,
            (true, true) => CellTriggerType::Stable,
            _ => CellTriggerType::None,
        })
        .collect::<Vec<_>>();
    if let Some(i) = force {
        t[i] = CellTriggerType::Activate;
    }
    t
}
fn seed_if_empty(s: &mut OrbitState) {
    if s.x.iter().take(s.active_count).all(|v| *v == 0)
        && s.y.iter().take(s.active_count).all(|v| *v == 0)
    {
        reset_orbit(s)
    }
}
fn reset_orbit(s: &mut OrbitState) {
    s.attractor_x = 64;
    s.attractor_y = 64;
    s.attractor_vx = 3;
    s.attractor_vy = 2;
    for i in 0..s.active_count {
        let a = i % 8;
        let (dx, dy) = [
            (24, 0),
            (16, 16),
            (0, 24),
            (-16, 16),
            (-24, 0),
            (-16, -16),
            (0, -24),
            (16, -16),
        ][a];
        s.x[i] = (s.attractor_x + dx).clamp(0, WORLD_MAX_X);
        s.y[i] = (s.attractor_y + dy).clamp(0, WORLD_MAX_Y);
        s.vx[i] = (-dy / 4).clamp(-SPEED, SPEED);
        s.vy[i] = (dx / 4).clamp(-SPEED, SPEED)
    }
}
fn nudge(s: &mut OrbitState) {
    let d = [(2, -1), (1, 2), (-2, 1), (-1, -2)][(s.tick_counter % 4) as usize];
    s.attractor_vx = (s.attractor_vx + d.0).clamp(-SPEED, SPEED);
    s.attractor_vy = (s.attractor_vy + d.1).clamp(-SPEED, SPEED)
}
fn reflect_particle(s: &mut OrbitState, i: usize) {
    if s.x[i] < 0 {
        s.x[i] = 0;
        s.vx[i] = s.vx[i].abs()
    }
    if s.y[i] < 0 {
        s.y[i] = 0;
        s.vy[i] = s.vy[i].abs()
    }
    if s.x[i] > WORLD_MAX_X {
        s.x[i] = WORLD_MAX_X;
        s.vx[i] = -s.vx[i].abs()
    }
    if s.y[i] > WORLD_MAX_Y {
        s.y[i] = WORLD_MAX_Y;
        s.vy[i] = -s.vy[i].abs()
    }
}
fn reflect_attractor(s: &mut OrbitState) {
    if s.attractor_x < 0 {
        s.attractor_x = 0;
        s.attractor_vx = s.attractor_vx.abs()
    }
    if s.attractor_y < 0 {
        s.attractor_y = 0;
        s.attractor_vy = s.attractor_vy.abs()
    }
    if s.attractor_x > WORLD_MAX_X {
        s.attractor_x = WORLD_MAX_X;
        s.attractor_vx = -s.attractor_vx.abs()
    }
    if s.attractor_y > WORLD_MAX_Y {
        s.attractor_y = WORLD_MAX_Y;
        s.attractor_vy = -s.attractor_vy.abs()
    }
}

#[cfg(test)]
mod tests;
