use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const VISIBLE: u8 = 8;
const ACTIVATE: u8 = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiversState {
    pub height: Vec<u8>,
    pub water: Vec<u8>,
    pub sediment: Vec<u8>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "rainPct")]
    pub rain_pct: u8,
    #[serde(rename = "flowPct")]
    pub flow_pct: u8,
    #[serde(rename = "erosionPct")]
    pub erosion_pct: u8,
    #[serde(rename = "evaporationPct")]
    pub evaporation_pct: u8,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
    #[serde(skip)]
    pub erosion_count: usize,
}
#[derive(Default, Deserialize)]
struct Config {
    height: Option<Vec<Value>>,
    water: Option<Vec<Value>>,
    sediment: Option<Vec<Value>>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "rainPct")]
    rain_pct: Option<Value>,
    #[serde(rename = "flowPct")]
    flow_pct: Option<Value>,
    #[serde(rename = "erosionPct")]
    erosion_pct: Option<Value>,
    #[serde(rename = "evaporationPct")]
    evaporation_pct: Option<Value>,
}

pub fn rivers_init(config: Value) -> Result<RiversState, String> {
    let mut s = from_config(config);
    s.trigger_types = triggers(&s.water, &s.water, &[]);
    Ok(s)
}
pub fn rivers_deserialize(data: Value) -> Result<RiversState, String> {
    let mut s = from_config(data);
    s.trigger_types = triggers(&s.water, &s.water, &[]);
    Ok(s)
}
pub fn rivers_serialize(state: &RiversState) -> Result<Value, String> {
    let mut s = state.clone();
    normalize(&mut s);
    serde_json::to_value(s).map_err(|e| e.to_string())
}
pub fn rivers_on_input(
    mut state: RiversState,
    input: DeviceInput,
    _: &mut BehaviorContext,
) -> RiversState {
    normalize(&mut state);
    let prev = state.water.clone();
    let mut forced = Vec::new();
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let i = grid_index(x, y);
            let old = state.water[i];
            state.water[i] = state.water[i].saturating_add(96);
            state.height[i] = state.height[i].saturating_sub(16);
            if state.water[i] > old {
                forced.push(i)
            }
            state.erosion_count = 1
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "rainBurst" =>
        {
            for (x, y) in [(1, 6), (3, 7), (5, 6), (6, 7)] {
                let i = grid_index(x, y);
                let old = state.water[i];
                state.water[i] = state.water[i].saturating_add(64);
                if state.water[i] > old && state.water[i] >= VISIBLE {
                    forced.push(i)
                }
            }
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "resetTerrain" =>
        {
            state.height = default_height();
            state.water.fill(0);
            state.sediment.fill(0);
            state.erosion_count = 0
        }
        _ => return state,
    }
    state.trigger_types = triggers(&prev, &state.water, &forced);
    state
}
pub fn rivers_on_tick(mut state: RiversState, _: &mut BehaviorContext) -> RiversState {
    normalize(&mut state);
    let ph = state.height.clone();
    let pw = state.water.clone();
    let ps = state.sediment.clone();
    let mut nh = ph.clone();
    let mut nw = pw.clone();
    let mut ns = ps.clone();
    let mut changed = [false; CELL_COUNT];
    let mut forced = Vec::new();
    for (i, water) in nw.iter_mut().enumerate().take(CELL_COUNT) {
        if hash_pct(state.tick_counter, i, 7) < u32::from(state.rain_pct) {
            let old = *water;
            *water = water.saturating_add(16);
            if *water > old && *water >= VISIBLE {
                forced.push(i)
            }
        }
    }
    let rain_water = nw.clone();
    nw.fill(0);
    for i in 0..CELL_COUNT {
        let w = rain_water[i];
        if w == 0 {
            continue;
        }
        if state.flow_pct > 0 {
            if let Some(dest) = lowest(&ph, &rain_water, i) {
                let amt = (u16::from(w) * u16::from(state.flow_pct) / 100)
                    .max(1)
                    .min(u16::from(w)) as u8;
                nw[i] = nw[i].saturating_add(w - amt);
                nw[dest] = nw[dest].saturating_add(amt);
                if hash_pct(state.tick_counter, i, 13) < u32::from(state.erosion_pct) {
                    nh[i] = nh[i].saturating_sub(1);
                    ns[dest] = ns[dest].saturating_add(1);
                    changed[i] = true
                }
                continue;
            }
        }
        nw[i] = nw[i].saturating_add(w)
    }
    for i in 0..CELL_COUNT {
        if state.evaporation_pct > 0 && nw[i] > 0 {
            let evap = (u16::from(nw[i]) * u16::from(state.evaporation_pct) / 100).max(1) as u8;
            nw[i] = nw[i].saturating_sub(evap)
        }
        if ns[i] > 0 && hash_pct(state.tick_counter, i, 29) < u32::from(state.erosion_pct) {
            nh[i] = nh[i].saturating_add(1);
            ns[i] = ns[i].saturating_sub(1);
            changed[i] = true
        }
    }
    state.height = nh;
    state.water = nw;
    state.sediment = ns;
    state.erosion_count = changed.iter().filter(|v| **v).count();
    state.tick_counter = state.tick_counter.wrapping_add(1);
    state.trigger_types = triggers(&pw, &state.water, &forced);
    state
}
pub fn rivers_render_model(s: &RiversState) -> BehaviorRenderModel {
    let w = s.water.iter().filter(|v| **v >= VISIBLE).count();
    BehaviorRenderModel {
        name: "rivers".into(),
        status_line: format!("W:{w} E:{}", s.erosion_count),
        cells: s.water.iter().map(|v| *v >= VISIBLE).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [80, 180, 255],
            inactive: crate::palette::BLACK,
            stable: [40, 90, 180],
        },
        trigger_types: Some(s.trigger_types.clone()),
    }
}
pub fn rivers_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("rainPct", "Rain", 0, 100, 1),
        number_item("flowPct", "Flow", 0, 100, 1),
        number_item("erosionPct", "Erosion", 0, 100, 1),
        number_item("evaporationPct", "Evaporation", 0, 100, 1),
        action_item("rainBurst", "Rain Burst"),
        action_item("resetTerrain", "Reset Terrain"),
    ]
}
fn from_config(v: Value) -> RiversState {
    let c: Config = serde_json::from_value(v).unwrap_or_default();
    let mut s = RiversState {
        height: norm_with_defaults(c.height, default_height()),
        water: norm(c.water, vec![0; CELL_COUNT]),
        sediment: norm(c.sediment, vec![0; CELL_COUNT]),
        trigger_types: norm_triggers(c.trigger_types),
        rain_pct: num(c.rain_pct, 20),
        flow_pct: num(c.flow_pct, 50),
        erosion_pct: num(c.erosion_pct, 15),
        evaporation_pct: num(c.evaporation_pct, 8),
        tick_counter: 0,
        erosion_count: 0,
    };
    normalize(&mut s);
    s
}
fn normalize(s: &mut RiversState) {
    s.height = norm_with_defaults(
        Some(s.height.iter().map(|v| Value::from(*v)).collect()),
        default_height(),
    );
    s.water = norm(
        Some(s.water.iter().map(|v| Value::from(*v)).collect()),
        vec![0; CELL_COUNT],
    );
    s.sediment = norm(
        Some(s.sediment.iter().map(|v| Value::from(*v)).collect()),
        vec![0; CELL_COUNT],
    );
    s.trigger_types = norm_triggers(Some(s.trigger_types.clone()));
    s.rain_pct = s.rain_pct.min(100);
    s.flow_pct = s.flow_pct.min(100);
    s.erosion_pct = s.erosion_pct.min(100);
    s.evaporation_pct = s.evaporation_pct.min(100)
}
fn default_height() -> Vec<u8> {
    (0..CELL_COUNT)
        .map(|i| {
            let x = i % GRID_WIDTH;
            let y = i / GRID_WIDTH;
            (180i32 - y as i32 * 16 + ((x * 17 + y * 11) % 24) as i32).clamp(0, 255) as u8
        })
        .collect()
}
fn norm(v: Option<Vec<Value>>, d: Vec<u8>) -> Vec<u8> {
    let mut o = v
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.as_u64().unwrap_or(0).min(255) as u8)
        .collect::<Vec<_>>();
    if o.is_empty() {
        return d;
    }
    o.resize(CELL_COUNT, 0);
    o.truncate(CELL_COUNT);
    o
}
fn norm_with_defaults(v: Option<Vec<Value>>, defaults: Vec<u8>) -> Vec<u8> {
    let Some(values) = v else {
        return defaults;
    };
    let mut out = defaults;
    for (i, value) in values.into_iter().take(CELL_COUNT).enumerate() {
        out[i] = value.as_u64().unwrap_or(u64::from(out[i])).min(255) as u8;
    }
    out
}
fn norm_triggers(v: Option<Vec<CellTriggerType>>) -> Vec<CellTriggerType> {
    let mut o = v.unwrap_or_default();
    o.resize(CELL_COUNT, CellTriggerType::None);
    o.truncate(CELL_COUNT);
    o
}
fn num(v: Option<Value>, d: u8) -> u8 {
    v.and_then(|v| v.as_u64())
        .map(|v| v.min(100) as u8)
        .unwrap_or(d)
}
fn neigh(i: usize) -> impl Iterator<Item = usize> {
    let x = i % GRID_WIDTH;
    let y = i / GRID_WIDTH;
    [(0, 1), (1, 0), (0, -1), (-1, 0)]
        .into_iter()
        .filter_map(move |(dx, dy)| Some((x.checked_add_signed(dx)?, y.checked_add_signed(dy)?)))
        .filter(|(x, y)| *x < GRID_WIDTH && *y < GRID_HEIGHT)
        .map(|(x, y)| grid_index(x, y))
}
fn lowest(h: &[u8], w: &[u8], i: usize) -> Option<usize> {
    let here = u16::from(h[i]) + u16::from(w[i]);
    neigh(i)
        .filter(|n| u16::from(h[*n]) + u16::from(w[*n]) < here)
        .min_by_key(|n| u16::from(h[*n]) + u16::from(w[*n]))
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
            if p[i] < ACTIVATE && n[i] >= ACTIVATE {
                CellTriggerType::Activate
            } else if p[i] >= VISIBLE && n[i] < VISIBLE {
                CellTriggerType::Deactivate
            } else if n[i] >= VISIBLE {
                CellTriggerType::Stable
            } else {
                CellTriggerType::None
            }
        })
        .collect::<Vec<_>>();
    for i in forced {
        t[*i] = CellTriggerType::Activate
    }
    t
}

#[cfg(test)]
mod tests;
