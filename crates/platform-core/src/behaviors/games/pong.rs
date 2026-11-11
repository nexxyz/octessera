use super::cpu_paddle::{
    hit_slope, landing, paddle_target, roll_plan, serve_slope, step_toward, wall_step, Ball,
    CpuPlan, SERVE_PAUSE_TICKS,
};
use crate::behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorContext, BehaviorRenderModel,
    BehaviorRenderPalette, DeviceInput, TileTriggerType,
};
use crate::behaviors::native_impl::config_items::{action_item, enum_item, number_item};
use crate::behaviors::native_impl::grid_transitions::{trigger_types_from_tiles, TILE_COUNT};
use crate::grid::{grid_index, GRID_HEIGHT, GRID_WIDTH};
use crate::palette::{BEHAVIOR_DIM_GREEN, BLACK, WHITE, YELLOW};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const RIGHT: u8 = GRID_WIDTH as u8 - 1;
const TOP: u8 = GRID_HEIGHT as u8 - 1;
const COLOR_SCHEMES: &[&str] = &["classic", "lawn", "sandcourt"];
const SAND_COURT: [u8; 3] = [120, 36, 4];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PongState {
    pub ball: Option<Ball>,
    /// The lowest row of the left (x=0) and right (x=7) paddle.
    pub paddles: [u8; 2],
    pub plan: CpuPlan,
    pub score: [u32; 2],
    #[serde(rename = "servePause")]
    pub serve_pause: u8,
    #[serde(rename = "serveDir")]
    pub serve_dir: i8,
    #[serde(rename = "cpuSkillPct")]
    pub cpu_skill_pct: u8,
    #[serde(rename = "paddleSize")]
    pub paddle_size: u8,
    #[serde(rename = "colorScheme")]
    pub color_scheme: String,
    #[serde(rename = "triggerTypes", skip_serializing, default)]
    pub trigger_types: Vec<TileTriggerType>,
}

#[derive(Default, Deserialize)]
struct Config {
    ball: Option<Ball>,
    paddles: Option<[u8; 2]>,
    plan: Option<CpuPlan>,
    score: Option<[u32; 2]>,
    #[serde(rename = "servePause")]
    serve_pause: Option<u8>,
    #[serde(rename = "serveDir")]
    serve_dir: Option<i8>,
    #[serde(rename = "cpuSkillPct")]
    cpu_skill_pct: Option<u64>,
    #[serde(rename = "paddleSize")]
    paddle_size: Option<u64>,
    #[serde(rename = "colorScheme")]
    color_scheme: Option<String>,
}

pub fn init(config: Value) -> Result<PongState, String> {
    Ok(from_value(config))
}

pub fn deserialize(data: Value) -> Result<PongState, String> {
    Ok(from_value(data))
}

pub fn serialize(state: &PongState) -> Result<Value, String> {
    serde_json::to_value(state).map_err(|error| error.to_string())
}

fn from_value(value: Value) -> PongState {
    let config: Config = serde_json::from_value(value).unwrap_or_default();
    let paddle_size = config.paddle_size.unwrap_or(3).clamp(2, 4) as u8;
    let center = center_start(paddle_size);
    let restored = config.paddles.is_some();
    let mut state = PongState {
        ball: config
            .ball
            .filter(|ball| ball.x <= RIGHT && ball.y <= TOP)
            .map(Ball::with_unit_velocity),
        paddles: config
            .paddles
            .unwrap_or([center; 2])
            .map(|start| start.min(GRID_HEIGHT as u8 - paddle_size)),
        plan: config.plan.unwrap_or_default(),
        score: config.score.unwrap_or_default(),
        serve_pause: config.serve_pause.unwrap_or(0).min(SERVE_PAUSE_TICKS),
        serve_dir: if config.serve_dir.unwrap_or(1) < 0 {
            -1
        } else {
            1
        },
        cpu_skill_pct: config.cpu_skill_pct.unwrap_or(80).min(100) as u8,
        paddle_size,
        color_scheme: color_scheme(config.color_scheme),
        trigger_types: Vec::new(),
    };
    if !restored {
        let dir = serve_slope(false);
        serve(&mut state, dir);
    }
    let tiles = tiles(&state);
    state.trigger_types = trigger_types_from_tiles(&tiles, &tiles);
    state
}

pub fn on_input(mut state: PongState, input: DeviceInput, _: &mut BehaviorContext) -> PongState {
    let previous = tiles(&state);
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let side = usize::from(x >= GRID_WIDTH / 2);
            let start = state.paddles[side];
            let y = y as u8;
            if y < start {
                state.paddles[side] = start - 1;
            } else if y >= start + state.paddle_size {
                state.paddles[side] = start + 1;
            } else {
                return state;
            }
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "serve" =>
        {
            let dir = serve_slope(false);
            serve(&mut state, dir);
        }
        _ => return state,
    }
    state.trigger_types = trigger_types_from_tiles(&previous, &tiles(&state));
    state
}

