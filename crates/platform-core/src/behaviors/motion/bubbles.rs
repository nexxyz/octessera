use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{
    action_item, number_item, trigger_types_from_cells, CELL_COUNT,
};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SUBSTEPS: i32 = 64;
const UNIT: i32 = 8;
const HARD_RADIUS_CAP: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bubble {
    pub x: i32,
    pub y: i32,
    pub radius: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BubblesState {
    pub bubbles: Vec<Bubble>,
    pub cells: Vec<bool>,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "minRadius")]
    pub min_radius: usize,
    #[serde(rename = "maxRadius")]
    pub max_radius: usize,
    #[serde(rename = "spawnInterval")]
    pub spawn_interval: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(rename = "spawnCount")]
    pub spawn_count: usize,
    pub drift: i32,
    pub current: i32,
    pub buoyancy: i32,
    #[serde(rename = "maxBubbles")]
    pub max_bubbles: usize,
    #[serde(rename = "tickCounter", default, skip_serializing)]
    pub tick_counter: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct BubblesConfig {
    #[serde(rename = "minRadius")]
    min_radius: Option<usize>,
    #[serde(rename = "maxRadius")]
    max_radius: Option<usize>,
    #[serde(rename = "spawnInterval")]
    spawn_interval: Option<usize>,
    #[serde(rename = "spawnStep")]
    spawn_step: Option<usize>,
    #[serde(rename = "spawnCount")]
    spawn_count: Option<usize>,
    drift: Option<i32>,
    current: Option<i32>,
    buoyancy: Option<i32>,
    #[serde(rename = "maxBubbles")]
    max_bubbles: Option<usize>,
}

fn clamp_radius(value: usize) -> usize {
    value.clamp(1, HARD_RADIUS_CAP)
}

fn normalize(state: &mut BubblesState) {
    state.min_radius = clamp_radius(state.min_radius);
    state.max_radius = clamp_radius(state.max_radius);
    if state.min_radius > state.max_radius {
        state.max_radius = state.min_radius;
    }
    state.spawn_interval = state.spawn_interval.min(30);
    state.spawn_step = state.spawn_step.min(63);
    state.spawn_count = state.spawn_count.clamp(1, 8);
    state.drift = state.drift.clamp(0, 8);
    state.current = state.current.clamp(-8, 8);
    state.buoyancy = state.buoyancy.clamp(1, 8);
    state.max_bubbles = state.max_bubbles.clamp(1, 64);
    for bubble in &mut state.bubbles {
        bubble.radius = bubble.radius.clamp(state.min_radius, state.max_radius);
        bubble.x = bubble.x.clamp(0, (GRID_WIDTH as i32 - 1) * SUBSTEPS);
        bubble.y = bubble.y.clamp(
            0,
            (GRID_HEIGHT as i32 + HARD_RADIUS_CAP as i32 + 1) * SUBSTEPS,
        );
    }
    state.bubbles.retain(|bubble| !fully_above(bubble));
    state.bubbles.truncate(state.max_bubbles);
    if state.cells.len() != CELL_COUNT {
        state.cells = vec![false; CELL_COUNT];
    }
    if state.trigger_types.len() != CELL_COUNT {
        state.trigger_types = vec![CellTriggerType::None; CELL_COUNT];
    }
}

