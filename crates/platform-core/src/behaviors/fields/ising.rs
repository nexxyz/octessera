use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, DeviceInput,
    TileTriggerType,
};
use crate::behaviors::native_impl::config_items::{action_item, number_item};
use crate::behaviors::native_impl::grid_transitions::TILE_COUNT;
use crate::behaviors::native_impl::state_codec::restore_trigger_types;
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::{Rng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Energies and temperatures count in 1/200 of one agreeing neighbor, so every setting is a whole
/// number and the flip odds come out the same on every platform.
const UNITS_PER_NEIGHBOR: i32 = 200;
/// Temperature 100 is 6.5 neighbors hot; the 2D Ising critical point (about 2.27) sits near 35.
const TEMPERATURE_UNITS_PER_PCT: i32 = 13;
/// Field Strength 100 is three quarters of a neighbor: a strong pull that still leaves room for
/// domains.
const FIELD_UNITS_AT_FULL: i32 = 150;

/// The Metropolis odds of a flip that costs `energy`, as a threshold for a `u32` roll: certain when
/// it frees energy, never at temperature 0, otherwise `exp(-energy / temperature)`, at most a half
/// so a hot grid does not strobe. The exponential is built from multiplications only, so it is
/// bit-exact everywhere.
fn flip_threshold(energy: i32, temperature: i32) -> u32 {
    if energy < 0 {
        return u32::MAX;
    }
    if temperature == 0 {
        return 0;
    }
    let mut odds = 1.0 - f64::from(energy) / f64::from(temperature) / 1_048_576.0;
    if odds <= 0.0 {
        return 0;
    }
    for _ in 0..20 {
        odds *= odds;
    }
    (odds.min(0.5) * 4_294_967_296.0) as u32
}

fn pct_threshold(pct: u32) -> u32 {
    (u64::from(pct.min(100)) * u64::from(u32::MAX) / 100) as u32
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsingState {
    pub spins: Vec<i8>,
    #[serde(rename = "fieldSign")]
    pub field_sign: i8,
    #[serde(rename = "heatTicks", skip_serializing, skip_deserializing)]
    pub heat_ticks: u8,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<TileTriggerType>,
    #[serde(rename = "temperaturePct")]
    pub temperature_pct: u8,
    #[serde(rename = "fieldStrengthPct")]
    pub field_strength_pct: u8,
    #[serde(rename = "noisePct")]
    pub noise_pct: u8,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
}

#[derive(Default, Deserialize)]
struct Config {
    spins: Option<Vec<Value>>,
    #[serde(rename = "fieldSign")]
    field_sign: Option<Value>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<TileTriggerType>>,
    #[serde(rename = "temperaturePct")]
    temperature_pct: Option<Value>,
    #[serde(rename = "fieldStrengthPct")]
    field_strength_pct: Option<Value>,
    #[serde(rename = "noisePct")]
    noise_pct: Option<Value>,
}

pub fn ising_init(config: Value) -> Result<IsingState, String> {
    let mut s = from_config(config);
    s.trigger_types = triggers(&s.spins, &s.spins);
    Ok(s)
}
pub fn ising_deserialize(data: Value) -> Result<IsingState, String> {
    let mut s = from_config(data);
    s.trigger_types = triggers(&s.spins, &s.spins);
    Ok(s)
}
pub fn ising_serialize(state: &IsingState) -> Result<Value, String> {
    let mut s = state.clone();
    normalize(&mut s);
    serde_json::to_value(s).map_err(|e| e.to_string())
}

pub fn ising_on_input(
    mut state: IsingState,
    input: DeviceInput,
    _: &mut BehaviorContext,
) -> IsingState {
    normalize(&mut state);
    let prev = state.spins.clone();
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let i = grid_index(x, y);
            state.spins[i] *= -1;
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "heatPulse" =>
        {
            state.heat_ticks = 1;
            state.trigger_types = triggers(&state.spins, &state.spins);
            return state;
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "flipField" =>
        {
            state.field_sign *= -1;
            state.trigger_types = triggers(&state.spins, &state.spins);
            return state;
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "randomizeSpins" =>
        {
            randomize(&mut state)
        }
        _ => return state,
    }
    state.trigger_types = triggers(&prev, &state.spins);
    state
}

pub fn ising_on_tick(mut state: IsingState, _: &mut BehaviorContext) -> IsingState {
    normalize(&mut state);
    let prev = state.spins.clone();
    let mut next = prev.clone();
    let heat_bonus = if state.heat_ticks > 0 { 35 } else { 0 };
    let temperature = i32::from(state.temperature_pct) * TEMPERATURE_UNITS_PER_PCT;
    let field =
        i32::from(state.field_sign) * i32::from(state.field_strength_pct) * FIELD_UNITS_AT_FULL
            / 100;
    let noise = pct_threshold(u32::from(state.noise_pct) + heat_bonus) / 2;
    let parity = (state.tick_counter % 2) as usize;
    let mut rng = crate::behavior_random::rng();
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            if (x + y) % 2 != parity {
                continue;
            }
            let i = grid_index(x, y);
            let energy =
                2 * i32::from(prev[i]) * (neighbor_sum(&prev, x, y) * UNITS_PER_NEIGHBOR + field);
            let accept = flip_threshold(energy, temperature);
            if rng.next_u32() < accept || rng.next_u32() < noise {
                next[i] = -prev[i];
            }
        }
    }
    state.spins = next;
    state.tick_counter = state.tick_counter.wrapping_add(1);
    state.heat_ticks = state.heat_ticks.saturating_sub(1);
    state.trigger_types = triggers(&prev, &state.spins);
    state
}

pub fn ising_render_model(state: &IsingState) -> BehaviorRenderModel {
    let positives = state.spins.iter().filter(|s| **s > 0).count();
    let sign = if state.field_sign > 0 { "+" } else { "-" };
    BehaviorRenderModel {
        name: "ising".into(),
        status_line: format!("+:{positives} F:{sign}"),
        tiles: state.spins.iter().map(|s| *s > 0).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [255, 220, 80],
            inactive: crate::palette::BLACK,
            stable: [80, 150, 255],
        },
        trigger_types: Some(state.trigger_types.clone()),
        tile_colors: None,
    }
}
pub fn ising_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("temperaturePct", "Temperature", 0, 100, 1),
        number_item("fieldStrengthPct", "Field Strength", 0, 100, 1),
        number_item("noisePct", "Noise", 0, 100, 1),
        action_item("heatPulse", "Heat Pulse"),
        action_item("flipField", "Flip Field"),
        action_item("randomizeSpins", "Randomize Spins"),
    ]
}