pub fn on_tick(mut state: PongState, _: &mut BehaviorContext) -> PongState {
    let previous = tiles(&state);
    let mut struck = None;
    match state.ball {
        Some(ball) if ball.x == 0 || ball.x == RIGHT => {
            state.ball = None;
            state.serve_pause = SERVE_PAUSE_TICKS;
            state.serve_dir = if ball.x == 0 { 1 } else { -1 };
            move_paddles(&mut state);
        }
        Some(ball) => {
            move_paddles(&mut state);
            struck = advance_ball(&mut state, ball);
        }
        None => {
            move_paddles(&mut state);
            state.serve_pause = state.serve_pause.saturating_sub(1);
            if state.serve_pause == 0 {
                let dir = state.serve_dir;
                serve(&mut state, dir);
            }
        }
    }
    let mut triggers = trigger_types_from_tiles(&previous, &tiles(&state));
    if let Some(index) = struck {
        triggers[index] = TileTriggerType::Activate;
    }
    state.trigger_types = triggers;
    state
}

fn serve(state: &mut PongState, dir: i8) {
    let x = if dir > 0 { 3 } else { 4 };
    let y = crate::behavior_random::rng().gen_range(2..=5);
    state.ball = Some(Ball {
        x,
        y,
        vx: dir,
        vy: serve_slope(false),
    });
    state.serve_pause = 0;
    state.plan = roll_plan(state.cpu_skill_pct, state.paddle_size);
}

fn move_paddles(state: &mut PongState) {
    let center = center_start(state.paddle_size);
    let mut targets = [center; 2];
    if let Some(ball) = state.ball.filter(|ball| ball.x > 0 && ball.x < RIGHT) {
        let side = usize::from(ball.vx > 0);
        let steps = if ball.vx > 0 { RIGHT - ball.x } else { ball.x };
        let row = landing(ball.y, ball.vy, steps, TOP);
        targets[side] = paddle_target(row, state.plan, state.paddle_size, GRID_HEIGHT as u8);
    }
    for (paddle, target) in state.paddles.iter_mut().zip(targets) {
        *paddle = step_toward(*paddle, target);
    }
}

/// Moves the ball one step and returns the struck paddle tile on a hit.
fn advance_ball(state: &mut PongState, ball: Ball) -> Option<usize> {
    let (y, vy) = wall_step(ball.y, ball.vy, TOP);
    let x = (i16::from(ball.x) + i16::from(ball.vx)) as u8;
    if x != 0 && x != RIGHT {
        state.ball = Some(Ball { x, y, vy, ..ball });
        return None;
    }
    let side = usize::from(x == RIGHT);
    let start = state.paddles[side];
    if y < start || y >= start + state.paddle_size {
        state.ball = Some(Ball { x, y, vy, ..ball });
        state.score[1 - side] = state.score[1 - side].saturating_add(1);
        return None;
    }
    let vx = -ball.vx;
    let (y_after, vy) = wall_step(ball.y, hit_slope(y - start, state.paddle_size), TOP);
    state.ball = Some(Ball {
        x: (i16::from(ball.x) + i16::from(vx)) as u8,
        y: y_after,
        vx,
        vy,
    });
    state.plan = roll_plan(state.cpu_skill_pct, state.paddle_size);
    Some(grid_index(usize::from(x), usize::from(y)))
}

fn tiles(state: &PongState) -> Vec<bool> {
    let mut tiles = vec![false; TILE_COUNT];
    for (side, start) in state.paddles.iter().enumerate() {
        let x = if side == 0 { 0 } else { GRID_WIDTH - 1 };
        for y in *start..start + state.paddle_size {
            tiles[grid_index(x, usize::from(y))] = true;
        }
    }
    if let Some(ball) = state.ball {
        tiles[grid_index(usize::from(ball.x), usize::from(ball.y))] = true;
    }
    tiles
}

fn center_start(paddle_size: u8) -> u8 {
    (GRID_HEIGHT as u8 - paddle_size) / 2
}

fn color_scheme(value: Option<String>) -> String {
    value
        .filter(|value| COLOR_SCHEMES.contains(&value.as_str()))
        .unwrap_or_else(|| "classic".into())
}

pub fn render_model(state: &PongState) -> BehaviorRenderModel {
    let palette = match state.color_scheme.as_str() {
        "lawn" => (YELLOW, BEHAVIOR_DIM_GREEN),
        "sandcourt" => (YELLOW, SAND_COURT),
        _ => (WHITE, BLACK),
    };
    let tiles = tiles(state);
    let mut colors = vec![palette.1; TILE_COUNT];
    for (color, lit) in colors.iter_mut().zip(&tiles) {
        if *lit {
            *color = WHITE;
        }
    }
    if let Some(ball) = state.ball {
        colors[grid_index(usize::from(ball.x), usize::from(ball.y))] = palette.0;
    }
    BehaviorRenderModel {
        name: "pong".into(),
        status_line: format!("{} : {}", state.score[0], state.score[1]),
        tiles,
        palette: BehaviorRenderPalette {
            active: palette.0,
            inactive: palette.1,
            stable: WHITE,
        },
        trigger_types: Some(state.trigger_types.clone()),
        tile_colors: Some(colors),
    }
}

pub fn config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("cpuSkillPct", "CPU Skill", 0, 100, 1),
        number_item("paddleSize", "Paddle Size", 2, 4, 1),
        enum_item("colorScheme", "Color", COLOR_SCHEMES),
        action_item("serve", "Serve"),
    ]
}

#[cfg(test)]
mod tests;
