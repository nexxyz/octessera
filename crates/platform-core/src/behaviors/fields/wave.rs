use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const THRESHOLD: i16 = 48;
const CLAMP: i16 = 127;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveState {
    pub displacement: Vec<i16>,
    pub velocity: Vec<i16>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "dampingPct")]
    pub damping_pct: u8,
    #[serde(rename = "tensionPct")]
    pub tension_pct: u8,
    #[serde(rename = "impulseStrength")]
    pub impulse_strength: i16,
    #[serde(rename = "autoImpulseInterval")]
    pub auto_impulse_interval: u8,
    #[serde(rename = "spawnStep")]
    pub spawn_step: u8,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
}

#[derive(Default, Deserialize)]
struct Config {
    displacement: Option<Vec<Value>>,
    velocity: Option<Vec<Value>>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "dampingPct")]
    damping_pct: Option<Value>,
    #[serde(rename = "tensionPct")]
    tension_pct: Option<Value>,
    #[serde(rename = "impulseStrength")]
    impulse_strength: Option<Value>,
    #[serde(rename = "autoImpulseInterval")]
    auto_impulse_interval: Option<Value>,
    #[serde(rename = "spawnStep")]
    spawn_step: Option<Value>,
}

pub fn wave_init(config: Value) -> Result<WaveState, String> {
    let seed_default = config
        .as_object()
        .map(|object| !object.contains_key("displacement") && !object.contains_key("velocity"))
        .unwrap_or(true);
    let mut state = state_from_config(config);
    if seed_default
        && state.displacement.iter().all(|value| *value == 0)
        && state.velocity.iter().all(|value| *value == 0)
    {
        impulse(&mut state, grid_index(GRID_WIDTH / 2, GRID_HEIGHT / 2));
    }
    state.trigger_types = triggers(&state.displacement, &state.displacement, &[]);
    Ok(state)
}

pub fn wave_deserialize(data: Value) -> Result<WaveState, String> {
    let mut state = state_from_config(data);
    state.trigger_types = triggers(&state.displacement, &state.displacement, &[]);
    Ok(state)
}

pub fn wave_serialize(state: &WaveState) -> Result<Value, String> {
    let mut state = state.clone();
    normalize(&mut state);
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn wave_on_input(
    mut state: WaveState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> WaveState {
    normalize(&mut state);
    let previous = state.displacement.clone();
    let forced = match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let index = grid_index(x, y);
            impulse(&mut state, index);
            vec![index]
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "dropImpulse" =>
        {
            let index = grid_index(GRID_WIDTH / 2, GRID_HEIGHT / 2);
            impulse(&mut state, index);
            cardinal_with_center(index)
        }
        _ => return state,
    };
    state.trigger_types = triggers(&previous, &state.displacement, &forced);
    state
}

pub fn wave_on_tick(mut state: WaveState, _context: &mut BehaviorContext) -> WaveState {
    normalize(&mut state);
    let previous = state.displacement.clone();
    let mut next_displacement = state.displacement.clone();
    let mut next_velocity = state.velocity.clone();
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            let average = neighbor_average(&state.displacement, x, y);
            let force = (average - state.displacement[index]) * i16::from(state.tension_pct) / 100;
            let velocity =
                (state.velocity[index] + force) * (100 - i16::from(state.damping_pct)) / 100;
            next_velocity[index] = velocity.clamp(-CLAMP, CLAMP);
            next_displacement[index] = ((state.displacement[index] + next_velocity[index])
                * (100 - i16::from(state.damping_pct))
                / 100)
                .clamp(-CLAMP, CLAMP);
        }
    }
    state.displacement = next_displacement;
    state.velocity = next_velocity;
    state.tick_counter = state.tick_counter.wrapping_add(1);
    let forced = if state.auto_impulse_interval > 0
        && state.tick_counter % u64::from(state.auto_impulse_interval)
            == u64::from(state.spawn_step % state.auto_impulse_interval)
    {
        let index =
            scheduled_impulse_index(state.tick_counter / u64::from(state.auto_impulse_interval));
        impulse(&mut state, index);
        cardinal_with_center(index)
    } else {
        vec![]
    };
    state.trigger_types = triggers(&previous, &state.displacement, &forced);
    state
}

