use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const VISIBLE: u8 = 16;
const ACTIVATE: u8 = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandRipplesState {
    pub sand: Vec<u8>,
    pub crest: Vec<u8>,
    #[serde(rename = "windDir")]
    pub wind_dir: String,
    #[serde(rename = "gustTicks", skip_serializing, skip_deserializing)]
    pub gust_ticks: u8,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "windStrengthPct")]
    pub wind_strength_pct: u8,
    #[serde(rename = "depositionPct")]
    pub deposition_pct: u8,
    #[serde(rename = "erosionPct")]
    pub erosion_pct: u8,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
}
#[derive(Default, Deserialize)]
struct Config {
    sand: Option<Vec<Value>>,
    crest: Option<Vec<Value>>,
    #[serde(rename = "windDir")]
    wind_dir: Option<String>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "windStrengthPct")]
    wind_strength_pct: Option<Value>,
    #[serde(rename = "depositionPct")]
    deposition_pct: Option<Value>,
    #[serde(rename = "erosionPct")]
    erosion_pct: Option<Value>,
}

pub fn sand_ripples_init(config: Value) -> Result<SandRipplesState, String> {
    let mut s = from_config(config);
    if s.sand.iter().all(|value| *value == 0) && s.crest.iter().all(|value| *value == 0) {
        seed_dunes(&mut s);
    }
    let r = rendered(&s);
    s.trigger_types = triggers(&r, &r, &[]);
    Ok(s)
}
pub fn sand_ripples_deserialize(data: Value) -> Result<SandRipplesState, String> {
    let mut s = from_config(data);
    let r = rendered(&s);
    s.trigger_types = triggers(&r, &r, &[]);
    Ok(s)
}
pub fn sand_ripples_serialize(state: &SandRipplesState) -> Result<Value, String> {
    let mut s = state.clone();
    normalize(&mut s);
    serde_json::to_value(s).map_err(|e| e.to_string())
}
pub fn sand_ripples_on_input(
    mut s: SandRipplesState,
    input: DeviceInput,
    _: &mut BehaviorContext,
) -> SandRipplesState {
    normalize(&mut s);
    let prev = rendered(&s);
    let mut forced = Vec::new();
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let i = grid_index(x, y);
            let old = (s.sand[i], s.crest[i]);
            s.sand[i] = s.sand[i].saturating_add(96);
            s.crest[i] = s.crest[i].saturating_add(64);
            if (s.sand[i], s.crest[i]) != old && rendered_cell(&s, i) >= VISIBLE {
                forced.push(i)
            }
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "gust" =>
        {
            s.gust_ticks = 1;
            s.trigger_types = triggers(&prev, &prev, &[]);
            return s;
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "shiftWind" =>
        {
            s.wind_dir = match s.wind_dir.as_str() {
                "east" => "north",
                "north" => "west",
                "west" => "south",
                _ => "east",
            }
            .into();
            s.trigger_types = triggers(&prev, &prev, &[]);
            return s;
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "seedDunes" =>
        {
            forced = seed_dunes(&mut s);
        }
        _ => return s,
    }
    let next = rendered(&s);
    s.trigger_types = triggers(&prev, &next, &forced);
    s
}
pub fn sand_ripples_on_tick(mut s: SandRipplesState, _: &mut BehaviorContext) -> SandRipplesState {
    normalize(&mut s);
    let prev = rendered(&s);
    let ps = s.sand.clone();
    let pc = s.crest.clone();
    let mut ns = ps.clone();
    let mut nc = pc.iter().map(|v| v.saturating_sub(1)).collect::<Vec<_>>();
    let mut forced = Vec::new();
    let wind =
        (u16::from(s.wind_strength_pct) + if s.gust_ticks > 0 { 35 } else { 0 }).min(100) as u8;
    for i in 0..CELL_COUNT {
        if wind == 0 {
            continue;
        }
        if ps[i] <= pc[i] / 2 || hash_pct(s.tick_counter, i, 7) >= u32::from(s.erosion_pct) {
            continue;
        }
        if let Some(d) = downwind(i, &s.wind_dir) {
            let amt = (u16::from(ps[i]) * u16::from(wind) / 100 / 4)
                .max(1)
                .min(u16::from(ps[i])) as u8;
            ns[i] = ns[i].saturating_sub(amt);
            ns[d] = ns[d].saturating_add(amt);
            forced.push(d);
            if hash_pct(s.tick_counter, d, 17) < u32::from(s.deposition_pct) {
                nc[d] = nc[d].saturating_add((amt / 2).max(1))
            }
        }
    }
    s.sand = ns;
    s.crest = nc;
    s.gust_ticks = s.gust_ticks.saturating_sub(1);
    s.tick_counter = s.tick_counter.wrapping_add(1);
    let next = rendered(&s);
    s.trigger_types = triggers(&prev, &next, &forced);
    s
}
pub fn sand_ripples_render_model(s: &SandRipplesState) -> BehaviorRenderModel {
    let r = rendered(s);
    let d = r.iter().filter(|v| **v >= VISIBLE).count();
    BehaviorRenderModel {
        name: "sand ripples".into(),
        status_line: format!("D:{d} W:{}", wind_label(&s.wind_dir)),
        cells: r.iter().map(|v| *v >= VISIBLE).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [240, 210, 120],
            inactive: crate::palette::BLACK,
            stable: [150, 120, 70],
        },
        trigger_types: Some(s.trigger_types.clone()),
    }
}
pub fn sand_ripples_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("windStrengthPct", "Wind Strength", 0, 100, 1),
        number_item("depositionPct", "Deposition", 0, 100, 1),
        number_item("erosionPct", "Erosion", 0, 100, 1),
        action_item("gust", "Gust"),
        action_item("shiftWind", "Shift Wind"),
        action_item("seedDunes", "Seed Dunes"),
    ]
}
fn from_config(v: Value) -> SandRipplesState {
    let c: Config = serde_json::from_value(v).unwrap_or_default();
    let mut s = SandRipplesState {
        sand: norm(c.sand),
        crest: norm(c.crest),
        wind_dir: wind(c.wind_dir),
        gust_ticks: 0,
        trigger_types: norm_triggers(c.trigger_types),
        wind_strength_pct: num(c.wind_strength_pct, 45),
        deposition_pct: num(c.deposition_pct, 35),
        erosion_pct: num(c.erosion_pct, 25),
        tick_counter: 0,
    };
    normalize(&mut s);
    s
}
fn normalize(s: &mut SandRipplesState) {
    s.sand = norm(Some(s.sand.iter().map(|v| Value::from(*v)).collect()));
    s.crest = norm(Some(s.crest.iter().map(|v| Value::from(*v)).collect()));
    s.wind_dir = wind(Some(s.wind_dir.clone()));
    s.wind_strength_pct = s.wind_strength_pct.min(100);
    s.deposition_pct = s.deposition_pct.min(100);
    s.erosion_pct = s.erosion_pct.min(100);
    s.trigger_types = norm_triggers(Some(s.trigger_types.clone()))
}
fn norm(v: Option<Vec<Value>>) -> Vec<u8> {
    let mut o = v
        .unwrap_or_default()
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
fn num(v: Option<Value>, d: u8) -> u8 {
    v.and_then(|v| v.as_u64())
        .map(|v| v.min(100) as u8)
        .unwrap_or(d)
}
fn wind(v: Option<String>) -> String {
    match v.as_deref() {
        Some("west") => "west",
        Some("north") => "north",
        Some("south") => "south",
        _ => "east",
    }
    .into()
}
fn wind_label(w: &str) -> &'static str {
    match w {
        "west" => "W",
        "north" => "N",
        "south" => "S",
        _ => "E",
    }
}
fn seed_dunes(s: &mut SandRipplesState) -> Vec<usize> {
    let mut forced = Vec::new();
    for (x, y) in [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 4),
        (4, 4),
        (5, 5),
        (6, 6),
        (7, 6),
    ] {
        let i = grid_index(x, y);
        s.sand[i] = s.sand[i].max(120);
        s.crest[i] = s.crest[i].max(96);
        if rendered_cell(s, i) >= VISIBLE {
            forced.push(i)
        }
    }
    forced
}
fn downwind(i: usize, w: &str) -> Option<usize> {
    let x = i % GRID_WIDTH;
    let y = i / GRID_WIDTH;
    let (nx, ny) = match w {
        "west" => (x.checked_sub(1)?, y),
        "north" => (x, y + 1),
        "south" => (x, y.checked_sub(1)?),
        _ => (x + 1, y),
    };
    if nx < GRID_WIDTH && ny < GRID_HEIGHT {
        Some(grid_index(nx, ny))
    } else {
        None
    }
}
fn rendered(s: &SandRipplesState) -> Vec<u8> {
    (0..CELL_COUNT).map(|i| rendered_cell(s, i)).collect()
}
fn rendered_cell(s: &SandRipplesState, i: usize) -> u8 {
    s.sand[i].max(s.crest[i])
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
        if n[*i] >= VISIBLE {
            t[*i] = CellTriggerType::Activate
        }
    }
    t
}

#[cfg(test)]
mod tests;
