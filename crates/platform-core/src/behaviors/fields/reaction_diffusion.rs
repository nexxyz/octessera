use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const VISIBLE: u8 = 32;
const ACTIVATE: u8 = 96;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReactionDiffusionState {
    pub a: Vec<u8>,
    pub b: Vec<u8>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "feedPct")]
    pub feed_pct: u8,
    #[serde(rename = "killPct")]
    pub kill_pct: u8,
    #[serde(rename = "diffusionPct")]
    pub diffusion_pct: u8,
    #[serde(rename = "reactionPct")]
    pub reaction_pct: u8,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
}

#[derive(Default, Deserialize)]
struct Config {
    a: Option<Vec<Value>>,
    b: Option<Vec<Value>>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "feedPct")]
    feed_pct: Option<Value>,
    #[serde(rename = "killPct")]
    kill_pct: Option<Value>,
    #[serde(rename = "diffusionPct")]
    diffusion_pct: Option<Value>,
    #[serde(rename = "reactionPct")]
    reaction_pct: Option<Value>,
}

pub fn reaction_diffusion_init(config: Value) -> Result<ReactionDiffusionState, String> {
    let mut s = from_config(config);
    s.trigger_types = triggers(&s.b, &s.b, &[]);
    Ok(s)
}
pub fn reaction_diffusion_deserialize(data: Value) -> Result<ReactionDiffusionState, String> {
    let mut s = from_config(data);
    s.trigger_types = triggers(&s.b, &s.b, &[]);
    Ok(s)
}
pub fn reaction_diffusion_serialize(state: &ReactionDiffusionState) -> Result<Value, String> {
    let mut s = state.clone();
    normalize(&mut s);
    serde_json::to_value(s).map_err(|e| e.to_string())
}

pub fn reaction_diffusion_on_input(
    mut state: ReactionDiffusionState,
    input: DeviceInput,
    _: &mut BehaviorContext,
) -> ReactionDiffusionState {
    normalize(&mut state);
    let prev = state.b.clone();
    let forced = match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            splash(&mut state, x, y)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "seedChemicals" =>
        {
            seed(&mut state)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "clearChemicals" =>
        {
            state.a.fill(255);
            state.b.fill(0);
            vec![]
        }
        _ => return state,
    };
    state.trigger_types = triggers(&prev, &state.b, &forced);
    state
}

pub fn reaction_diffusion_on_tick(
    mut state: ReactionDiffusionState,
    _: &mut BehaviorContext,
) -> ReactionDiffusionState {
    normalize(&mut state);
    let pa = state.a.clone();
    let pb = state.b.clone();
    let mut na = pa.clone();
    let mut nb = pb.clone();
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let i = grid_index(x, y);
            let (avg_a, avg_b) = avg(&pa, &pb, x, y);
            let lap_a = avg_a - i32::from(pa[i]);
            let lap_b = avg_b - i32::from(pb[i]);
            let reaction =
                (u32::from(pa[i]) * u32::from(pb[i]) * u32::from(pb[i]) / (255 * 255)) as i32;
            let a = i32::from(pa[i]) + i32::from(state.diffusion_pct) * lap_a / 100
                - i32::from(state.reaction_pct) * reaction / 100
                + i32::from(state.feed_pct) * (255 - i32::from(pa[i])) / 100;
            let b = i32::from(pb[i])
                + i32::from(state.diffusion_pct) * lap_b / 100
                + i32::from(state.reaction_pct) * reaction / 100
                - i32::from(state.kill_pct) * i32::from(pb[i]) / 100;
            na[i] = a.clamp(0, 255) as u8;
            nb[i] = b.clamp(0, 255) as u8;
        }
    }
    state.a = na;
    state.b = nb;
    state.tick_counter = state.tick_counter.wrapping_add(1);
    state.trigger_types = triggers(&pb, &state.b, &[]);
    state
}