fn from_config(v: Value) -> IsingState {
    let c: Config = serde_json::from_value(v).unwrap_or_default();
    let mut s = IsingState {
        spins: norm_spins(c.spins),
        field_sign: sign(c.field_sign, 1),
        heat_ticks: 0,
        trigger_types: restore_trigger_types(c.trigger_types),
        temperature_pct: num(c.temperature_pct, 35),
        field_strength_pct: num(c.field_strength_pct, 5),
        noise_pct: num(c.noise_pct, 8),
        tick_counter: 0,
    };
    normalize(&mut s);
    s
}
fn normalize(s: &mut IsingState) {
    s.spins = norm_spins(Some(s.spins.iter().map(|v| Value::from(*v)).collect()));
    s.field_sign = if s.field_sign < 0 { -1 } else { 1 };
    s.trigger_types = restore_trigger_types(Some(s.trigger_types.clone()));
    s.temperature_pct = s.temperature_pct.min(100);
    s.field_strength_pct = s.field_strength_pct.min(100);
    s.noise_pct = s.noise_pct.min(100)
}
fn norm_spins(v: Option<Vec<Value>>) -> Vec<i8> {
    let source = v.unwrap_or_default();
    let mut out = Vec::with_capacity(TILE_COUNT);
    for i in 0..TILE_COUNT {
        let x = i % GRID_WIDTH;
        let y = i / GRID_WIDTH;
        let default = if (x + y + (x * y % 3)).is_multiple_of(2) {
            1
        } else {
            -1
        };
        let spin = source
            .get(i)
            .and_then(|v| v.as_i64())
            .map(|v| if v < 0 { -1 } else { 1 })
            .unwrap_or(default);
        out.push(spin)
    }
    out
}
fn num(v: Option<Value>, d: u8) -> u8 {
    v.and_then(|v| v.as_u64())
        .map(|v| v.min(100) as u8)
        .unwrap_or(d)
}
fn sign(v: Option<Value>, d: i8) -> i8 {
    v.and_then(|v| v.as_i64())
        .map(|v| if v < 0 { -1 } else { 1 })
        .unwrap_or(d)
}
fn neighbor_sum(spins: &[i8], x: usize, y: usize) -> i32 {
    let mut sum = 0;
    for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
            if nx < GRID_WIDTH && ny < GRID_HEIGHT {
                sum += i32::from(spins[grid_index(nx, ny)])
            }
        }
    }
    sum
}
fn randomize(s: &mut IsingState) {
    let mut rng = crate::behavior_random::rng();
    for spin in &mut s.spins {
        *spin = if rng.gen_bool(0.5) { 1 } else { -1 };
    }
}
fn triggers(p: &[i8], n: &[i8]) -> Vec<TileTriggerType> {
    (0..TILE_COUNT)
        .map(|i| match (p[i], n[i]) {
            (-1, 1) => TileTriggerType::Activate,
            (1, -1) => TileTriggerType::Deactivate,
            (1, 1) => TileTriggerType::Stable,
            _ => TileTriggerType::None,
        })
        .collect()
}

#[cfg(test)]
mod tests;