pub fn wave_render_model(state: &WaveState) -> BehaviorRenderModel {
    let energy = state
        .displacement
        .iter()
        .map(|value| value.unsigned_abs() as usize)
        .sum::<usize>()
        / 64;
    let peaks = state
        .trigger_types
        .iter()
        .filter(|trigger| **trigger == CellTriggerType::Activate)
        .count();
    BehaviorRenderModel {
        name: "wave".into(),
        status_line: format!("energy:{energy} peaks:{peaks}"),
        cells: state
            .displacement
            .iter()
            .map(|value| value.abs() >= 12)
            .collect(),
        palette: crate::BehaviorRenderPalette {
            active: [180, 240, 255],
            inactive: crate::palette::BLACK,
            stable: [30, 90, 180],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn wave_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("dampingPct", "Damping", 0, 100, 1),
        number_item("tensionPct", "Tension", 0, 100, 1),
        number_item("impulseStrength", "Impulse Strength", 1, 127, 1),
        number_item("autoImpulseInterval", "Impulse Interval", 0, 64, 1),
        number_item("spawnStep", "Spawn Step", 0, 63, 1),
        action_item("dropImpulse", "Drop Impulse"),
    ]
}

fn state_from_config(config: Value) -> WaveState {
    let config: Config = serde_json::from_value(config).unwrap_or_default();
    let mut state = WaveState {
        displacement: normalize_values(config.displacement.unwrap_or_default()),
        velocity: normalize_values(config.velocity.unwrap_or_default()),
        trigger_types: normalize_triggers(config.trigger_types),
        damping_pct: number(config.damping_pct, 4, 100),
        tension_pct: number(config.tension_pct, 45, 100),
        impulse_strength: i16::from(number(config.impulse_strength, 80, 127).max(1)),
        auto_impulse_interval: number(config.auto_impulse_interval, 20, 64),
        spawn_step: number(config.spawn_step, 7, 63),
        tick_counter: 0,
    };
    normalize(&mut state);
    state
}

fn normalize(state: &mut WaveState) {
    state.displacement = normalize_values(
        state
            .displacement
            .iter()
            .map(|value| Value::from(*value))
            .collect(),
    );
    state.velocity = normalize_values(
        state
            .velocity
            .iter()
            .map(|value| Value::from(*value))
            .collect(),
    );
    state.trigger_types = normalize_triggers(Some(state.trigger_types.clone()));
    state.damping_pct = state.damping_pct.min(100);
    state.tension_pct = state.tension_pct.min(100);
    state.impulse_strength = state.impulse_strength.clamp(1, 127);
    state.auto_impulse_interval = state.auto_impulse_interval.min(64);
    state.spawn_step = state.spawn_step.min(63);
}

fn normalize_values(values: Vec<Value>) -> Vec<i16> {
    let mut normalized = values
        .into_iter()
        .map(|value| {
            value
                .as_i64()
                .unwrap_or(0)
                .clamp(i64::from(-CLAMP), i64::from(CLAMP)) as i16
        })
        .collect::<Vec<_>>();
    normalized.resize(CELL_COUNT, 0);
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

fn impulse(state: &mut WaveState, index: usize) {
    state.displacement[index] =
        (state.displacement[index] + state.impulse_strength).clamp(-CLAMP, CLAMP);
    state.velocity[index] = state.impulse_strength / 2;
}

fn scheduled_impulse_index(step: u64) -> usize {
    let points = [(2, 2), (5, 5), (2, 6), (6, 1)];
    let (x, y) = points[step as usize % points.len()];
    grid_index(x, y)
}

fn neighbor_average(values: &[i16], x: usize, y: usize) -> i16 {
    let mut sum = 0;
    for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
            if nx < GRID_WIDTH && ny < GRID_HEIGHT {
                sum += values[grid_index(nx, ny)];
            } else {
                sum += values[grid_index(x, y)];
            }
        } else {
            sum += values[grid_index(x, y)];
        }
    }
    sum / 4
}

fn cardinal_with_center(index: usize) -> Vec<usize> {
    let mut indices = vec![index];
    let x = index % GRID_WIDTH;
    let y = index / GRID_WIDTH;
    for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
            if nx < GRID_WIDTH && ny < GRID_HEIGHT {
                indices.push(grid_index(nx, ny));
            }
        }
    }
    indices
}

fn triggers(old: &[i16], next: &[i16], force_activate: &[usize]) -> Vec<CellTriggerType> {
    let mut triggers = (0..CELL_COUNT)
        .map(|index| {
            let was_hot = old[index].abs() >= THRESHOLD;
            let is_hot = next[index].abs() >= THRESHOLD;
            if !was_hot && is_hot {
                CellTriggerType::Activate
            } else if was_hot && !is_hot {
                CellTriggerType::Deactivate
            } else if is_hot {
                CellTriggerType::Stable
            } else {
                CellTriggerType::None
            }
        })
        .collect::<Vec<_>>();
    for index in force_activate {
        triggers[*index] = CellTriggerType::Activate;
    }
    triggers
}

#[cfg(test)]
mod tests;
