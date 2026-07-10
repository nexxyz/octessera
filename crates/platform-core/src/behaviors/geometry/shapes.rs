use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::common::{
    action_item, enum_item, number_item, trigger_types_from_cells, CELL_COUNT,
};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub trigger_types: Vec<CellTriggerType>,
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
}

pub fn shapes_init(config: Value) -> Result<ShapesState, String> {
    let config: ShapesConfig = serde_json::from_value(config).unwrap_or_default();
    Ok(ShapesState {
        pulses: vec![],
        lifetimes: vec![0; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        pulse_shape: config.pulse_shape.unwrap_or_else(|| "ring".into()),
        lifespan: config.lifespan.unwrap_or(3),
        max_radius: config.max_radius.unwrap_or(12),
        auto_pulse_interval: config.auto_pulse_interval.unwrap_or(0),
        spawn_step: 0,
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

fn shape_cells(shape: &str, ox: usize, oy: usize, r: usize) -> Vec<usize> {
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
    let mut rng = rand::thread_rng();
    (rng.gen_range(0..GRID_WIDTH), rng.gen_range(0..GRID_HEIGHT))
}

fn add_pulse(state: &ShapesState, ox: usize, oy: usize) -> ShapesState {
    let mut next = state.clone();
    for index in shape_cells(&state.pulse_shape, ox, oy, 0) {
        next.lifetimes[index] = state.lifespan;
    }
    next.pulses.push(Pulse {
        ox,
        oy,
        radius: 0,
        max_radius: state.max_radius,
    });
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
            let prev = shape_cells(
                &state.pulse_shape,
                pulse.ox,
                pulse.oy,
                radius.saturating_sub(1),
            );
            for index in shape_cells(&state.pulse_shape, pulse.ox, pulse.oy, radius) {
                if !prev.contains(&index) {
                    lifetimes[index] = state.lifespan;
                }
            }
            pulses.push(Pulse {
                radius,
                ..pulse.clone()
            });
        }
    }
    let tick_counter = state.tick_counter + 1;
    if state.auto_pulse_interval > 0
        && (tick_counter - 1) % state.auto_pulse_interval
            == state.spawn_step % state.auto_pulse_interval
    {
        let (x, y) = random_point();
        for index in shape_cells(&state.pulse_shape, x, y, 0) {
            lifetimes[index] = state.lifespan;
        }
        pulses.push(Pulse {
            ox: x,
            oy: y,
            radius: 0,
            max_radius: state.max_radius,
        });
    }
    let cells = lifetimes.iter().map(|life| *life > 0).collect::<Vec<_>>();
    let trigger_types = trigger_types_from_cells(&previous, &cells);
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
        cells: state.lifetimes.iter().map(|life| *life > 0).collect(),
        trigger_types: Some(state.trigger_types.clone()),
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
        number_item("maxRadius", "Max Radius", 4, 32, 1),
        number_item("autoPulseInterval", "Spawn Interval", 0, 20, 1),
        number_item("spawnStep", "Spawn Step", 0, 63, 1),
        action_item("spawnPulse", "Spawn Pulse"),
    ]
}

pub(crate) fn random_point_for_dla() -> (usize, usize) {
    random_point()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_expands_lifespans_and_wavefront_only_adds_leading_edge() {
        let mut context = BehaviorContext::new(120.0);
        let state = shapes_init(
            serde_json::json!({ "pulseShape": "diamond", "lifespan": 2, "maxRadius": 2 }),
        )
        .unwrap();
        let state = shapes_on_input(state, DeviceInput::GridPress { x: 3, y: 3 }, &mut context);
        assert_eq!(state.pulses[0].radius, 0);
        assert!(state.lifetimes[grid_index(3, 3)] > 0);

        let ticked = shapes_on_tick(state, &mut context);
        assert_eq!(ticked.pulses[0].radius, 1);
        assert!(ticked.lifetimes[grid_index(4, 3)] > 0);

        let ticked = shapes_on_tick(ticked, &mut context);
        assert_eq!(ticked.pulses[0].radius, 2);
        let expired = shapes_on_tick(ticked, &mut context);
        assert!(expired.pulses.is_empty());
    }

    #[test]
    fn shapes_render_and_config_menu_match_contract() {
        let state = shapes_init(Value::Null).unwrap();
        let model = shapes_render_model(&state);
        assert_eq!(model.name, "shapes");
        assert!(model.status_line.contains("ring"));
        assert_eq!(model.trigger_types.as_ref().unwrap().len(), CELL_COUNT);
        assert_eq!(
            shapes_config_menu()
                .iter()
                .map(|item| item.key.as_str())
                .collect::<Vec<_>>(),
            vec![
                "pulseShape",
                "lifespan",
                "maxRadius",
                "autoPulseInterval",
                "spawnStep",
                "spawnPulse"
            ]
        );
    }
}
