use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, DeviceInput,
    TileTriggerType,
};
use crate::behavior_random::tile_roll_pct;
use crate::behaviors::native_impl::config_items::{action_item, number_item};
use crate::behaviors::native_impl::grid_transitions::TILE_COUNT;
use crate::behaviors::native_impl::state_codec::restore_trigger_types;
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const EMPTY: u8 = 0;
const STEM: u8 = 1;
const TIP: u8 = 2;
const LEAF: u8 = 3;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VinesState {
    pub tiles: Vec<u8>,
    pub energy: Vec<u8>,
    pub ages: Vec<u8>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<TileTriggerType>,
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
    tiles: Option<Vec<Value>>,
    energy: Option<Vec<Value>>,
    ages: Option<Vec<Value>>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<TileTriggerType>>,
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
    if s.tiles.iter().all(|tile| *tile == EMPTY) {
        for (x, y) in [(3, 0), (4, 0), (3, 1)] {
            let index = grid_index(x, y);
            plant(&mut s, index);
        }
    }
    s.trigger_types = triggers(&s.tiles, &s.tiles);
    Ok(s)
}
pub fn vines_deserialize(data: Value) -> Result<VinesState, String> {
    let mut s = from_config(data);
    s.trigger_types = triggers(&s.tiles, &s.tiles);
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
    let prev = state.tiles.clone();
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
    state.trigger_types = triggers_with_forced(&prev, &state.tiles, &forced);
    state
}
pub fn vines_on_tick(mut state: VinesState, _: &mut BehaviorContext) -> VinesState {
    normalize(&mut state);
    let prev = state.tiles.clone();
    let prev_energy = state.energy.clone();
    let prev_ages = state.ages.clone();
    let mut reserved = [false; TILE_COUNT];
    let mut forced = Vec::new();
    for i in 0..TILE_COUNT {
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
            if tile_roll_pct(state.tick_counter, i, 11) < u32::from(state.growth_pct) {
                let dest = first_unreserved(&candidates, &reserved);
                if let Some(dest) = dest {
                    reserved[dest] = true;
                    state.tiles[i] = STEM;
                    state.ages[i] = 0;
                    plant_with_energy(&mut state, dest, 200);
                    if tile_roll_pct(state.tick_counter, i, 29) < u32::from(state.branch_pct) {
                        if let Some(branch) = first_unreserved(&candidates, &reserved) {
                            reserved[branch] = true;
                            plant_with_energy(&mut state, branch, 200);
                        }
                    }
                }
            }
        }
    }
    for i in 0..TILE_COUNT {
        if state.tiles[i] == STEM && state.ages[i] >= state.prune_age {
            state.tiles[i] = LEAF;
            state.energy[i] /= 2;
            state.ages[i] = 0;
            forced.push(i);
        } else if state.tiles[i] == LEAF
            && state.ages[i] >= state.prune_age / 2
            && tile_roll_pct(state.tick_counter, i, 47) < u32::from(state.growth_pct)
        {
            state.tiles[i] = EMPTY;
            state.energy[i] = 0;
            state.ages[i] = 0
        }
    }
    thin_full_frame(&mut state);
    state.tick_counter = state.tick_counter.wrapping_add(1);
    state.trigger_types = triggers_with_forced(&prev, &state.tiles, &forced);
    state
}
pub fn vines_render_model(state: &VinesState) -> BehaviorRenderModel {
    let visible = state.tiles.iter().filter(|c| **c != EMPTY).count();
    let tips = state.tiles.iter().filter(|c| **c == TIP).count();
    BehaviorRenderModel {
        name: "vines".into(),
        status_line: format!("V:{visible} T:{tips}"),
        tiles: state.tiles.iter().map(|c| *c != EMPTY).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [170, 255, 100],
            inactive: crate::palette::BLACK,
            stable: [40, 160, 60],
        },
        trigger_types: Some(state.trigger_types.clone()),
        tile_colors: None,
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
        tiles: norm(c.tiles, 0, 3),
        energy: norm(c.energy, 0, 255),
        ages: norm(c.ages, 0, 255),
        trigger_types: restore_trigger_types(c.trigger_types),
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
    s.tiles = norm(
        Some(s.tiles.iter().map(|v| Value::from(*v)).collect()),
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
    s.trigger_types = restore_trigger_types(Some(s.trigger_types.clone()));
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
    o.resize(TILE_COUNT, d);
    o.truncate(TILE_COUNT);
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
    s.tiles[i] = TIP;
    s.energy[i] = e;
    s.ages[i] = 0
}
fn prune_action(s: &mut VinesState) {
    let mut rows = (0..TILE_COUNT)
        .filter(|i| s.tiles[*i] != EMPTY && s.tiles[*i] != TIP)
        .map(|i| (s.ages[i], i))
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    for (_, i) in rows.into_iter().take(8) {
        s.tiles[i] = EMPTY;
        s.energy[i] = 0;
        s.ages[i] = 0
    }
}
fn thin_full_frame(s: &mut VinesState) {
    if s.tiles.contains(&EMPTY) {
        return;
    }
    let mut removed = 0;
    for offset in 0..TILE_COUNT {
        let i = ((s.tick_counter as usize * 13) + offset) % TILE_COUNT;
        if removed >= 4 {
            break;
        }
        if s.tiles[i] != TIP || tile_roll_pct(s.tick_counter, i, 67) < 25 {
            s.tiles[i] = EMPTY;
            s.energy[i] = 0;
            s.ages[i] = 0;
            removed += 1;
        }
    }
}
fn candidates(tiles: &[u8], x: usize, y: usize, source_energy: u32, light_bias: u32) -> Vec<usize> {
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
                if tiles[i] == EMPTY {
                    out.push(i)
                }
            }
        }
    }
    out.sort_by(|a, b| {
        score(tiles, *b, source_energy, light_bias).cmp(&score(
            tiles,
            *a,
            source_energy,
            light_bias,
        ))
    });
    out
}
fn score(tiles: &[u8], i: usize, source_energy: u32, light_bias: u32) -> u32 {
    let x = i % GRID_WIDTH;
    let y = i / GRID_WIDTH;
    empty_neighbors(tiles, x, y) * 10 + y as u32 * light_bias / 10 + source_energy / 8
}
fn empty_neighbors(tiles: &[u8], x: usize, y: usize) -> u32 {
    let mut c = 0;
    for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
            if nx < GRID_WIDTH && ny < GRID_HEIGHT && tiles[grid_index(nx, ny)] == EMPTY {
                c += 1
            }
        }
    }
    c
}
fn first_unreserved(candidates: &[usize], reserved: &[bool; TILE_COUNT]) -> Option<usize> {
    candidates.iter().copied().find(|i| !reserved[*i])
}
fn triggers(p: &[u8], n: &[u8]) -> Vec<TileTriggerType> {
    (0..TILE_COUNT)
        .map(|i| {
            if p[i] == EMPTY && n[i] != EMPTY {
                TileTriggerType::Activate
            } else if p[i] != EMPTY && n[i] == EMPTY {
                TileTriggerType::Deactivate
            } else if n[i] != EMPTY {
                TileTriggerType::Stable
            } else {
                TileTriggerType::None
            }
        })
        .collect()
}
fn triggers_with_forced(p: &[u8], n: &[u8], forced: &[usize]) -> Vec<TileTriggerType> {
    let mut trigger_types = triggers(p, n);
    for index in forced {
        trigger_types[*index] = TileTriggerType::Activate;
    }
    trigger_types
}

#[cfg(test)]
mod tests;
