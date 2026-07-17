use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, enum_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SCALE: i32 = 1024;
const MODES: &[&str] = &["mandelbrot", "julia"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FractalExplorerState {
    #[serde(rename = "centerX")]
    pub center_x: i32,
    #[serde(rename = "centerY")]
    pub center_y: i32,
    pub zoom: u16,
    #[serde(rename = "driftX")]
    pub drift_x: i16,
    #[serde(rename = "driftY")]
    pub drift_y: i16,
    #[serde(rename = "juliaCx")]
    pub julia_cx: i16,
    #[serde(rename = "juliaCy")]
    pub julia_cy: i16,
    pub mode: String,
    #[serde(rename = "regionIndex")]
    pub region_index: u8,
    pub classes: Vec<u8>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "zoomRatePct")]
    pub zoom_rate_pct: u8,
    #[serde(rename = "driftPct")]
    pub drift_pct: u8,
    #[serde(rename = "iterationLimit")]
    pub iteration_limit: u8,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
}

#[derive(Default, Deserialize)]
struct Config {
    #[serde(rename = "centerX")]
    center_x: Option<Value>,
    #[serde(rename = "centerY")]
    center_y: Option<Value>,
    zoom: Option<Value>,
    #[serde(rename = "driftX")]
    drift_x: Option<Value>,
    #[serde(rename = "driftY")]
    drift_y: Option<Value>,
    #[serde(rename = "juliaCx")]
    julia_cx: Option<Value>,
    #[serde(rename = "juliaCy")]
    julia_cy: Option<Value>,
    mode: Option<String>,
    #[serde(rename = "fractalMode")]
    fractal_mode: Option<String>,
    #[serde(rename = "regionIndex")]
    region_index: Option<Value>,
    classes: Option<Vec<Value>>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "zoomRatePct")]
    zoom_rate_pct: Option<Value>,
    #[serde(rename = "driftPct")]
    drift_pct: Option<Value>,
    #[serde(rename = "iterationLimit")]
    iteration_limit: Option<Value>,
}

pub fn fractal_explorer_init(config: Value) -> Result<FractalExplorerState, String> {
    let mut s = from_config(config);
    sample_classes(&mut s);
    s.trigger_types = triggers(&s.classes, &s.classes, &[]);
    Ok(s)
}
pub fn fractal_explorer_deserialize(data: Value) -> Result<FractalExplorerState, String> {
    let mut s = from_config(data);
    s.trigger_types = triggers(&s.classes, &s.classes, &[]);
    Ok(s)
}
pub fn fractal_explorer_serialize(state: &FractalExplorerState) -> Result<Value, String> {
    let mut s = state.clone();
    normalize(&mut s);
    serde_json::to_value(s).map_err(|e| e.to_string())
}

pub fn fractal_explorer_on_input(
    mut s: FractalExplorerState,
    input: DeviceInput,
    _: &mut BehaviorContext,
) -> FractalExplorerState {
    normalize(&mut s);
    let prev = s.classes.clone();
    let forced = match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            s.center_x =
                (s.center_x + ((2 * x as i32 - 7) * SCALE / i32::from(s.zoom))).clamp(-4096, 4096);
            s.center_y =
                (s.center_y + ((2 * y as i32 - 7) * SCALE / i32::from(s.zoom))).clamp(-4096, 4096);
            sample_classes(&mut s);
            vec![grid_index(x, y)]
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "jumpRegion" =>
        {
            jump_region(&mut s);
            sample_classes(&mut s);
            vec![]
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "toggleFractalMode" =>
        {
            s.mode = if s.mode == "mandelbrot" {
                "julia".into()
            } else {
                "mandelbrot".into()
            };
            sample_classes(&mut s);
            vec![]
        }
        _ => return s,
    };
    s.trigger_types = triggers(&prev, &s.classes, &forced);
    s
}

pub fn fractal_explorer_on_tick(
    mut s: FractalExplorerState,
    _: &mut BehaviorContext,
) -> FractalExplorerState {
    normalize(&mut s);
    let prev = s.classes.clone();
    sample_classes(&mut s);
    let detail = s.classes.iter().filter(|c| **c == 1).count();
    s.center_x =
        (s.center_x + i32::from(s.drift_x) * i32::from(s.drift_pct) / 100).clamp(-4096, 4096);
    s.center_y =
        (s.center_y + i32::from(s.drift_y) * i32::from(s.drift_pct) / 100).clamp(-4096, 4096);
    s.zoom = s
        .zoom
        .saturating_add(((u32::from(s.zoom_rate_pct) * u32::from(s.zoom) / 1000).max(1)) as u16)
        .clamp(256, 16384);
    if !(4..=52).contains(&detail) || s.zoom >= 16384 {
        jump_region(&mut s);
        sample_classes(&mut s);
    }
    s.tick_counter = s.tick_counter.wrapping_add(1);
    s.trigger_types = triggers(&prev, &s.classes, &[]);
    s
}

pub fn fractal_explorer_render_model(s: &FractalExplorerState) -> BehaviorRenderModel {
    let d = s.classes.iter().filter(|c| **c != 0).count();
    BehaviorRenderModel {
        name: "fractal explorer".into(),
        status_line: format!("D:{d} Z:{}", s.zoom / 1024),
        cells: s.classes.iter().map(|c| *c != 0).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [255, 220, 180],
            inactive: crate::palette::BLACK,
            stable: [100, 80, 200],
        },
        trigger_types: Some(s.trigger_types.clone()),
    }
}
pub fn fractal_explorer_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("zoomRatePct", "Zoom Rate", 0, 100, 1),
        number_item("driftPct", "Drift", 0, 100, 1),
        number_item("iterationLimit", "Iteration Limit", 8, 64, 1),
        enum_item("fractalMode", "Fractal Mode", MODES),
        action_item("jumpRegion", "Jump Region"),
        action_item("toggleFractalMode", "Toggle Fractal Mode"),
    ]
}

