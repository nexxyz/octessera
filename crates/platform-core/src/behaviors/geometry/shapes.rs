use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, DeviceInput,
    TileTriggerType,
};
use crate::behaviors::native_impl::config_items::{action_item, enum_item, number_item};
use crate::behaviors::native_impl::grid_transitions::{trigger_types_from_tiles, TILE_COUNT};
use crate::behaviors::native_impl::state_codec::{deserialize, restore_trigger_types};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MIN_RADIUS: usize = 4;
const DEFAULT_MAX_RADIUS: usize = MIN_RADIUS;
/// By this radius a ring or filled disk has swept the whole grid from any tile.
const MAX_RADIUS: usize = 12;

fn clamp_radius(radius: usize) -> usize {
    radius.clamp(MIN_RADIUS, MAX_RADIUS)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pulse {
    pub ox: usize,
    pub oy: usize,
    pub radius: usize,
    #[serde(rename = "maxRadius")]
    pub max_radius: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapesState {
    pub pulses: Vec<Pulse>,
    pub lifetimes: Vec<usize>,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<TileTriggerType>,
    #[serde(rename = "pulseShape")]
    pub pulse_shape: String,
    pub lifespan: usize,
    #[serde(rename = "maxRadius")]
    pub max_radius: usize,
    #[serde(rename = "autoPulseInterval")]
    pub auto_pulse_interval: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(rename = "tickCounter", default, skip_serializing)]
    pub tick_counter: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct ShapesConfig {
    #[serde(rename = "pulseShape")]
    pulse_shape: Option<String>,
    lifespan: Option<usize>,
    #[serde(rename = "maxRadius")]
    max_radius: Option<usize>,
    #[serde(rename = "autoPulseInterval")]
    auto_pulse_interval: Option<usize>,
    #[serde(rename = "spawnStep")]
    spawn_step: Option<usize>,
}

/// A saved Shapes world fitted to the grid, so a short or hand-edited save still plays.
pub fn shapes_deserialize(data: Value) -> Result<ShapesState, String> {
    let mut state: ShapesState = deserialize(data)?;
    state.lifetimes.resize(TILE_COUNT, 0);
    state.max_radius = clamp_radius(state.max_radius);
    for pulse in &mut state.pulses {
        pulse.max_radius = clamp_radius(pulse.max_radius);
    }
    state.trigger_types = restore_trigger_types(Some(state.trigger_types));
    Ok(state)
}

pub fn shapes_init(config: Value) -> Result<ShapesState, String> {
    let config: ShapesConfig = serde_json::from_value(config).unwrap_or_default();
    let mut lifetimes = vec![0; TILE_COUNT];
    let center = grid_index(GRID_WIDTH / 2, GRID_HEIGHT / 2);
    lifetimes[center] = config.lifespan.unwrap_or(3);
    Ok(ShapesState {
        pulses: vec![Pulse {
            ox: GRID_WIDTH / 2,
            oy: GRID_HEIGHT / 2,
            radius: 0,
            max_radius: clamp_radius(config.max_radius.unwrap_or(DEFAULT_MAX_RADIUS)),
        }],
        lifetimes,
        trigger_types: vec![TileTriggerType::None; TILE_COUNT],
        pulse_shape: config.pulse_shape.unwrap_or_else(|| "ring".into()),
        lifespan: config.lifespan.unwrap_or(3),
        max_radius: clamp_radius(config.max_radius.unwrap_or(DEFAULT_MAX_RADIUS)),
        auto_pulse_interval: config.auto_pulse_interval.unwrap_or(4),
        spawn_step: config.spawn_step.unwrap_or(2).min(63),
        tick_counter: 0,
    })
}

fn in_shape(shape: &str, cx: usize, cy: usize, ox: usize, oy: usize, r: usize) -> bool {
    let dx = cx as isize - ox as isize;
    let dy = cy as isize - oy as isize;
    let adx = dx.unsigned_abs();
    let ady = dy.unsigned_abs();
    let dist = ((dx * dx + dy * dy) as f32).sqrt();
    match shape {
        "filled" => dist <= r as f32 + 0.5,
        "diamond" => adx + ady <= r,
        "cross" => (adx == r && ady <= 1) || (ady == r && adx <= 1),
        "x" => adx == ady && adx <= r,
        _ => (dist - r as f32).abs() < 0.6,
    }
}

fn shape_tiles(shape: &str, ox: usize, oy: usize, r: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            if in_shape(shape, x, y, ox, oy, r) {
                out.push(grid_index(x, y));
            }
        }
    }
    out
}

fn random_point() -> (usize, usize) {
    let mut rng = crate::behavior_random::rng();
    (rng.gen_range(0..GRID_WIDTH), rng.gen_range(0..GRID_HEIGHT))
}