pub fn reaction_diffusion_render_model(state: &ReactionDiffusionState) -> BehaviorRenderModel {
    let visible = state.b.iter().filter(|v| **v >= VISIBLE).count();
    let edges = edge_count(&state.b);
    BehaviorRenderModel {
        name: "reaction diffusion".into(),
        status_line: format!("B:{visible} E:{edges}"),
        cells: state.b.iter().map(|v| *v >= VISIBLE).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [120, 255, 220],
            inactive: crate::palette::BLACK,
            stable: [40, 120, 100],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}
pub fn reaction_diffusion_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("feedPct", "Feed", 0, 100, 1),
        number_item("killPct", "Kill", 0, 100, 1),
        number_item("diffusionPct", "Diffusion", 0, 100, 1),
        number_item("reactionPct", "Reaction", 0, 100, 1),
        action_item("seedChemicals", "Seed Chemicals"),
        action_item("clearChemicals", "Clear Chemicals"),
    ]
}

fn from_config(v: Value) -> ReactionDiffusionState {
    let c: Config = serde_json::from_value(v).unwrap_or_default();
    let mut s = ReactionDiffusionState {
        a: norm(c.a, 255),
        b: norm(c.b, 0),
        trigger_types: norm_triggers(c.trigger_types),
        feed_pct: num(c.feed_pct, 35, 100),
        kill_pct: num(c.kill_pct, 55, 100),
        diffusion_pct: num(c.diffusion_pct, 35, 100),
        reaction_pct: num(c.reaction_pct, 50, 100),
        tick_counter: 0,
    };
    normalize(&mut s);
    s
}
fn normalize(s: &mut ReactionDiffusionState) {
    s.a = norm(Some(s.a.iter().map(|v| Value::from(*v)).collect()), 255);
    s.b = norm(Some(s.b.iter().map(|v| Value::from(*v)).collect()), 0);
    s.trigger_types = norm_triggers(Some(s.trigger_types.clone()));
    s.feed_pct = s.feed_pct.min(100);
    s.kill_pct = s.kill_pct.min(100);
    s.diffusion_pct = s.diffusion_pct.min(100);
    s.reaction_pct = s.reaction_pct.min(100)
}
fn norm(v: Option<Vec<Value>>, d: u8) -> Vec<u8> {
    let mut o = v
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.as_u64().unwrap_or(u64::from(d)).min(255) as u8)
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
fn splash(s: &mut ReactionDiffusionState, x: usize, y: usize) -> Vec<usize> {
    let mut f = Vec::new();
    apply(s, grid_index(x, y), 160, 80, &mut f);
    for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
            if nx < GRID_WIDTH && ny < GRID_HEIGHT {
                apply(s, grid_index(nx, ny), 80, 40, &mut f)
            }
        }
    }
    f
}
fn apply(s: &mut ReactionDiffusionState, i: usize, add_b: u8, sub_a: u8, f: &mut Vec<usize>) {
    let old = s.b[i];
    s.b[i] = s.b[i].saturating_add(add_b);
    s.a[i] = s.a[i].saturating_sub(sub_a);
    if s.b[i] > old {
        f.push(i)
    }
}
fn seed(s: &mut ReactionDiffusionState) -> Vec<usize> {
    let mut f = Vec::new();
    for (x, y) in [(3, 3), (4, 4), (1, 1), (6, 6)] {
        f.extend(splash(s, x, y));
    }
    f
}
fn avg(a: &[u8], b: &[u8], x: usize, y: usize) -> (i32, i32) {
    let mut sa = 0;
    let mut sb = 0;
    let mut c = 0;
    for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
            if nx < GRID_WIDTH && ny < GRID_HEIGHT {
                let i = grid_index(nx, ny);
                sa += i32::from(a[i]);
                sb += i32::from(b[i]);
                c += 1
            }
        }
    }
    if c == 0 {
        (
            i32::from(a[grid_index(x, y)]),
            i32::from(b[grid_index(x, y)]),
        )
    } else {
        (sa / c, sb / c)
    }
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
        t[*i] = CellTriggerType::Activate;
    }
    t
}
fn edge_count(b: &[u8]) -> usize {
    let mut count = 0;
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let i = grid_index(x, y);
            if b[i] < VISIBLE {
                continue;
            }
            let edge = [(0, 1), (1, 0), (0, -1), (-1, 0)].iter().any(|(dx, dy)| {
                match (x.checked_add_signed(*dx), y.checked_add_signed(*dy)) {
                    (Some(nx), Some(ny)) if nx < GRID_WIDTH && ny < GRID_HEIGHT => {
                        b[grid_index(nx, ny)] < VISIBLE
                    }
                    _ => true,
                }
            });
            if edge {
                count += 1
            }
        }
    }
    count
}

#[cfg(test)]
mod tests;