fn from_config(v: Value) -> FractalExplorerState {
    let c: Config = serde_json::from_value(v).unwrap_or_default();
    let mode = c
        .fractal_mode
        .or(c.mode)
        .unwrap_or_else(|| "mandelbrot".into());
    let mut s = FractalExplorerState {
        center_x: vali(c.center_x, 0, -4096, 4096),
        center_y: vali(c.center_y, 0, -4096, 4096),
        zoom: valu(c.zoom, 1024, 256, 16384) as u16,
        drift_x: vali(c.drift_x, 3, -64, 64) as i16,
        drift_y: vali(c.drift_y, 2, -64, 64) as i16,
        julia_cx: vali(c.julia_cx, -700, -2048, 2048) as i16,
        julia_cy: vali(c.julia_cy, 270, -2048, 2048) as i16,
        mode: if MODES.contains(&mode.as_str()) {
            mode
        } else {
            "mandelbrot".into()
        },
        region_index: valu(c.region_index, 0, 0, 7) as u8,
        classes: norm_classes(c.classes),
        trigger_types: norm_triggers(c.trigger_types),
        zoom_rate_pct: valu(c.zoom_rate_pct, 8, 0, 100) as u8,
        drift_pct: valu(c.drift_pct, 20, 0, 100) as u8,
        iteration_limit: valu(c.iteration_limit, 24, 8, 64) as u8,
        tick_counter: 0,
    };
    normalize(&mut s);
    s
}
fn normalize(s: &mut FractalExplorerState) {
    s.center_x = s.center_x.clamp(-4096, 4096);
    s.center_y = s.center_y.clamp(-4096, 4096);
    s.zoom = s.zoom.clamp(256, 16384);
    s.drift_x = s.drift_x.clamp(-64, 64);
    s.drift_y = s.drift_y.clamp(-64, 64);
    s.julia_cx = s.julia_cx.clamp(-2048, 2048);
    s.julia_cy = s.julia_cy.clamp(-2048, 2048);
    if !MODES.contains(&s.mode.as_str()) {
        s.mode = "mandelbrot".into()
    }
    s.region_index = s.region_index.min(7);
    s.classes = norm_classes(Some(s.classes.iter().map(|v| Value::from(*v)).collect()));
    s.trigger_types = norm_triggers(Some(s.trigger_types.clone()));
    s.zoom_rate_pct = s.zoom_rate_pct.min(100);
    s.drift_pct = s.drift_pct.min(100);
    s.iteration_limit = s.iteration_limit.clamp(8, 64)
}
fn vali(v: Option<Value>, d: i32, min: i32, max: i32) -> i32 {
    v.and_then(|v| v.as_i64())
        .unwrap_or(i64::from(d))
        .clamp(i64::from(min), i64::from(max)) as i32
}
fn valu(v: Option<Value>, d: u32, min: u32, max: u32) -> u32 {
    v.and_then(|v| v.as_u64())
        .unwrap_or(u64::from(d))
        .clamp(u64::from(min), u64::from(max)) as u32
}
fn norm_classes(v: Option<Vec<Value>>) -> Vec<u8> {
    let mut o = v
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.as_u64().unwrap_or(0).min(2) as u8)
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
fn sample_classes(s: &mut FractalExplorerState) {
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let idx = grid_index(x, y);
            let mx = s.center_x + ((2 * x as i32 - 7) * SCALE * 3) / (2 * i32::from(s.zoom));
            let my = s.center_y + ((2 * y as i32 - 7) * SCALE * 3) / (2 * i32::from(s.zoom));
            let (mut zx, mut zy, cx, cy) = if s.mode == "julia" {
                (mx, my, i32::from(s.julia_cx), i32::from(s.julia_cy))
            } else {
                (0, 0, mx, my)
            };
            let mut iter = 0;
            while iter < s.iteration_limit && zx * zx + zy * zy <= 4 * SCALE * SCALE {
                let nx = (zx * zx - zy * zy) / SCALE + cx;
                zy = (2 * zx * zy) / SCALE + cy;
                zx = nx;
                iter += 1;
            }
            s.classes[idx] = if iter == s.iteration_limit {
                0
            } else if iter >= s.iteration_limit / 4 && iter <= s.iteration_limit * 9 / 10 {
                1
            } else {
                2
            };
        }
    }
}
fn jump_region(s: &mut FractalExplorerState) {
    s.region_index = (s.region_index + 1) % 8;
    let r = [
        (-512, 0, 3, 2, -700, 270),
        (256, 256, -2, 3, 285, 10),
        (-1024, 256, 4, -1, -400, 600),
        (0, -512, 1, 4, 355, 355),
        (768, -256, -3, -2, -800, 156),
        (-256, -768, 2, -3, 45, 618),
        (1024, 0, -4, 1, -726, 188),
        (0, 0, 3, 2, -700, 270),
    ][s.region_index as usize];
    s.center_x = r.0;
    s.center_y = r.1;
    s.drift_x = r.2;
    s.drift_y = r.3;
    s.julia_cx = r.4;
    s.julia_cy = r.5;
    s.zoom = 1024
}
fn triggers(p: &[u8], n: &[u8], forced: &[usize]) -> Vec<CellTriggerType> {
    let mut t = (0..CELL_COUNT)
        .map(|i| {
            if p[i] == 0 && n[i] != 0 || n[i] > p[i] {
                CellTriggerType::Activate
            } else if p[i] != 0 && n[i] == 0 {
                CellTriggerType::Deactivate
            } else if n[i] != 0 {
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

#[cfg(test)]
mod tests;
