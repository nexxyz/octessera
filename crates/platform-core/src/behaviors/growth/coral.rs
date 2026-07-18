use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const EMPTY: u8 = 0;
const A: u8 = 1;
const B: u8 = 2;
const DEAD: u8 = 3;
const CARDINAL_OFFSETS: [(isize, isize); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

struct ForcedTrigger {
    index: usize,
    trigger_type: CellTriggerType,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoralState {
    pub cells: Vec<u8>,
    pub ages: Vec<u8>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
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
    cells: Option<Vec<Value>>,
    ages: Option<Vec<Value>>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "growthPct")]
    growth_pct: Option<Value>,
    #[serde(rename = "competitionPct")]
    competition_pct: Option<Value>,
    #[serde(rename = "breakawayAge")]
    breakaway_age: Option<Value>,
}

pub fn coral_init(config: Value) -> Result<CoralState, String> {
    let mut s = from_config(config);
    if s.cells.iter().all(|cell| *cell == EMPTY) {
        for (x, y, c) in [(1, 0, A), (2, 0, A), (5, 0, B), (6, 0, B)] {
            let i = grid_index(x, y);
            s.cells[i] = c;
        }
    }
    s.trigger_types = triggers(&s.cells, &s.cells, &[]);
    Ok(s)
}
pub fn coral_deserialize(data: Value) -> Result<CoralState, String> {
    let mut s = from_config(data);
    s.trigger_types = triggers(&s.cells, &s.cells, &[]);
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
    let prev = state.cells.clone();
    let mut forced = Vec::new();
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let i = grid_index(x, y);
            state.cells[i] = match state.cells[i] {
                EMPTY | DEAD => A,
                A => B,
                _ => EMPTY,
            };
            state.ages[i] = 0;
            forced.push(ForcedTrigger {
                index: i,
                trigger_type: if state.cells[i] == EMPTY {
                    CellTriggerType::Deactivate
                } else {
                    CellTriggerType::Activate
                },
            })
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "seedCoral" =>
        {
            for (x, y, c) in [(1, 0, A), (2, 0, A), (5, 0, B), (6, 0, B)] {
                let i = grid_index(x, y);
                state.cells[i] = c;
                state.ages[i] = 0;
                forced.push(ForcedTrigger {
                    index: i,
                    trigger_type: CellTriggerType::Activate,
                })
            }
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "breakCoral" =>
        {
            break_action(&mut state)
        }
        _ => return state,
    }
    state.trigger_types = triggers(&prev, &state.cells, &forced);
    state
}
pub fn coral_on_tick(mut state: CoralState, _: &mut BehaviorContext) -> CoralState {
    normalize(&mut state);
    let prev = state.cells.clone();
    let prev_ages = state.ages.clone();
    let mut forced = Vec::new();
    for i in 0..CELL_COUNT {
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
            if hash_pct(state.tick_counter, i, 7) >= u32::from(state.growth_pct) {
                continue;
            }
            if let Some(c) = best_colony(&prev, x, y) {
                state.cells[i] = c;
                state.ages[i] = 0;
                forced.push(ForcedTrigger {
                    index: i,
                    trigger_type: CellTriggerType::Activate,
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
                    && hash_pct(state.tick_counter, i, 13) < u32::from(state.competition_pct)
                {
                    state.cells[i] = DEAD;
                    state.ages[i] = 0
                }
            }
        }
    }
    for i in 0..CELL_COUNT {
        if state.cells[i] == DEAD && state.ages[i] >= state.breakaway_age {
            state.cells[i] = EMPTY;
            state.ages[i] = 0
        }
    }
    thin_full_frame(&mut state);
    state.tick_counter = state.tick_counter.wrapping_add(1);
    state.trigger_types = triggers(&prev, &state.cells, &forced);
    state
}
pub fn coral_render_model(state: &CoralState) -> BehaviorRenderModel {
    let a = state.cells.iter().filter(|c| **c == A).count();
    let b = state.cells.iter().filter(|c| **c == B).count();
    let d = state.cells.iter().filter(|c| **c == DEAD).count();
    BehaviorRenderModel {
        name: "coral".into(),
        status_line: format!("A:{a} B:{b} D:{d}"),
        cells: state.cells.iter().map(|c| *c != EMPTY).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [255, 140, 100],
            inactive: crate::palette::BLACK,
            stable: [100, 200, 180],
        },
        trigger_types: Some(state.trigger_types.clone()),
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
        cells: norm(c.cells, 0, 3),
        ages: norm(c.ages, 0, 255),
        trigger_types: norm_triggers(c.trigger_types),
        growth_pct: num(c.growth_pct, 35, 100),
        competition_pct: num(c.competition_pct, 20, 100),
        breakaway_age: num(c.breakaway_age, 30, 64).max(1),
        tick_counter: 0,
    };
    normalize(&mut s);
    s
}
fn normalize(s: &mut CoralState) {
    s.cells = norm(
        Some(s.cells.iter().map(|v| Value::from(*v)).collect()),
        0,
        3,
    );
    s.ages = norm(
        Some(s.ages.iter().map(|v| Value::from(*v)).collect()),
        0,
        255,
    );
    s.trigger_types = norm_triggers(Some(s.trigger_types.clone()));
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
fn best_colony(cells: &[u8], x: usize, y: usize) -> Option<u8> {
    let mut best = (0, A);
    for c in [A, B] {
        let mut same = 0;
        let mut empty = 0;
        for_each_cardinal(x, y, |i| {
            if cells[i] == c {
                same += 1;
            }
            if cells[i] == EMPTY {
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
fn exposed(cells: &[u8], i: usize) -> bool {
    let x = i % GRID_WIDTH;
    let y = i / GRID_WIDTH;
    any_cardinal(x, y, |n| cells[n] == EMPTY)
}
fn break_action(s: &mut CoralState) {
    let mut rows = (0..CELL_COUNT)
        .filter(|i| s.cells[*i] == DEAD)
        .map(|i| (0, s.ages[i], i))
        .chain(
            (0..CELL_COUNT)
                .filter(|i| (s.cells[*i] == A || s.cells[*i] == B) && exposed(&s.cells, *i))
                .map(|i| (1, s.ages[i], i)),
        )
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    for (_, _, i) in rows.into_iter().take(8) {
        s.cells[i] = EMPTY;
        s.ages[i] = 0
    }
}
fn thin_full_frame(s: &mut CoralState) {
    if s.cells.contains(&EMPTY) {
        return;
    }
    let mut removed = 0;
    for offset in 0..CELL_COUNT {
        let i = ((s.tick_counter as usize * 11) + offset) % CELL_COUNT;
        if removed >= 4 {
            break;
        }
        if s.cells[i] == DEAD || exposed(&s.cells, i) || hash_pct(s.tick_counter, i, 71) < 25 {
            s.cells[i] = EMPTY;
            s.ages[i] = 0;
            removed += 1;
        }
    }
}
fn hash_pct(tick: u64, index: usize, salt: u64) -> u32 {
    let mut x = tick
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(index as u64 * 0x85EB_CA6B)
        .wrapping_add(salt * 0xC2B2_AE35);
    x ^= x >> 16;
    ((x.wrapping_mul(0x27D4_EB2D) >> 24) % 100) as u32
}
fn triggers(p: &[u8], n: &[u8], forced: &[ForcedTrigger]) -> Vec<CellTriggerType> {
    let mut t = (0..CELL_COUNT)
        .map(|i| {
            if p[i] == EMPTY && n[i] != EMPTY {
                CellTriggerType::Activate
            } else if p[i] != EMPTY && (n[i] == EMPTY || n[i] == DEAD && p[i] != DEAD) {
                CellTriggerType::Deactivate
            } else if n[i] != EMPTY {
                CellTriggerType::Stable
            } else {
                CellTriggerType::None
            }
        })
        .collect::<Vec<_>>();
    for i in forced {
        t[i.index] = i.trigger_type;
    }
    t
}

#[cfg(test)]
mod tests;