pub fn bubbles_init(config: Value) -> Result<BubblesState, String> {
    let config: BubblesConfig = serde_json::from_value(config).unwrap_or_default();
    let mut state = BubblesState {
        bubbles: vec![],
        cells: vec![false; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        min_radius: config.min_radius.unwrap_or(1),
        max_radius: config.max_radius.unwrap_or(3),
        spawn_interval: config.spawn_interval.unwrap_or(3),
        spawn_step: config.spawn_step.unwrap_or(0),
        spawn_count: config.spawn_count.unwrap_or(2),
        drift: config.drift.unwrap_or(0),
        current: config.current.unwrap_or(0),
        buoyancy: config.buoyancy.unwrap_or(3),
        max_bubbles: config.max_bubbles.unwrap_or(24),
        tick_counter: 0,
    };
    normalize(&mut state);
    Ok(state)
}

pub fn bubbles_deserialize(data: Value) -> Result<BubblesState, String> {
    let mut state: BubblesState =
        serde_json::from_value(data).map_err(|error| error.to_string())?;
    normalize(&mut state);
    let cells = render_cells(&state.bubbles);
    state.trigger_types = trigger_types_from_cells(&cells, &cells);
    state.cells = cells;
    Ok(state)
}

pub fn bubbles_serialize(state: &BubblesState) -> Result<Value, String> {
    let mut next = state.clone();
    normalize(&mut next);
    next.cells = render_cells(&next.bubbles);
    next.trigger_types = trigger_types_from_cells(&next.cells, &next.cells);
    serde_json::to_value(next).map_err(|error| error.to_string())
}

fn random_bubble(state: &BubblesState) -> Bubble {
    let mut rng = rand::thread_rng();
    Bubble {
        x: rng.gen_range(0..GRID_WIDTH) as i32 * SUBSTEPS,
        y: 0,
        radius: rng.gen_range(state.min_radius..=state.max_radius),
    }
}

fn spawn_one(state: &BubblesState, bubbles: &mut Vec<Bubble>) {
    if bubbles.len() < state.max_bubbles {
        bubbles.push(random_bubble(state));
    }
}

pub fn bubbles_on_input(
    state: BubblesState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> BubblesState {
    let mut next = state.clone();
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "addBubble" =>
        {
            spawn_one(&state, &mut next.bubbles)
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            if next.bubbles.len() < next.max_bubbles {
                next.bubbles.push(Bubble {
                    x: x as i32 * SUBSTEPS,
                    y: y as i32 * SUBSTEPS,
                    radius: next.min_radius,
                });
            }
        }
        _ => return state,
    }
    with_rendered_cells(next, &state.cells)
}

fn bubble_cells(bubble: &Bubble) -> Vec<(usize, usize)> {
    let cx = (bubble.x + SUBSTEPS / 2) / SUBSTEPS;
    let cy = (bubble.y + SUBSTEPS / 2) / SUBSTEPS;
    let offsets: &[(i32, i32)] = match bubble.radius {
        1 => &[(0, 0)],
        2 => &[(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)],
        3 => &[(0, -1), (-1, 0), (1, 0), (0, 1)],
        _ => &[
            (-2, -2),
            (-1, -2),
            (0, -2),
            (1, -2),
            (2, -2),
            (-2, -1),
            (2, -1),
            (-2, 0),
            (2, 0),
            (-2, 1),
            (2, 1),
            (-2, 2),
            (-1, 2),
            (0, 2),
            (1, 2),
            (2, 2),
        ],
    };
    offsets
        .iter()
        .filter_map(|(dx, dy)| {
            let x = cx + dx;
            let y = cy + dy;
            (x >= 0 && x < GRID_WIDTH as i32 && y >= 0 && y < GRID_HEIGHT as i32)
                .then_some((x as usize, y as usize))
        })
        .collect()
}

fn touches(a: &Bubble, b: &Bubble) -> bool {
    let cells_a = bubble_cells(a);
    let cells_b = bubble_cells(b);
    cells_a.iter().any(|(ax, ay)| {
        cells_b
            .iter()
            .any(|(bx, by)| (*ax as i32 - *bx as i32).abs() + (*ay as i32 - *by as i32).abs() <= 1)
    })
}

fn merge_bubbles(bubbles: &mut Vec<Bubble>, max_radius: usize) {
    loop {
        let mut merged = false;
        'scan: for left in 0..bubbles.len() {
            for right in left + 1..bubbles.len() {
                if touches(&bubbles[left], &bubbles[right]) {
                    let a = bubbles[left].clone();
                    let b = bubbles[right].clone();
                    let weight = (a.radius + b.radius) as i32;
                    bubbles[left] = Bubble {
                        x: (a.x * a.radius as i32 + b.x * b.radius as i32) / weight,
                        y: (a.y * a.radius as i32 + b.y * b.radius as i32) / weight,
                        radius: (a.radius + b.radius).min(max_radius).min(HARD_RADIUS_CAP),
                    };
                    bubbles.remove(right);
                    merged = true;
                    break 'scan;
                }
            }
        }
        if !merged {
            break;
        }
    }
}

