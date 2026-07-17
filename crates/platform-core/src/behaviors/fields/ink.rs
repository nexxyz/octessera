use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{action_item, number_item, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const ACTIVATE: u8 = 96;
const VISIBLE: u8 = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InkState {
    pub ink: Vec<u8>,
    #[serde(rename = "triggerTypes", skip_serializing)]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "diffusionPct")]
    pub diffusion_pct: u8,
    #[serde(rename = "fadePct")]
    pub fade_pct: u8,
    #[serde(rename = "dropStrength")]
    pub drop_strength: u8,
    #[serde(rename = "autoDropInterval")]
    pub auto_drop_interval: u8,
    #[serde(rename = "spawnStep")]
    pub spawn_step: u8,
    #[serde(rename = "tickCounter", skip_serializing, skip_deserializing)]
    pub tick_counter: u64,
}

#[derive(Default, Deserialize)]
struct Config {
    ink: Option<Vec<Value>>,
    #[serde(rename = "triggerTypes")]
    trigger_types: Option<Vec<CellTriggerType>>,
    #[serde(rename = "diffusionPct")]
    diffusion_pct: Option<Value>,
    #[serde(rename = "fadePct")]
    fade_pct: Option<Value>,
    #[serde(rename = "dropStrength")]
    drop_strength: Option<Value>,
    #[serde(rename = "autoDropInterval")]
    auto_drop_interval: Option<Value>,
    #[serde(rename = "spawnStep")]
    spawn_step: Option<Value>,
}

pub fn ink_init(config: Value) -> Result<InkState, String> {
    let seed_default = config
        .as_object()
        .map(|object| !object.contains_key("ink"))
        .unwrap_or(true);
    let mut state = from_config(config);
    if seed_default && state.ink.iter().all(|value| *value == 0) {
        drop_splash_at(&mut state, GRID_WIDTH / 2, GRID_HEIGHT / 2);
    }
    state.trigger_types = triggers(&state.ink, &state.ink, &[]);
    Ok(state)
}

pub fn ink_deserialize(data: Value) -> Result<InkState, String> {
    let mut state = from_config(data);
    state.trigger_types = triggers(&state.ink, &state.ink, &[]);
    Ok(state)
}

pub fn ink_serialize(state: &InkState) -> Result<Value, String> {
    let mut state = state.clone();
    normalize(&mut state);
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn ink_on_input(
    mut state: InkState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> InkState {
    normalize(&mut state);
    let previous = state.ink.clone();
    let forced = match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let index = grid_index(x, y);
            add_ink(&mut state, index);
            vec![index]
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "dropInk" =>
        {
            drop_splash(&mut state)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "clearInk" =>
        {
            state.ink.fill(0);
            vec![]
        }
        _ => return state,
    };
    state.trigger_types = triggers(&previous, &state.ink, &forced);
    state
}

pub fn ink_on_tick(mut state: InkState, _context: &mut BehaviorContext) -> InkState {
    normalize(&mut state);
    let previous = state.ink.clone();
    let mut next = state.ink.clone();
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            let index = grid_index(x, y);
            let average = neighbor_average(&state.ink, x, y);
            let diffused = i16::from(state.ink[index])
                + (average - i16::from(state.ink[index])) * i16::from(state.diffusion_pct) / 100;
            let faded = diffused * (100 - i16::from(state.fade_pct)) / 100;
            next[index] = faded.clamp(0, 255) as u8;
        }
    }
    state.ink = next;
    state.tick_counter = state.tick_counter.wrapping_add(1);
    let forced = if state.auto_drop_interval > 0
        && state.tick_counter % u64::from(state.auto_drop_interval)
            == u64::from(state.spawn_step % state.auto_drop_interval)
    {
        let (x, y) = scheduled_drop_point(state.tick_counter / u64::from(state.auto_drop_interval));
        drop_splash_at(&mut state, x, y)
    } else {
        vec![]
    };
    state.trigger_types = triggers(&previous, &state.ink, &forced);
    state
}

pub fn ink_render_model(state: &InkState) -> BehaviorRenderModel {
    let visible = state.ink.iter().filter(|value| **value >= VISIBLE).count();
    let flashes = state
        .trigger_types
        .iter()
        .filter(|trigger| **trigger == CellTriggerType::Activate)
        .count();
    BehaviorRenderModel {
        name: "ink".into(),
        status_line: format!("I:{visible} F:{flashes}"),
        cells: state.ink.iter().map(|value| *value >= VISIBLE).collect(),
        palette: crate::BehaviorRenderPalette {
            active: [120, 80, 255],
            inactive: crate::palette::BLACK,
            stable: [40, 30, 140],
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn ink_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("diffusionPct", "Diffusion", 0, 100, 1),
        number_item("fadePct", "Fade", 0, 100, 1),
        number_item("dropStrength", "Drop Strength", 1, 255, 1),
        number_item("autoDropInterval", "Drop Interval", 0, 64, 1),
        number_item("spawnStep", "Spawn Step", 0, 63, 1),
        action_item("dropInk", "Drop Ink"),
        action_item("clearInk", "Clear Ink"),
    ]
}

fn from_config(config: Value) -> InkState {
    let config: Config = serde_json::from_value(config).unwrap_or_default();
    let mut state = InkState {
        ink: norm_ink(config.ink.unwrap_or_default()),
        trigger_types: norm_triggers(config.trigger_types),
        diffusion_pct: num(config.diffusion_pct, 30, 100),
        fade_pct: num(config.fade_pct, 5, 100),
        drop_strength: num(config.drop_strength, 180, 255).max(1),
        auto_drop_interval: num(config.auto_drop_interval, 16, 64),
        spawn_step: num(config.spawn_step, 5, 63),
        tick_counter: 0,
    };
    normalize(&mut state);
    state
}

fn normalize(state: &mut InkState) {
    state.ink = norm_ink(state.ink.iter().map(|value| Value::from(*value)).collect());
    state.trigger_types = norm_triggers(Some(state.trigger_types.clone()));
    state.diffusion_pct = state.diffusion_pct.min(100);
    state.fade_pct = state.fade_pct.min(100);
    state.drop_strength = state.drop_strength.clamp(1, 255);
    state.auto_drop_interval = state.auto_drop_interval.min(64);
    state.spawn_step = state.spawn_step.min(63);
}

fn norm_ink(values: Vec<Value>) -> Vec<u8> {
    let mut values = values
        .into_iter()
        .map(|value| value.as_u64().unwrap_or(0).min(255) as u8)
        .collect::<Vec<_>>();
    values.resize(CELL_COUNT, 0);
    values.truncate(CELL_COUNT);
    values
}

fn norm_triggers(values: Option<Vec<CellTriggerType>>) -> Vec<CellTriggerType> {
    let mut values = values.unwrap_or_default();
    values.resize(CELL_COUNT, CellTriggerType::None);
    values.truncate(CELL_COUNT);
    values
}

fn num(value: Option<Value>, default: u8, max: u8) -> u8 {
    value
        .and_then(|value| value.as_u64())
        .map(|value| value.min(u64::from(max)) as u8)
        .unwrap_or(default)
}

fn add_ink(state: &mut InkState, index: usize) {
    state.ink[index] = state.ink[index].saturating_add(state.drop_strength);
}

fn drop_splash(state: &mut InkState) -> Vec<usize> {
    drop_splash_at(state, GRID_WIDTH / 2, GRID_HEIGHT / 2)
}

fn drop_splash_at(state: &mut InkState, x: usize, y: usize) -> Vec<usize> {
    let center = grid_index(x, y);
    let mut affected = vec![center];
    add_ink(state, center);
    for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
            if nx < GRID_WIDTH && ny < GRID_HEIGHT {
                let index = grid_index(nx, ny);
                let amount = state.drop_strength / 2;
                if amount > 0 {
                    state.ink[index] = state.ink[index].saturating_add(amount);
                    affected.push(index);
                }
            }
        }
    }
    affected
}

