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
use crate::palette::{WHITE, YELLOW};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wall::{
    add_block, block_color, break_block, fresh_wall, has_block, normalize_hues, top_row_tiles,
    wall_blocks, BlockRow,
};

mod wall;

const RIGHT: u8 = GRID_WIDTH as u8 - 1;
const TOP: u8 = GRID_HEIGHT as u8 - 1;
const PADDLE: u8 = 3;
const MAX_ROWS: u8 = 3;
const ROW_SPAWNS: &[&str] = &["stable", "new"];
const BACKGROUND: [u8; 3] = [0, 0, 24];
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockoutState {
    /// The wall, top row (y=7) first.
    pub rows: Vec<BlockRow>,
    /// The leftmost column of the bottom-row paddle.
    pub paddle: u8,
    pub ball: Option<Ball>,
    pub plan: CpuPlan,
    #[serde(rename = "servePause")]
    pub serve_pause: u8,
    #[serde(rename = "rowsCleared")]
    pub rows_cleared: u32,
    #[serde(rename = "cpuSkillPct")]
    pub cpu_skill_pct: u8,
    #[serde(rename = "startRows")]
    pub start_rows: u8,
    #[serde(rename = "rowSpawn")]
    pub row_spawn: String,
    /// The block broken this step and its row color: it stays lit for the step, then clears.
    #[serde(skip)]
    pub flash: Option<(usize, [u8; 3])>,
    #[serde(rename = "triggerTypes", skip_serializing, default)]
    pub trigger_types: Vec<TileTriggerType>,
}

#[derive(Default, Deserialize)]
struct Config {
    rows: Option<Vec<BlockRow>>,
    paddle: Option<u8>,
    ball: Option<Ball>,
    plan: Option<CpuPlan>,
    #[serde(rename = "servePause")]
    serve_pause: Option<u8>,
    #[serde(rename = "rowsCleared")]
    rows_cleared: Option<u32>,
    #[serde(rename = "cpuSkillPct")]
    cpu_skill_pct: Option<u64>,
    #[serde(rename = "startRows")]
    start_rows: Option<u64>,
    #[serde(rename = "rowSpawn")]
    row_spawn: Option<String>,
}

/// Tiles one step lit up or let go on purpose, beyond the ball and paddle moving.
#[derive(Default)]
struct Accents {
    activated: Vec<usize>,
    cleared_flash: Option<usize>,
}

pub fn init(config: Value) -> Result<BlockoutState, String> {
    Ok(from_value(config))
}

pub fn deserialize(data: Value) -> Result<BlockoutState, String> {
    Ok(from_value(data))
}

pub fn serialize(state: &BlockoutState) -> Result<Value, String> {
    serde_json::to_value(state).map_err(|error| error.to_string())
}

fn from_value(value: Value) -> BlockoutState {
    let config: Config = serde_json::from_value(value).unwrap_or_default();
    let start_rows = config.start_rows.unwrap_or(3).clamp(1, u64::from(MAX_ROWS)) as u8;
    let restored = config.paddle.is_some();
    let rows = config
        .rows
        .filter(|rows| rows.len() == usize::from(start_rows))
        .map(normalize_hues)
        .unwrap_or_else(|| fresh_wall(start_rows));
    let mut state = BlockoutState {
        rows,
        paddle: config.paddle.unwrap_or(2).min(GRID_WIDTH as u8 - PADDLE),
        ball: config
            .ball
            .filter(|ball| ball.x <= RIGHT && ball.y <= TOP)
            .map(Ball::with_unit_velocity),
        plan: config.plan.unwrap_or_default(),
        serve_pause: config.serve_pause.unwrap_or(0).min(SERVE_PAUSE_TICKS),
        rows_cleared: config.rows_cleared.unwrap_or(0),
        cpu_skill_pct: config.cpu_skill_pct.unwrap_or(80).min(100) as u8,
        start_rows,
        row_spawn: config
            .row_spawn
            .filter(|mode| ROW_SPAWNS.contains(&mode.as_str()))
            .unwrap_or_else(|| "new".into()),
        flash: None,
        trigger_types: Vec::new(),
    };
    if !restored {
        serve(&mut state);
    }
    let tiles = tiles(&state);
    state.trigger_types = trigger_types_from_tiles(&tiles, &tiles);
    state
}