fn fully_above(bubble: &Bubble) -> bool {
    bubble_cells(bubble).is_empty() && bubble.y / SUBSTEPS >= GRID_HEIGHT as i32
}

fn render_cells(bubbles: &[Bubble]) -> Vec<bool> {
    let mut cells = vec![false; CELL_COUNT];
    for bubble in bubbles {
        for (x, y) in bubble_cells(bubble) {
            cells[grid_index(x, y)] = true;
        }
    }
    cells
}

fn with_rendered_cells(mut state: BubblesState, previous_cells: &[bool]) -> BubblesState {
    let cells = render_cells(&state.bubbles);
    let previous = if previous_cells.len() == CELL_COUNT {
        previous_cells.to_vec()
    } else {
        vec![false; CELL_COUNT]
    };
    state.trigger_types = trigger_types_from_cells(&previous, &cells);
    state.cells = cells;
    state
}

pub fn bubbles_on_tick(state: BubblesState, _context: &mut BehaviorContext) -> BubblesState {
    let tick_counter = state.tick_counter + 1;
    let mut rng = rand::thread_rng();
    let mut bubbles = state.bubbles.clone();
    if state.spawn_interval > 0
        && (tick_counter - 1) % state.spawn_interval == state.spawn_step % state.spawn_interval
    {
        for _ in 0..state.spawn_count {
            spawn_one(&state, &mut bubbles);
        }
    }
    for bubble in &mut bubbles {
        let sway = rng.gen_range(-1..=1) * state.drift * UNIT;
        bubble.x =
            (bubble.x + state.current * UNIT + sway).clamp(0, (GRID_WIDTH as i32 - 1) * SUBSTEPS);
        bubble.y += state.buoyancy * UNIT;
    }
    bubbles.retain(|bubble| !fully_above(bubble));
    merge_bubbles(&mut bubbles, state.max_radius);
    let previous_cells = state.cells.clone();
    with_rendered_cells(
        BubblesState {
            bubbles,
            tick_counter,
            ..state
        },
        &previous_cells,
    )
}

pub fn bubbles_render_model(state: &BubblesState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "bubbles".into(),
        status_line: format!(
            "{} bubble{}",
            state.bubbles.len(),
            if state.bubbles.len() == 1 { "" } else { "s" }
        ),
        cells: state.cells.clone(),
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::WHITE,
            inactive: crate::palette::BLUE,
            stable: crate::palette::GRAY,
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn bubbles_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("spawnInterval", "Spawn Interval", 0, 30, 1),
        number_item("spawnStep", "Spawn Step", 0, 63, 1),
        number_item("spawnCount", "Spawn Count", 1, 8, 1),
        number_item("minRadius", "Min Radius", 1, HARD_RADIUS_CAP as i32, 1),
        number_item("maxRadius", "Max Radius", 1, HARD_RADIUS_CAP as i32, 1),
        number_item("drift", "Drift", 0, 8, 1),
        number_item("current", "Current", -8, 8, 1),
        number_item("buoyancy", "Buoyancy", 1, 8, 1),
        number_item("maxBubbles", "Max Bubbles", 1, 64, 1),
        action_item("addBubble", "Add Bubble"),
    ]
}

#[cfg(test)]
#[path = "bubbles_tests.rs"]
mod tests;