fn scheduled_drop_point(step: u64) -> (usize, usize) {
    let points = [(2, 2), (5, 4), (3, 6), (6, 1)];
    points[step as usize % points.len()]
}

fn neighbor_average(values: &[u8], x: usize, y: usize) -> i16 {
    let mut sum = 0;
    let mut count = 0;
    for (dx, dy) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
        if let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
            if nx < GRID_WIDTH && ny < GRID_HEIGHT {
                sum += i16::from(values[grid_index(nx, ny)]);
                count += 1;
            }
        }
    }
    if count == 0 {
        i16::from(values[grid_index(x, y)])
    } else {
        sum / count
    }
}

fn triggers(previous: &[u8], next: &[u8], forced: &[usize]) -> Vec<CellTriggerType> {
    let mut triggers = (0..CELL_COUNT)
        .map(|index| {
            if previous[index] < ACTIVATE && next[index] >= ACTIVATE {
                CellTriggerType::Activate
            } else if previous[index] >= VISIBLE && next[index] < VISIBLE {
                CellTriggerType::Deactivate
            } else if next[index] >= VISIBLE {
                CellTriggerType::Stable
            } else {
                CellTriggerType::None
            }
        })
        .collect::<Vec<_>>();
    for index in forced {
        triggers[*index] = CellTriggerType::Activate;
    }
    triggers
}

#[cfg(test)]
mod tests;