pub fn on_input(
    mut state: BlockoutState,
    input: DeviceInput,
    _: &mut BehaviorContext,
) -> BlockoutState {
    let (ball_before, paddle_before) = (ball_index(&state), state.paddle);
    let mut accents = Accents::default();
    match input {
        DeviceInput::GridPress { x, y } if x < GRID_WIDTH && y < GRID_HEIGHT => {
            let row = usize::from(TOP - y as u8);
            let index = grid_index(x, y);
            if row < state.rows.len() {
                if has_block(&state.rows, x as u8, y as u8) || ball_before == Some(index) {
                    return state;
                }
                add_block(&mut state.rows, x as u8, y as u8);
                accents.activated.push(index);
            } else {
                let target = (x as u8).saturating_sub(1).min(GRID_WIDTH as u8 - PADDLE);
                if target == state.paddle {
                    return state;
                }
                state.paddle = step_toward(state.paddle, target);
            }
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "serve" =>
        {
            serve(&mut state)
        }
        DeviceInput::BehaviorAction(BehaviorActionInput { action_type })
            if action_type == "newWall" =>
        {
            state.rows = fresh_wall(state.start_rows);
            if state.row_spawn == "new" {
                accents
                    .activated
                    .extend(wall_blocks(&state.rows).map(|(index, _)| index));
            }
            serve(&mut state);
        }
        _ => return state,
    }
    state.trigger_types = triggers(ball_before, paddle_before, &state, &accents);
    state
}

pub fn on_tick(mut state: BlockoutState, _: &mut BehaviorContext) -> BlockoutState {
    let (ball_before, paddle_before) = (ball_index(&state), state.paddle);
    let mut accents = Accents {
        cleared_flash: state.flash.take().map(|(index, _)| index),
        ..Accents::default()
    };
    match state.ball {
        Some(ball) if ball.y == 0 => {
            state.ball = None;
            state.serve_pause = SERVE_PAUSE_TICKS;
        }
        Some(ball) => {
            move_paddle(&mut state);
            advance_ball(&mut state, ball, &mut accents);
        }
        None => {
            state.serve_pause = state.serve_pause.saturating_sub(1);
            if state.serve_pause == 0 {
                serve(&mut state);
            }
        }
    }
    state.trigger_types = triggers(ball_before, paddle_before, &state, &accents);
    state
}

fn serve(state: &mut BlockoutState) {
    state.ball = Some(Ball {
        x: state.paddle + 1,
        y: 1,
        vx: serve_slope(true),
        vy: 1,
    });
    state.serve_pause = 0;
}

fn move_paddle(state: &mut BlockoutState) {
    let Some(ball) = state.ball else {
        return;
    };
    let highest = GRID_WIDTH as u8 - PADDLE;
    let target = if ball.vy < 0 {
        let column = landing(ball.x, ball.vx, ball.y, RIGHT);
        paddle_target(column, state.plan, PADDLE, GRID_WIDTH as u8)
    } else {
        ball.x.saturating_sub(1).min(highest)
    };
    state.paddle = step_toward(state.paddle, target);
}

fn advance_ball(state: &mut BlockoutState, ball: Ball, accents: &mut Accents) {
    let (x, vx) = wall_step(ball.x, ball.vx, RIGHT);
    let (y, vy) = wall_step(ball.y, ball.vy, TOP);
    let next = if y == 0 {
        if x < state.paddle || x >= state.paddle + PADDLE {
            Ball { x, y, vx, vy }
        } else {
            accents.activated.push(grid_index(usize::from(x), 0));
            let (x_after, vx) = wall_step(ball.x, hit_slope(x - state.paddle, PADDLE), RIGHT);
            Ball {
                x: x_after,
                y: ball.y + 1,
                vx,
                vy: 1,
            }
        }
    } else if has_block(&state.rows, x, y) {
        let index = grid_index(usize::from(x), usize::from(y));
        state.flash = Some((index, block_color(&state.rows, y)));
        accents.activated.push(index);
        if break_block(&mut state.rows, x, y) {
            state.rows_cleared = state.rows_cleared.saturating_add(1);
            if state.row_spawn == "new" {
                accents.activated.extend(top_row_tiles());
            }
        }
        let (y_after, vy) = wall_step(ball.y, -vy, TOP);
        if y_after == y || y_after == 0 || has_block(&state.rows, x, y_after) {
            Ball { vx, vy, ..ball }
        } else {
            Ball {
                x,
                y: y_after,
                vx,
                vy,
            }
        }
    } else {
        Ball { x, y, vx, vy }
    };
    if ball.vy > 0 && next.vy < 0 {
        state.plan = roll_plan(state.cpu_skill_pct, PADDLE);
    }
    state.ball = Some(next);
}

fn ball_index(state: &BlockoutState) -> Option<usize> {
    state
        .ball
        .map(|ball| grid_index(usize::from(ball.x), usize::from(ball.y)))
}

fn paddle_tiles(start: u8) -> impl Iterator<Item = usize> {
    (start..start + PADDLE).map(|x| grid_index(usize::from(x), 0))
}

fn tiles(state: &BlockoutState) -> Vec<bool> {
    let mut tiles = vec![false; TILE_COUNT];
    for index in wall_blocks(&state.rows)
        .map(|(index, _)| index)
        .chain(paddle_tiles(state.paddle))
        .chain(ball_index(state))
        .chain(state.flash.map(|(index, _)| index))
    {
        tiles[index] = true;
    }
    tiles
}

/// Blocks pushed down by a new row move silently; only the ball, the paddle edges, breaks and
/// new blocks make events. A broken block activates for its last step and deactivates as it
/// clears.
fn triggers(
    ball_before: Option<usize>,
    paddle_before: u8,
    state: &BlockoutState,
    accents: &Accents,
) -> Vec<TileTriggerType> {
    let tiles = tiles(state);
    let mut triggers = trigger_types_from_tiles(&tiles, &tiles);
    for index in ball_before
        .into_iter()
        .chain(paddle_tiles(paddle_before))
        .chain(accents.cleared_flash)
    {
        if !tiles[index] {
            triggers[index] = TileTriggerType::Deactivate;
        }
    }
    let new_paddle = paddle_tiles(state.paddle)
        .filter(|index| !paddle_tiles(paddle_before).any(|before| before == *index));
    let moved_ball = ball_index(state).filter(|index| Some(*index) != ball_before);
    for index in new_paddle
        .chain(moved_ball)
        .chain(accents.activated.iter().copied())
    {
        triggers[index] = TileTriggerType::Activate;
    }
    triggers
}

pub fn render_model(state: &BlockoutState) -> BehaviorRenderModel {
    let mut colors = vec![BACKGROUND; TILE_COUNT];
    for (index, color) in state.flash.into_iter().chain(wall_blocks(&state.rows)) {
        colors[index] = color;
    }
    for index in paddle_tiles(state.paddle) {
        colors[index] = WHITE;
    }
    if let Some(index) = ball_index(state) {
        colors[index] = YELLOW;
    }
    let blocks: u32 = state.rows.iter().map(|row| row.blocks.count_ones()).sum();
    BehaviorRenderModel {
        name: "blockout".into(),
        status_line: format!("B:{blocks} R:{}", state.rows_cleared),
        tiles: tiles(state),
        palette: BehaviorRenderPalette {
            active: YELLOW,
            inactive: BACKGROUND,
            stable: WHITE,
        },
        trigger_types: Some(state.trigger_types.clone()),
        tile_colors: Some(colors),
    }
}

pub fn config_menu() -> Vec<BehaviorConfigItem> {
    vec![
        number_item("cpuSkillPct", "CPU Skill", 0, 100, 1),
        number_item("startRows", "Start Rows", 1, i32::from(MAX_ROWS), 1),
        enum_item("rowSpawn", "Row Spawn", ROW_SPAWNS),
        action_item("serve", "Serve"),
        action_item("newWall", "New Wall"),
    ]
}

#[cfg(test)]
mod tests;