/// A random tile that is dark this tick, so a scheduled shape always lights its start; any tile
/// when none is dark.
fn dark_point(lifetimes: &[usize]) -> (usize, usize) {
    let dark = (0..TILE_COUNT)
        .filter(|index| lifetimes[*index] == 0)
        .collect::<Vec<_>>();
    if dark.is_empty() {
        return random_point();
    }
    let index = dark[crate::behavior_random::rng().gen_range(0..dark.len())];
    (index % GRID_WIDTH, index / GRID_WIDTH)
}

fn add_pulse(state: &ShapesState, ox: usize, oy: usize) -> ShapesState {
    let previous = state
        .lifetimes
        .iter()
        .map(|life| *life > 0)
        .collect::<Vec<_>>();
    let mut next = state.clone();
    for index in shape_tiles(&state.pulse_shape, ox, oy, 0) {
        next.lifetimes[index] = state.lifespan;
    }
    next.pulses.push(Pulse {
        ox,
        oy,
        radius: 0,
        max_radius: state.max_radius,
    });
    let tiles = next
        .lifetimes
        .iter()
        .map(|life| *life > 0)
        .collect::<Vec<_>>();
    next.trigger_types = trigger_types_from_tiles(&previous, &tiles);
    next
}

pub fn shapes_on_input(
    state: ShapesState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> ShapesState {
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "spawnPulse" =>
        {
            let (x, y) = random_point();
            add_pulse(&state, x, y)
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            add_pulse(&state, x, y)
        }
        _ => state,
    }
}

pub fn shapes_on_tick(state: ShapesState, _context: &mut BehaviorContext) -> ShapesState {
    let previous = state
        .lifetimes
        .iter()
        .map(|life| *life > 0)
        .collect::<Vec<_>>();
    let mut lifetimes = state
        .lifetimes
        .iter()
        .map(|life| life.saturating_sub(1))
        .collect::<Vec<_>>();
    let mut pulses = Vec::new();
    for pulse in &state.pulses {
        let radius = pulse.radius + 1;
        if radius <= pulse.max_radius {
            let prev = if state.pulse_shape == "filled" {
                Vec::new()
            } else {
                shape_tiles(&state.pulse_shape, pulse.ox, pulse.oy, radius - 1)
            };
            for index in shape_tiles(&state.pulse_shape, pulse.ox, pulse.oy, radius) {
                if !prev.contains(&index) {
                    let crossed = previous[index] && lifetimes[index] > 0;
                    let relight = !crossed || state.pulse_shape == "filled";
                    lifetimes[index] = if relight { state.lifespan } else { 0 };
                }
            }
            pulses.push(Pulse {
                radius,
                ..pulse.clone()
            });
        }
    }
    let tick_counter = state.tick_counter + 1;
    let filled_growing = state.pulse_shape == "filled" && !pulses.is_empty();
    if state.auto_pulse_interval > 0
        && !filled_growing
        && (tick_counter - 1) % state.auto_pulse_interval
            == state.spawn_step % state.auto_pulse_interval
    {
        let (x, y) = dark_point(&lifetimes);
        for index in shape_tiles(&state.pulse_shape, x, y, 0) {
            lifetimes[index] = state.lifespan;
        }
        pulses.push(Pulse {
            ox: x,
            oy: y,
            radius: 0,
            max_radius: state.max_radius,
        });
    }
    let tiles = lifetimes.iter().map(|life| *life > 0).collect::<Vec<_>>();
    let trigger_types = trigger_types_from_tiles(&previous, &tiles);
    ShapesState {
        pulses,
        lifetimes,
        trigger_types,
        tick_counter,
        ..state
    }
}

pub fn shapes_render_model(state: &ShapesState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "shapes".into(),
        status_line: format!(
            "{} pulse{} [{}]",
            state.pulses.len(),
            if state.pulses.len() == 1 { "" } else { "s" },
            state.pulse_shape
        ),
        tiles: state.lifetimes.iter().map(|life| *life > 0).collect(),
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::WHITE,
            inactive: crate::palette::BLACK,
            stable: crate::palette::MAGENTA,
        },
        trigger_types: Some(state.trigger_types.clone()),
        tile_colors: None,
    }
}

pub fn shapes_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        enum_item(
            "pulseShape",
            "Shape",
            &["ring", "filled", "diamond", "cross", "x"],
        ),
        number_item("lifespan", "Lifespan", 1, 12, 1),
        number_item(
            "maxRadius",
            "Max Radius",
            MIN_RADIUS as i32,
            MAX_RADIUS as i32,
            1,
        ),
        number_item("autoPulseInterval", "Spawn Interval", 0, 20, 1),
        number_item("spawnStep", "Spawn Step", 0, 63, 1),
        action_item("spawnPulse", "Spawn Pulse"),
    ]
}

pub(crate) fn random_point_for_dla() -> (usize, usize) {
    random_point()
}

#[cfg(test)]
mod tests;
