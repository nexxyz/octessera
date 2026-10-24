use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel, CellTriggerType,
    DeviceInput,
};
use crate::behaviors::native_impl::config_items::{action_item, number_item};
use crate::behaviors::native_impl::grid_transitions::{trigger_types_from_cells, CELL_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ball {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BounceState {
    pub balls: Vec<Ball>,
    pub cells: Vec<bool>,
    #[serde(rename = "triggerTypes")]
    pub trigger_types: Vec<CellTriggerType>,
    #[serde(rename = "maxBalls")]
    pub max_balls: usize,
    #[serde(rename = "spawnInterval")]
    pub spawn_interval: usize,
    #[serde(rename = "spawnStep")]
    pub spawn_step: usize,
    #[serde(rename = "tickCounter", default, skip_serializing)]
    pub tick_counter: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct BounceConfig {
    #[serde(rename = "maxBalls")]
    max_balls: Option<usize>,
    #[serde(rename = "spawnInterval")]
    spawn_interval: Option<usize>,
    #[serde(rename = "spawnStep")]
    spawn_step: Option<usize>,
}

fn random_ball_at(x: f32, y: f32) -> Ball {
    let mut rng = crate::behavior_random::rng();
    Ball {
        x,
        y,
        vx: rng.gen_range(-1.0..1.0),
        vy: rng.gen_range(-1.0..1.0),
    }
}

fn random_ball() -> Ball {
    let mut rng = crate::behavior_random::rng();
    random_ball_at(
        rng.gen_range(0..GRID_WIDTH) as f32,
        rng.gen_range(0..GRID_HEIGHT) as f32,
    )
}

pub fn bounce_init(config: Value) -> Result<BounceState, String> {
    let seed_default = config
        .as_object()
        .map(|object| !object.contains_key("balls") && !object.contains_key("cells"))
        .unwrap_or(true);
    let config: BounceConfig = serde_json::from_value(config).unwrap_or_default();
    Ok(BounceState {
        balls: if seed_default {
            vec![Ball {
                x: 2.0,
                y: 2.0,
                vx: 0.75,
                vy: 0.5,
            }]
        } else {
            vec![]
        },
        cells: vec![false; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        max_balls: config.max_balls.unwrap_or(60),
        spawn_interval: config.spawn_interval.unwrap_or(16),
        spawn_step: config.spawn_step.unwrap_or(7).min(63),
        tick_counter: 0,
    })
}

pub fn bounce_on_input(
    state: BounceState,
    input: DeviceInput,
    _context: &mut BehaviorContext,
) -> BounceState {
    let mut next = state.clone();
    if next.balls.len() >= next.max_balls {
        return state;
    }
    match input {
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "addBall" =>
        {
            next.balls.push(random_ball())
        }
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            next.balls.push(random_ball_at(x as f32, y as f32))
        }
        _ => return state,
    };
    next
}

pub fn bounce_on_tick(state: BounceState, _context: &mut BehaviorContext) -> BounceState {
    let tick_counter = state.tick_counter + 1;
    let mut balls = state.balls.clone();
    if state.spawn_interval > 0
        && balls.len() < state.max_balls
        && (tick_counter - 1) % state.spawn_interval == state.spawn_step % state.spawn_interval
    {
        balls.push(scheduled_ball(tick_counter / state.spawn_interval));
    }
    for ball in &mut balls {
        ball.x += ball.vx;
        ball.y += ball.vy;
        if ball.x < 0.0 {
            ball.x = -ball.x;
            ball.vx = -ball.vx;
        }
        if ball.x >= GRID_WIDTH as f32 {
            ball.x = 2.0 * (GRID_WIDTH - 1) as f32 - ball.x;
            ball.vx = -ball.vx;
        }
        if ball.y < 0.0 {
            ball.y = -ball.y;
            ball.vy = -ball.vy;
        }
        if ball.y >= GRID_HEIGHT as f32 {
            ball.y = 2.0 * (GRID_HEIGHT - 1) as f32 - ball.y;
            ball.vy = -ball.vy;
        }
    }
    let mut cells = vec![false; CELL_COUNT];
    for ball in &balls {
        let x = (ball.x.round() as isize).clamp(0, GRID_WIDTH as isize - 1) as usize;
        let y = (ball.y.round() as isize).clamp(0, GRID_HEIGHT as isize - 1) as usize;
        cells[grid_index(x, y)] = true;
    }
    let trigger_types = trigger_types_from_cells(&state.cells, &cells);
    BounceState {
        balls,
        cells,
        trigger_types,
        tick_counter,
        ..state
    }
}

fn scheduled_ball(step: usize) -> Ball {
    let starts = [
        Ball {
            x: 1.0,
            y: 1.0,
            vx: 0.5,
            vy: 0.75,
        },
        Ball {
            x: 6.0,
            y: 2.0,
            vx: -0.75,
            vy: 0.5,
        },
        Ball {
            x: 2.0,
            y: 6.0,
            vx: 0.6,
            vy: -0.5,
        },
        Ball {
            x: 5.0,
            y: 5.0,
            vx: -0.5,
            vy: -0.7,
        },
    ];
    starts[step % starts.len()].clone()
}

pub fn bounce_render_model(state: &BounceState) -> BehaviorRenderModel {
    BehaviorRenderModel {
        name: "bounce".into(),
        status_line: format!(
            "{} ball{}",
            state.balls.len(),
            if state.balls.len() == 1 { "" } else { "s" }
        ),
        cells: state.cells.clone(),
        palette: crate::BehaviorRenderPalette {
            active: crate::palette::WHITE,
            inactive: crate::palette::BLACK,
            stable: crate::palette::MAGENTA,
        },
        trigger_types: Some(state.trigger_types.clone()),
    }
}

pub fn bounce_config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("spawnInterval", "Spawn Interval", 0, 30, 1),
        number_item("spawnStep", "Spawn Step", 0, 63, 1),
        number_item("maxBalls", "Max Balls", 1, 100, 1),
        action_item("addBall", "Add Ball"),
    ]
}

#[cfg(test)]
mod tests;
