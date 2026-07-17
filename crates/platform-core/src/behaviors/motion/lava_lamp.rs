use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const BLOBS: usize = 8;
const VISIBLE: u8 = 32;
const ACTIVATE: u8 = 80;
const MAX_POS: i16 = 7 * 16 + 15;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LavaLampState {
    pub x: Vec<i16>,
    pub y: Vec<i16>,
    pub vx: Vec<i16>,
    pub vy: Vec<i16>,
    pub radius: Vec<u8>,
    #[serde(rename = "activeCount")]
    pub active_count: u8,
    #[serde(rename = "heatTicks", skip_serializing, skip_deserializing)]
    pub heat_ticks: u8,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "blobCount")]
    pub blob_count: u8,
    #[serde(rename = "viscosityPct")]
    pub viscosity_pct: u8,
    #[serde(rename = "heatPct")]
    pub heat_pct: u8,
    #[serde(rename = "mergePct")]
    pub merge_pct: u8,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
    #[serde(rename = "lastMergeCount", skip_serializing, skip_deserializing)]
    pub last_merge_count: u8,
}
#[derive(Default, Deserialize)]
struct Config {
    x: Option<Vec<Value>>,
    y: Option<Vec<Value>>,
    vx: Option<Vec<Value>>,
    vy: Option<Vec<Value>>,
    radius: Option<Vec<Value>>,
    #[serde(rename = "activeCount")]
    active_count: Option<Value>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "blobCount")]
    blob_count: Option<Value>,
    #[serde(rename = "viscosityPct")]
    viscosity_pct: Option<Value>,
    #[serde(rename = "heatPct")]
    heat_pct: Option<Value>,
    #[serde(rename = "mergePct")]
    merge_pct: Option<Value>,
}
pub fn lava_lamp_init(config: Value) -> Result<LavaLampState, String> {
    let mut s = from_config(config);
    let f = field(&s);
    s.trigger_types = triggers(&f, &f, &[]);
    Ok(s)
}
pub fn lava_lamp_deserialize(data: Value) -> Result<LavaLampState, String> {
    lava_lamp_init(data)
}
pub fn lava_lamp_serialize(state: &LavaLampState) -> Result<Value, String> {
    let mut s = state.clone();
    normalize(&mut s);
    serde_json::to_value(s).map_err(|e| e.to_string())
}
pub fn lava_lamp_on_input(
    mut s: LavaLampState,
    input: DeviceInput,
    _: &mut BehaviorContext,
) -> LavaLampState {
    normalize(&mut s);
    let prev = field(&s);
    let mut forced = Vec::new();
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let px = (x as i16) * 16 + 8;
            let py = (y as i16) * 16 + 8;
            if usize::from(s.active_count) < BLOBS {
                let i = usize::from(s.active_count);
                s.active_count += 1;
                s.blob_count = s.active_count;
                s.x[i] = px;
                s.y[i] = py;
                s.vx[i] = 0;
                s.vy[i] = 0;
                s.radius[i] = 18
            } else {
                let i = nearest(&s, px, py);
                s.x[i] = px;
                s.y[i] = py;
                s.vx[i] = 0;
                s.vy[i] = 0
            }
            forced.push(grid_index(x, y))
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "heatLamp" =>
        {
            s.heat_ticks = 1;
            s.trigger_types = triggers(&prev, &prev, &[]);
            return s;
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "resetBlobs" =>
        {
            reset_changed(&mut s, &mut forced)
        }
        _ => return s,
    }
    let next = field(&s);
    s.trigger_types = triggers(&prev, &next, &forced);
    s
}
pub fn lava_lamp_on_tick(mut s: LavaLampState, _: &mut BehaviorContext) -> LavaLampState {
    normalize(&mut s);
    let prev = field(&s);
    let mut forced = Vec::new();
    for i in 0..usize::from(s.active_count) {
        s.vy[i] -= i16::from(s.heat_pct / 20) + i16::from(s.heat_ticks) * 2;
        s.vx[i] += hash_pct(s.tick_counter, i, 5) as i16 % 3 - 1;
        let damp = 100 - i16::from(s.viscosity_pct / 2);
        s.vx[i] = (s.vx[i] * damp / 100).clamp(-12, 12);
        s.vy[i] = (s.vy[i] * damp / 100).clamp(-12, 12);
        s.x[i] += s.vx[i];
        s.y[i] += s.vy[i];
        reflect(&mut s.x[i], &mut s.vx[i]);
        reflect(&mut s.y[i], &mut s.vy[i]);
    }
    merge(&mut s, &mut forced);
    split(&mut s, &mut forced);
    s.heat_ticks = s.heat_ticks.saturating_sub(1);
    s.tick_counter = s.tick_counter.wrapping_add(1);
    let next = field(&s);
    s.trigger_types = triggers(&prev, &next, &forced);
    s
}
pub fn lava_lamp_render_model(s: &LavaLampState) -> BehaviorRenderModel {
    let f = field(s);
    BehaviorRenderModel {
        name: "lava lamp".into(),
        status_line: format!("B:{} M:{}", s.active_count, s.last_merge_count),
        cells: f.iter().map(|v| *v >= VISIBLE).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [255, 120, 80],
            inactive: crate::palette::BLACK,
            stable: [160, 50, 120],
        },
        trigger_types: Some(s.trigger_types.clone()),
    }
}
pub fn lava_lamp_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("blobCount", "Blob Count", 1, 8, 1),
        number_item("viscosityPct", "Viscosity", 0, 100, 1),
        number_item("heatPct", "Heat", 0, 100, 1),
        number_item("mergePct", "Merge", 0, 100, 1),
        action_item("heatLamp", "Heat Lamp"),
        action_item("resetBlobs", "Reset Blobs"),
    ]
}
fn from_config(v: Value) -> LavaLampState {
    let c: Config = serde_json::from_value(v).unwrap_or_default();
    let mut s = defaults();
    s.x = norm_pos(c.x, &s.x);
    s.y = norm_pos(c.y, &s.y);
    s.vx = norm_vel(c.vx, &s.vx);
    s.vy = norm_vel(c.vy, &s.vy);
    s.radius = norm_r(c.radius, &s.radius);
    s.blob_count = num(c.blob_count, 4, 8).max(1);
    s.active_count = num(c.active_count, s.blob_count, 8).max(1);
    s.trigger_types = norm_triggers(c.trigger_types);
    s.viscosity_pct = num(c.viscosity_pct, 40, 100);
    s.heat_pct = num(c.heat_pct, 35, 100);
    s.merge_pct = num(c.merge_pct, 25, 100);
    normalize(&mut s);
    s
}
fn defaults() -> LavaLampState {
    let mut s = LavaLampState {
        x: vec![0; BLOBS],
        y: vec![0; BLOBS],
        vx: vec![0; BLOBS],
        vy: vec![0; BLOBS],
        radius: vec![18; BLOBS],
        active_count: 4,
        heat_ticks: 0,
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        blob_count: 4,
        viscosity_pct: 40,
        heat_pct: 35,
        merge_pct: 25,
        tick_counter: 0,
        last_merge_count: 0,
    };
    let mut f = Vec::new();
    reset(&mut s, &mut f);
    s
}
fn normalize(s: &mut LavaLampState) {
    s.x = norm_pos(
        Some(s.x.iter().map(|v| Value::from(*v)).collect()),
        &defaults().x,
    );
    s.y = norm_pos(
        Some(s.y.iter().map(|v| Value::from(*v)).collect()),
        &defaults().y,
    );
    s.vx = norm_vel(
        Some(s.vx.iter().map(|v| Value::from(*v)).collect()),
        &defaults().vx,
    );
    s.vy = norm_vel(
        Some(s.vy.iter().map(|v| Value::from(*v)).collect()),
        &defaults().vy,
    );
    s.radius = norm_r(
        Some(s.radius.iter().map(|v| Value::from(*v)).collect()),
        &defaults().radius,
    );
    s.active_count = s.active_count.clamp(1, 8);
    s.blob_count = s.blob_count.clamp(1, 8);
    s.viscosity_pct = s.viscosity_pct.min(100);
    s.heat_pct = s.heat_pct.min(100);
    s.merge_pct = s.merge_pct.min(100);
    s.trigger_types = norm_triggers(Some(s.trigger_types.clone()))
}
fn norm_pos(v: Option<Vec<Value>>, d: &[i16]) -> Vec<i16> {
    let mut o = v
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.as_i64().unwrap_or(0).clamp(0, i64::from(MAX_POS)) as i16)
        .collect::<Vec<_>>();
    if o.is_empty() {
        return d.to_vec();
    }
    o.resize(BLOBS, 0);
    o.truncate(BLOBS);
    o
}
fn norm_vel(v: Option<Vec<Value>>, d: &[i16]) -> Vec<i16> {
    let mut o = v
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.as_i64().unwrap_or(0).clamp(-12, 12) as i16)
        .collect::<Vec<_>>();
    if o.is_empty() {
        return d.to_vec();
    }
    o.resize(BLOBS, 0);
    o.truncate(BLOBS);
    o
}
fn norm_r(v: Option<Vec<Value>>, d: &[u8]) -> Vec<u8> {
    let mut o = v
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.as_u64().unwrap_or(18).clamp(8, 40) as u8)
        .collect::<Vec<_>>();
    if o.is_empty() {
        return d.to_vec();
    }
    o.resize(BLOBS, 18);
    o.truncate(BLOBS);
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
fn reset(s: &mut LavaLampState, f: &mut Vec<usize>) {
    let pts = [
        (2, 1, 18, 1, -2),
        (5, 2, 22, -1, -1),
        (3, 5, 16, 1, 1),
        (6, 6, 20, -1, 1),
    ];
    s.active_count = 4;
    s.blob_count = 4;
    for (i, (x, y, r, vx, vy)) in pts.iter().copied().enumerate() {
        s.x[i] = x * 16 + 8;
        s.y[i] = y * 16 + 8;
        s.radius[i] = r;
        s.vx[i] = vx;
        s.vy[i] = vy;
        f.push(grid_index(x as usize, y as usize))
    }
    for i in pts.len()..BLOBS {
        s.x[i] = 0;
        s.y[i] = 0;
        s.vx[i] = 0;
        s.vy[i] = 0;
        s.radius[i] = 18
    }
}
fn reset_changed(s: &mut LavaLampState, f: &mut Vec<usize>) {
    let before = field(s);
    reset(s, &mut Vec::new());
    let after = field(s);
    for (x, y) in [(2, 1), (5, 2), (3, 5), (6, 6)] {
        let index = grid_index(x, y);
        if before[index] != after[index] && after[index] >= VISIBLE {
            f.push(index);
        }
    }
}
fn reflect(p: &mut i16, v: &mut i16) {
    if *p < 0 {
        *p = 0;
        *v = v.abs()
    } else if *p > MAX_POS {
        *p = MAX_POS;
        *v = -v.abs()
    }
}
fn field(s: &LavaLampState) -> Vec<u8> {
    let mut out = vec![0; CELL_COUNT];
    for cy in 0..GRID_HEIGHT {
        for cx in 0..GRID_WIDTH {
            let mut sum = 0u32;
            let px = cx as i32 * 16 + 8;
            let py = cy as i32 * 16 + 8;
            for i in 0..usize::from(s.active_count) {
                let dx = px - i32::from(s.x[i]);
                let dy = py - i32::from(s.y[i]);
                let r = u32::from(s.radius[i]);
                sum = sum.saturating_add(r * r * 16 / ((dx * dx + dy * dy) as u32 + 16));
            }
            out[grid_index(cx, cy)] = sum.min(255) as u8
        }
    }
    out
}
fn nearest(s: &LavaLampState, px: i16, py: i16) -> usize {
    (0..usize::from(s.active_count))
        .min_by_key(|i| {
            let dx = i32::from(s.x[*i] - px);
            let dy = i32::from(s.y[*i] - py);
            dx * dx + dy * dy
        })
        .unwrap_or(0)
}
fn merge(s: &mut LavaLampState, f: &mut Vec<usize>) {
    s.last_merge_count = 0;
    let n = usize::from(s.active_count);
    for i in 0..n {
        for j in i + 1..n {
            let dx = i32::from(s.x[i] - s.x[j]);
            let dy = i32::from(s.y[i] - s.y[j]);
            let avg = i32::from((s.radius[i] + s.radius[j]) / 2);
            if dx * dx + dy * dy < avg * avg
                && hash_pct(s.tick_counter, i * 8 + j, 17) < u32::from(s.merge_pct)
            {
                s.x[i] = (s.x[i] + s.x[j]) / 2;
                s.y[i] = (s.y[i] + s.y[j]) / 2;
                s.vx[i] = (s.vx[i] + s.vx[j]) / 2;
                s.vy[i] = (s.vy[i] + s.vy[j]) / 2;
                s.radius[i] = (s.radius[i] + s.radius[j] / 2).min(40);
                for k in j..n - 1 {
                    s.x[k] = s.x[k + 1];
                    s.y[k] = s.y[k + 1];
                    s.vx[k] = s.vx[k + 1];
                    s.vy[k] = s.vy[k + 1];
                    s.radius[k] = s.radius[k + 1];
                }
                s.active_count -= 1;
                s.blob_count = s.active_count;
                s.last_merge_count = 1;
                f.push(cell_of(s.x[i], s.y[i]));
                return;
            }
        }
    }
}
fn split(s: &mut LavaLampState, f: &mut Vec<usize>) {
    if s.active_count >= 8 {
        return;
    }
    for i in 0..usize::from(s.active_count) {
        if s.radius[i] >= 32
            && hash_pct(s.tick_counter, i, 29)
                < u32::from(s.heat_pct) + u32::from(s.heat_ticks) * 20
        {
            let j = usize::from(s.active_count);
            s.active_count += 1;
            s.blob_count = s.active_count;
            let r = (u16::from(s.radius[i]) * 70 / 100) as u8;
            s.radius[i] = r;
            s.radius[j] = r;
            s.x[j] = (s.x[i] + 8).min(MAX_POS);
            s.y[j] = s.y[i];
            s.x[i] = (s.x[i] - 8).max(0);
            s.vx[j] = -s.vx[i].abs();
            s.vx[i] = s.vx[i].abs();
            s.vy[j] = s.vy[i];
            f.push(cell_of(s.x[i], s.y[i]));
            f.push(cell_of(s.x[j], s.y[j]));
            return;
        }
    }
}
fn cell_of(x: i16, y: i16) -> usize {
    grid_index((x / 16).clamp(0, 7) as usize, (y / 16).clamp(0, 7) as usize)
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
