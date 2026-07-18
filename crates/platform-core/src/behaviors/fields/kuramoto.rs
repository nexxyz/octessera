use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KuramotoState {
    pub phases: Vec<u8>,
    pub frequencies: Vec<u8>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "couplingPct")]
    pub coupling_pct: u8,
    #[serde(rename = "frequencySpread")]
    pub frequency_spread: u8,
    #[serde(rename = "jitterPct")]
    pub jitter_pct: u8,
    #[serde(rename = "jitterState")]
    pub jitter_state: u32,
}

#[derive(Default, Deserialize)]
struct Config {
    phases: Option<Vec<Value>>,
    frequencies: Option<Vec<Value>>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "couplingPct")]
    coupling_pct: Option<Value>,
    #[serde(rename = "frequencySpread")]
    frequency_spread: Option<Value>,
    #[serde(rename = "jitterPct")]
    jitter_pct: Option<Value>,
    #[serde(rename = "jitterState")]
    jitter_state: Option<Value>,
}

pub fn kuramoto_init(config: Value) -> Result<KuramotoState, String> {
    let mut state = state_from_config(config);
    state.trigger_types = trigger_types(&state.phases, &state.phases, false);
    Ok(state)
}

pub fn kuramoto_deserialize(data: Value) -> Result<KuramotoState, String> {
    let mut state = state_from_config(data);
    state.trigger_types = trigger_types(&state.phases, &state.phases, false);
    Ok(state)
}

pub fn kuramoto_serialize(state: &KuramotoState) -> Result<Value, String> {
    let mut state = state.clone();
    normalize(&mut state);
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn kuramoto_on_input(
    mut state: KuramotoState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> KuramotoState {
    normalize(&mut state);
    let old = state.phases.clone();
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            state.phases[grid_index(x, y)] = 255;
            state.trigger_types = vec![CellTriggerType::None; CELL_COUNT];
            state.trigger_types[grid_index(x, y)] = CellTriggerType::Activate;
            state
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "desyncPulse" =>
        {
            desync(&mut state);
            state.trigger_types = trigger_types(&old, &state.phases, false);
            state
        }
        _ => state,
    }
}

pub fn kuramoto_on_tick(mut state: KuramotoState, _context: &mut BehaviorContext) -> KuramotoState {
    normalize(&mut state);
    let old = state.phases.clone();
    let mut next = old.clone();
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            let correction = neighbor_correction(&old, x, y) * i16::from(state.coupling_pct) / 100;
            let jitter = jitter(&mut state, index);
            let advance = (i16::from(state.frequencies[index]) + correction + jitter).clamp(0, 255);
            next[index] = old[index].wrapping_add(advance as u8);
        }
    }
    state.trigger_types = trigger_types(&old, &next, true);
    state.phases = next;
    state
}

