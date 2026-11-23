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
const BLEACH_AGE: u8 = 32;
const A: u8 = 1;
const B: u8 = 2;
const DEAD: u8 = 3;
const CARDINAL_OFFSETS: [(isize, isize); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

struct ForcedTrigger {
    index: usize,
    trigger_type: TileTriggerType,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoralState {
    pub tiles: Vec<u8>,
    pub ages: Vec<u8>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<TileTriggerType>,
    #[serde(rename = "growthPct")]
    pub growth_pct: u8,
    #[serde(rename = "competitionPct")]
    pub competition_pct: u8,
    #[serde(rename = "breakawayAge")]
    pub breakaway_age: u8,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
}

#[derive(Default, Deserialize)]
struct Config {
    tiles: Option<Vec<Value>>,
    ages: Option<Vec<Value>>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<TileTriggerType>>,
    #[serde(rename = "growthPct")]
    growth_pct: Option<Value>,
    #[serde(rename = "competitionPct")]
    competition_pct: Option<Value>,
    #[serde(rename = "breakawayAge")]
    breakaway_age: Option<Value>,
}

pub fn coral_init(config: Value) -> Result<CoralState, String> {
    let mut s = from_config(config);
    if s.tiles.iter().all(|tile| *tile == EMPTY) {
        for (x, y, c) in [(1, 0, A), (2, 0, A), (5, 0, B), (6, 0, B)] {
            let i = grid_index(x, y);
            s.tiles[i] = c;
        }
    }
    s.trigger_types = triggers(&s.tiles, &s.tiles, &[]);
    Ok(s)
}
pub fn coral_deserialize(data: Value) -> Result<CoralState, String> {
    let mut s = from_config(data);
    s.trigger_types = triggers(&s.tiles, &s.tiles, &[]);
    Ok(s)
}
pub fn coral_serialize(state: &CoralState) -> Result<Value, String> {
    let mut s = state.clone();
    normalize(&mut s);
    serde_json::to_value(s).map_err(|e| e.to_string())
}
pub fn coral_on_input(
    mut state: CoralState,
    input: DeviceInput,
    _: &mut BehaviorContext,
) -> CoralState {
    normalize(&mut state);
    let prev = state.tiles.clone();
    let mut forced = Vec::new();
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let i = grid_index(x, y);
            state.tiles[i] = match state.tiles[i] {
                EMPTY | DEAD => A,
                A => B,
                _ => EMPTY,
            };
            state.ages[i] = 0;
            forced.push(ForcedTrigger {
                index: i,
                trigger_type: if state.tiles[i] == EMPTY {
                    TileTriggerType::Deactivate
                } else {
                    TileTriggerType::Activate
                },
            })
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "seedCoral" =>
        {
            plant_seeds(&mut state, &mut forced)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "breakCoral" =>
        {
            break_action(&mut state)
        }
        _ => return state,
    }
    state.trigger_types = triggers(&prev, &state.tiles, &forced);
    state
}
pub fn coral_on_tick(mut state: CoralState, _: &mut BehaviorContext) -> CoralState {
    normalize(&mut state);
    let prev = state.tiles.clone();
    let prev_ages = state.ages.clone();
    let mut forced = Vec::new();
    for i in 0..TILE_COUNT {
        if prev[i] != EMPTY {
            state.ages[i] = prev_ages[i].saturating_add(1)
        }
    }
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let i = grid_index(x, y);
            if prev[i] != EMPTY {
                continue;
            }
            if tile_roll_pct(state.tick_counter, i, 7) >= u32::from(state.growth_pct) {
                continue;
            }
            if let Some(c) = best_colony(&prev, x, y) {
                let mut kin = 0;
                for_each_cardinal(x, y, |n| kin += u32::from(prev[n] == c));
                if kin == 1
                    && tile_roll_pct(state.tick_counter, i, 19)
                        < u32::from(state.competition_pct) * 3 / 4
                {
                    continue;
                }
                state.tiles[i] = c;
                state.ages[i] = 0;
                forced.push(ForcedTrigger {
                    index: i,
                    trigger_type: TileTriggerType::Activate,
                })
            }
        }
    }
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let i = grid_index(x, y);
            if prev[i] == A || prev[i] == B {
                let opp = if prev[i] == A { B } else { A };
                if any_cardinal(x, y, |n| prev[n] == opp)
                    && tile_roll_pct(state.tick_counter, i, 13) < u32::from(state.competition_pct)
                {
                    state.tiles[i] = DEAD;
                    state.ages[i] = 0
                }
            }
        }
    }
    for i in 0..TILE_COUNT {
        if matches!(state.tiles[i], A | B) && state.ages[i] >= BLEACH_AGE {
            state.tiles[i] = DEAD;
            state.ages[i] = 0
        }
        if state.tiles[i] == DEAD && state.ages[i] >= state.breakaway_age {
            state.tiles[i] = EMPTY;
            state.ages[i] = 0
        }
    }
    if !state.tiles.iter().any(|tile| matches!(*tile, A | B)) {
        plant_seeds(&mut state, &mut forced);
    }
    state.tick_counter = state.tick_counter.wrapping_add(1);
    state.trigger_types = triggers(&prev, &state.tiles, &forced);
    state
}
pub fn coral_render_model(state: &CoralState) -> BehaviorRenderModel {
    let a = state.tiles.iter().filter(|c| **c == A).count();
    let b = state.tiles.iter().filter(|c| **c == B).count();
    let d = state.tiles.iter().filter(|c| **c == DEAD).count();
    BehaviorRenderModel {
        name: "coral".into(),
        status_line: format!("A:{a} B:{b} D:{d}"),
        tiles: state.tiles.iter().map(|c| matches!(*c, A | B)).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [255, 140, 100],
            inactive: crate::palette::BLACK,
            stable: [100, 200, 180],
        },
        trigger_types: Some(state.trigger_types.clone()),
        tile_colors: None,
    }
}
pub fn coral_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("growthPct", "Growth", 0, 100, 1),
        number_item("competitionPct", "Competition", 0, 100, 1),
        number_item("breakawayAge", "Breakaway Age", 1, 64, 1),
        action_item("seedCoral", "Seed Coral"),
        action_item("breakCoral", "Break Coral"),
    ]
}

