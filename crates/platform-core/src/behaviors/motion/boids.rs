use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_BOIDS: usize = 24;
const UNIT: i16 = 16;
const WORLD_MAX_X: i16 = (GRID_WIDTH as i16 - 1) * UNIT + 15;
const WORLD_MAX_Y: i16 = (GRID_HEIGHT as i16 - 1) * UNIT + 15;
const NEAR: i16 = 32;
const SPEED: i16 = 12;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoidsState {
    pub x: Vec<i16>,
    pub y: Vec<i16>,
    pub vx: Vec<i16>,
    pub vy: Vec<i16>,
    #[serde(rename = "activeCount")]
    pub active_count: usize,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "separationPct")]
    pub separation_pct: u8,
    #[serde(rename = "alignmentPct")]
    pub alignment_pct: u8,
    #[serde(rename = "cohesionPct")]
    pub cohesion_pct: u8,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
}

#[derive(Default, Deserialize)]
struct Config {
    x: Option<Vec<Value>>,
    y: Option<Vec<Value>>,
    vx: Option<Vec<Value>>,
    vy: Option<Vec<Value>>,
    #[serde(rename = "activeCount")]
    active_count: Option<Value>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "flockSize")]
    flock_size: Option<Value>,
    #[serde(rename = "separationPct")]
    separation_pct: Option<Value>,
    #[serde(rename = "alignmentPct")]
    alignment_pct: Option<Value>,
    #[serde(rename = "cohesionPct")]
    cohesion_pct: Option<Value>,
}

pub fn boids_init(config: Value) -> Result<BoidsState, String> {
    let mut s = from_config(config);
    seed_if_empty(&mut s);
    s.trigger_types = triggers(&occupancy(&s), &occupancy(&s));
    Ok(s)
}
pub fn boids_deserialize(data: Value) -> Result<BoidsState, String> {
    let mut s = from_config(data);
    s.trigger_types = triggers(&occupancy(&s), &occupancy(&s));
    Ok(s)
}
pub fn boids_serialize(state: &BoidsState) -> Result<Value, String> {
    let mut s = state.clone();
    normalize(&mut s);
    serde_json::to_value(s).map_err(|e| e.to_string())
}