pub fn kuramoto_render_model(state: &KuramotoState) -> BehaviorRenderModel {
    let flash = state
        .trigger_types
        .iter()
        .filter(|trigger| **trigger == CellTriggerType::Activate)
        .count();
    let visible = visible_cells(&state.phases);
    let sync = visible.iter().filter(|cell| **cell).count() * 100 / CELL_COUNT;
    BehaviorRenderModel {
        name: "kuramoto".into(),
        status_line: format!("sync:{sync}% flash:{flash}"),
        cells: visible,
        palette: crate::BehaviorRenderPalette {
            active: [255, 255, 200],
            inactive: crate::palette::BLACK,
            stable: [120, 80, 255],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn kuramoto_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("couplingPct", "Coupling", 0, 100, 1),
        number_item("frequencySpread", "Frequency Spread", 0, 32, 1),
        number_item("jitterPct", "Jitter", 0, 100, 1),
        action_item("desyncPulse", "Desync Pulse"),
    ]
}

fn state_from_config(config: Value) -> KuramotoState {
    let config: Config = serde_json::from_value(config).unwrap_or_default();
    let frequency_spread = number(config.frequency_spread, 10, 32);
    let mut state = KuramotoState {
        phases: normalize_values(config.phases.unwrap_or_default(), 0),
        frequencies: normalize_values(config.frequencies.unwrap_or_default(), 4),
        trigger_types: normalize_triggers(config.trigger_types),
        coupling_pct: number(config.coupling_pct, 10, 100),
        frequency_spread,
        jitter_pct: number(config.jitter_pct, 20, 100),
        jitter_state: config
            .jitter_state
            .and_then(|value| value.as_u64())
            .unwrap_or(1) as u32,
    };
    if state.phases.iter().all(|phase| *phase == 0) {
        for index in 0..CELL_COUNT {
            state.phases[index] = ((index * 37) % 256) as u8;
        }
    }
    if state.frequencies.iter().all(|frequency| *frequency == 4) {
        seed_frequencies(&mut state, frequency_spread);
    }
    normalize(&mut state);
    state
}

fn normalize(state: &mut KuramotoState) {
    state.phases = normalize_values(
        state
            .phases
            .iter()
            .map(|phase| Value::from(*phase))
            .collect(),
        0,
    );
    state.frequencies = normalize_values(
        state
            .frequencies
            .iter()
            .map(|frequency| Value::from(*frequency))
            .collect(),
        4,
    );
    state.trigger_types = normalize_triggers(Some(state.trigger_types.clone()));
    state.coupling_pct = state.coupling_pct.min(100);
    state.frequency_spread = state.frequency_spread.min(32);
    state.jitter_pct = state.jitter_pct.min(100);
    if state.jitter_state == 0 {
        state.jitter_state = 1;
    }
}

fn normalize_values(values: Vec<Value>, default: u8) -> Vec<u8> {
    let mut normalized = values
        .into_iter()
        .map(|value| value.as_u64().unwrap_or(u64::from(default)).min(255) as u8)
        .collect::<Vec<_>>();
    normalized.resize(CELL_COUNT, default);
    normalized.truncate(CELL_COUNT);
    normalized
}

fn normalize_triggers(triggers: Option<Vec<CellTriggerType>>) -> Vec<CellTriggerType> {
    let mut triggers = triggers.unwrap_or_default();
    triggers.resize(CELL_COUNT, CellTriggerType::None);
    triggers.truncate(CELL_COUNT);
    triggers
}

fn number(value: Option<Value>, default: u8, max: u8) -> u8 {
    value
        .and_then(|value| value.as_u64())
        .map(|value| value.min(u64::from(max)) as u8)
        .unwrap_or(default)
}

fn seed_frequencies(state: &mut KuramotoState, spread: u8) {
    for index in 0..CELL_COUNT {
        let offset = ((index * 13 + index / GRID_WIDTH) % (usize::from(spread) + 1)) as u8;
        state.frequencies[index] = 4 + offset;
    }
}

fn neighbor_correction(phases: &[u8], x: usize, y: usize) -> i16 {
    let mut sum = 0;
    let mut count = 0;
    for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
            if nx < GRID_WIDTH && ny < GRID_HEIGHT {
                sum += circular_diff(phases[grid_index(x, y)], phases[grid_index(nx, ny)]);
                count += 1;
            }
        }
    }
    if count == 0 {
        0
    } else {
        sum / count
    }
}

fn circular_diff(from: u8, to: u8) -> i16 {
    let diff = i16::from(to) - i16::from(from);
    if diff > 127 {
        diff - 256
    } else if diff < -128 {
        diff + 256
    } else {
        diff
    }
}

fn jitter(state: &mut KuramotoState, index: usize) -> i16 {
    if state.jitter_pct == 0 {
        return 0;
    }
    state.jitter_state = state
        .jitter_state
        .wrapping_mul(1_664_525)
        .wrapping_add(1_013_904_223 + index as u32);
    if (state.jitter_state % 100) < u32::from(state.jitter_pct) {
        ((state.jitter_state >> 8) % 5) as i16 - 2
    } else {
        0
    }
}

fn desync(state: &mut KuramotoState) {
    for index in 0..CELL_COUNT {
        state.phases[index] = state.phases[index].wrapping_add(((index * 29 + 17) % 97) as u8);
    }
    state.jitter_state = state.jitter_state.wrapping_add(0x9e37_79b9);
}

fn visible_cells(phases: &[u8]) -> Vec<bool> {
    phases
        .iter()
        .map(|phase| *phase >= 240 || *phase <= 12)
        .collect()
}

fn trigger_types(old: &[u8], next: &[u8], allow_wrap: bool) -> Vec<CellTriggerType> {
    let visible = visible_cells(next);
    (0..CELL_COUNT)
        .map(|index| {
            if allow_wrap && next[index] < old[index] {
                CellTriggerType::Activate
            } else if visible[index] {
                CellTriggerType::Stable
            } else {
                CellTriggerType::None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