fn from_config(v: Value) -> CoralState {
    let c: Config = serde_json::from_value(v).unwrap_or_default();
    let mut s = CoralState {
        tiles: norm(c.tiles, 0, 3),
        ages: norm(c.ages, 0, 255),
        trigger_types: restore_trigger_types(c.trigger_types),
        growth_pct: num(c.growth_pct, 35, 100),
        competition_pct: num(c.competition_pct, 20, 100),
        breakaway_age: num(c.breakaway_age, 30, 64).max(1),
        tick_counter: 0,
    };
    normalize(&mut s);
    s
}
fn normalize(s: &mut CoralState) {
    s.tiles = norm(
        Some(s.tiles.iter().map(|v| Value::from(*v)).collect()),
        0,
        3,
    );
    s.ages = norm(
        Some(s.ages.iter().map(|v| Value::from(*v)).collect()),
        0,
        255,
    );
    s.trigger_types = restore_trigger_types(Some(s.trigger_types.clone()));
    s.growth_pct = s.growth_pct.min(100);
    s.competition_pct = s.competition_pct.min(100);
    s.breakaway_age = s.breakaway_age.clamp(1, 64)
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
fn for_each_cardinal(x: usize, y: usize, mut f: impl FnMut(usize)) {
    for (dx, dy) in CARDINAL_OFFSETS {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
            if nx < GRID_WIDTH && ny < GRID_HEIGHT {
                f(grid_index(nx, ny))
            }
        }
    }
}
fn any_cardinal(x: usize, y: usize, mut f: impl FnMut(usize) -> bool) -> bool {
    let mut found = false;
    for_each_cardinal(x, y, |index| {
        if f(index) {
            found = true;
        }
    });
    found
}
fn best_colony(tiles: &[u8], x: usize, y: usize) -> Option<u8> {
    let mut best = (0, A);
    for c in [A, B] {
        let mut same = 0;
        let mut empty = 0;
        for_each_cardinal(x, y, |i| {
            if tiles[i] == c {
                same += 1;
            }
            if tiles[i] == EMPTY {
                empty += 1;
            }
        });
        if same == 0 {
            continue;
        }
        let score = same * 10 + empty * 3;
        if score > best.0 {
            best = (score, c)
        }
    }
    if best.0 == 0 {
        None
    } else {
        Some(best.1)
    }
}
fn exposed(tiles: &[u8], i: usize) -> bool {
    let x = i % GRID_WIDTH;
    let y = i / GRID_WIDTH;
    any_cardinal(x, y, |n| tiles[n] == EMPTY)
}
fn plant_seeds(state: &mut CoralState, forced: &mut Vec<ForcedTrigger>) {
    for (x, y, c) in [(1, 0, A), (2, 0, A), (5, 0, B), (6, 0, B)] {
        let i = grid_index(x, y);
        state.tiles[i] = c;
        state.ages[i] = 0;
        forced.push(ForcedTrigger {
            index: i,
            trigger_type: TileTriggerType::Activate,
        })
    }
}
fn break_action(s: &mut CoralState) {
    let mut rows = (0..TILE_COUNT)
        .filter(|i| s.tiles[*i] == DEAD)
        .map(|i| (0, s.ages[i], i))
        .chain(
            (0..TILE_COUNT)
                .filter(|i| (s.tiles[*i] == A || s.tiles[*i] == B) && exposed(&s.tiles, *i))
                .map(|i| (1, s.ages[i], i)),
        )
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    for (_, _, i) in rows.into_iter().take(8) {
        s.tiles[i] = EMPTY;
        s.ages[i] = 0
    }
}
fn triggers(p: &[u8], n: &[u8], forced: &[ForcedTrigger]) -> Vec<TileTriggerType> {
    let live = |tile: u8| matches!(tile, A | B);
    let mut t = (0..TILE_COUNT)
        .map(|i| match (live(p[i]), live(n[i])) {
            (false, true) => TileTriggerType::Activate,
            (true, false) => TileTriggerType::Deactivate,
            (true, true) => TileTriggerType::Stable,
            (false, false) => TileTriggerType::None,
        })
        .collect::<Vec<_>>();
    for i in forced {
        t[i.index] = i.trigger_type;
    }
    t
}

#[cfg(test)]
mod tests;