pub fn boids_on_input(
    mut state: BoidsState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> BoidsState {
    normalize(&mut state);
    let prev = occupancy(&state);
    let mut force_activate = None;
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            place_boid(&mut state, x, y);
            force_activate = Some(grid_index(x, y));
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "scatterFlock" =>
        {
            scatter(&mut state);
            state.trigger_types = triggers(&prev, &prev);
            return state;
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "seedFlock" =>
        {
            seed_flock(&mut state)
        }
        _ => return state,
    }
    state.trigger_types = triggers(&prev, &occupancy(&state));
    if let Some(index) = force_activate {
        state.trigger_types[index] = CellTriggerType::Activate;
    }
    state
}

pub fn boids_on_tick(mut state: BoidsState, _context: &mut BehaviorContext) -> BoidsState {
    normalize(&mut state);
    let prev_occ = occupancy(&state);
    let ox = state.x.clone();
    let oy = state.y.clone();
    let ovx = state.vx.clone();
    let ovy = state.vy.clone();
    for i in 0..state.active_count {
        let mut sx = 0;
        let mut sy = 0;
        let mut ax = 0;
        let mut ay = 0;
        let mut cx = 0;
        let mut cy = 0;
        let mut n = 0;
        for j in 0..state.active_count {
            if i == j {
                continue;
            }
            let dx = ox[j] - ox[i];
            let dy = oy[j] - oy[i];
            if dx.abs() <= NEAR && dy.abs() <= NEAR {
                n += 1;
                sx -= dx.signum() * UNIT / (dx.abs() / UNIT + 1);
                sy -= dy.signum() * UNIT / (dy.abs() / UNIT + 1);
                ax += ovx[j];
                ay += ovy[j];
                cx += ox[j];
                cy += oy[j];
            }
        }
        if n > 0 {
            ax = ax / n - ovx[i];
            ay = ay / n - ovy[i];
            cx = cx / n - ox[i];
            cy = cy / n - oy[i];
            state.vx[i] = (ovx[i]
                + sx * i16::from(state.separation_pct) / 100
                + ax * i16::from(state.alignment_pct) / 100
                + cx.signum() * i16::from(state.cohesion_pct) / 20)
                .clamp(-SPEED, SPEED);
            state.vy[i] = (ovy[i]
                + sy * i16::from(state.separation_pct) / 100
                + ay * i16::from(state.alignment_pct) / 100
                + cy.signum() * i16::from(state.cohesion_pct) / 20)
                .clamp(-SPEED, SPEED);
        }
        if state.vx[i] == 0 && state.vy[i] == 0 {
            let (hx, hy) = heading(i);
            state.vx[i] = hx;
            state.vy[i] = hy;
        }
        state.x[i] += state.vx[i];
        state.y[i] += state.vy[i];
        reflect(&mut state, i);
    }
    state.tick_counter = state.tick_counter.wrapping_add(1);
    state.trigger_types = triggers(&prev_occ, &occupancy(&state));
    state
}

pub fn boids_render_model(state: &BoidsState) -> BehaviorRenderModel {
    let occ = occupancy(state);
    let cells = occ.iter().filter(|v| **v).count();
    BehaviorRenderModel {
        name: "boids".into(),
        status_line: format!("B:{} C:{cells}", state.active_count),
        cells: occ,
        palette: crate::BehaviorRenderPalette {
            active: [255, 240, 160],
            inactive: crate::palette::BLACK,
            stable: [120, 200, 255],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}
pub fn boids_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("flockSize", "Flock Size", 1, 24, 1),
        number_item("separationPct", "Separation", 0, 100, 1),
        number_item("alignmentPct", "Alignment", 0, 100, 1),
        number_item("cohesionPct", "Cohesion", 0, 100, 1),
        action_item("scatterFlock", "Scatter Flock"),
        action_item("seedFlock", "Seed Flock"),
    ]
}

fn from_config(v: Value) -> BoidsState {
    let c: Config = serde_json::from_value(v).unwrap_or_default();
    let mut s = BoidsState {
        x: norm(c.x, 0, 0, WORLD_MAX_X),
        y: norm(c.y, 0, 0, WORLD_MAX_Y),
        vx: norm(c.vx, 1, -SPEED, SPEED),
        vy: norm(c.vy, 1, -SPEED, SPEED),
        active_count: num(c.flock_size.or(c.active_count), 12, 24) as usize,
        trigger_types: norm_triggers(c.trigger_types),
        separation_pct: num(c.separation_pct, 45, 100),
        alignment_pct: num(c.alignment_pct, 35, 100),
        cohesion_pct: num(c.cohesion_pct, 25, 100),
        tick_counter: 0,
    };
    normalize(&mut s);
    s
}
fn normalize(s: &mut BoidsState) {
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
    s.active_count = s.active_count.clamp(1, MAX_BOIDS);
    s.trigger_types = norm_triggers(Some(s.trigger_types.clone()));
    s.separation_pct = s.separation_pct.min(100);
    s.alignment_pct = s.alignment_pct.min(100);
    s.cohesion_pct = s.cohesion_pct.min(100);
    for i in s.active_count..MAX_BOIDS {
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
    o.resize(MAX_BOIDS, d);
    o.truncate(MAX_BOIDS);
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
        .map(|v| v.clamp(1, u64::from(max)) as u8)
        .unwrap_or(d)
}
fn cell(x: i16, y: i16) -> usize {
    grid_index(
        (x / UNIT).clamp(0, GRID_WIDTH as i16 - 1) as usize,
        (y / UNIT).clamp(0, GRID_HEIGHT as i16 - 1) as usize,
    )
}
fn occupancy(s: &BoidsState) -> Vec<bool> {
    let mut o = vec![false; CELL_COUNT];
    for i in 0..s.active_count {
        o[cell(s.x[i], s.y[i])] = true
    }
    o
}
fn triggers(p: &[bool], n: &[bool]) -> Vec<CellTriggerType> {
    (0..CELL_COUNT)
        .map(|i| match (p[i], n[i]) {
            (false, true) => CellTriggerType::Activate,
            (true, false) => CellTriggerType::Deactivate,
            (true, true) => CellTriggerType::Stable,
            _ => CellTriggerType::None,
        })
        .collect()
}
fn heading(i: usize) -> (i16, i16) {
    match i % 4 {
        0 => (4, 3),
        1 => (-4, 3),
        2 => (4, -3),
        _ => (-4, -3),
    }
}
fn seed_if_empty(s: &mut BoidsState) {
    if s.x.iter().take(s.active_count).all(|v| *v == 0)
        && s.y.iter().take(s.active_count).all(|v| *v == 0)
    {
        seed_flock(s)
    }
}
fn seed_flock(s: &mut BoidsState) {
    s.active_count = s.active_count.clamp(1, MAX_BOIDS);
    for i in 0..s.active_count {
        let col = (i % 6) as i16;
        let row = (i / 6) as i16;
        s.x[i] = 48 + col * 5;
        s.y[i] = 48 + row * 5;
        let (hx, hy) = heading(i);
        s.vx[i] = hx;
        s.vy[i] = hy
    }
}
fn scatter(s: &mut BoidsState) {
    for i in 0..s.active_count {
        let (hx, hy) = heading(i + 1);
        s.vx[i] = hx * 2;
        s.vy[i] = hy * 2
    }
}
fn place_boid(s: &mut BoidsState, x: usize, y: usize) {
    let px = x as i16 * UNIT;
    let py = y as i16 * UNIT;
    let slot = if s.active_count < MAX_BOIDS {
        let i = s.active_count;
        s.active_count += 1;
        i
    } else {
        nearest(s, px, py)
    };
    s.x[slot] = px;
    s.y[slot] = py;
    let (hx, hy) = heading(slot);
    s.vx[slot] = hx;
    s.vy[slot] = hy
}
fn nearest(s: &BoidsState, x: i16, y: i16) -> usize {
    (0..s.active_count)
        .min_by_key(|i| (s.x[*i] - x).abs() + (s.y[*i] - y).abs())
        .unwrap_or(0)
}
fn reflect(s: &mut BoidsState, i: usize) {
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

#[cfg(test)]
mod tests;
