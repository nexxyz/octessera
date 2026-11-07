use super::wall::ROW_HUES;
use super::*;
use crate::behavior_random::LayerRandom;
use serde_json::json;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

fn trigger(state: &BlockoutState, x: usize, y: usize) -> TileTriggerType {
    state.trigger_types[grid_index(x, y)]
}

fn row(blocks: u8, hue: u8) -> BlockRow {
    BlockRow { blocks, hue }
}

fn with_ball(mut state: BlockoutState, ball: Ball) -> BlockoutState {
    state.ball = Some(ball);
    state
}

#[test]
fn default_wall_paddle_and_ball() {
    let state = init(Value::Null).unwrap();
    assert_eq!(state.rows, vec![row(255, 0), row(255, 1), row(255, 2)]);
    assert_eq!(state.paddle, 2);
    assert_eq!(state.row_spawn, "new");
    let ball = state.ball.unwrap();
    assert_eq!((ball.x, ball.y, ball.vy), (3, 1, 1));
    let model = render_model(&state);
    assert_eq!(model.tiles.iter().filter(|tile| **tile).count(), 28);
    let colors = model.tile_colors.unwrap();
    assert_eq!(colors[grid_index(0, 7)], ROW_HUES[0]);
    assert_eq!(colors[grid_index(0, 5)], ROW_HUES[2]);
    assert_eq!(colors[grid_index(2, 0)], WHITE);
    assert_eq!(colors[grid_index(3, 1)], YELLOW);
    assert_eq!(colors[grid_index(3, 3)], BACKGROUND);
}

#[test]
fn a_block_hit_breaks_it_with_a_one_step_flash_and_turns_the_ball() {
    let state = with_ball(
        init(Value::Null).unwrap(),
        Ball {
            x: 3,
            y: 4,
            vx: 0,
            vy: 1,
        },
    );
    let state = on_tick(state, &mut context());
    assert_eq!(state.rows[2].blocks, !(1 << 3));
    assert_eq!(trigger(&state, 3, 5), TileTriggerType::Activate);
    let model = render_model(&state);
    assert!(model.tiles[grid_index(3, 5)]);
    assert_eq!(model.tile_colors.unwrap()[grid_index(3, 5)], ROW_HUES[2]);
    assert_eq!(
        state.ball,
        Some(Ball {
            x: 3,
            y: 3,
            vx: 0,
            vy: -1
        })
    );
    assert_eq!(trigger(&state, 3, 3), TileTriggerType::Activate);
    let state = on_tick(state, &mut context());
    assert_eq!(state.flash, None);
    assert_eq!(trigger(&state, 3, 5), TileTriggerType::Deactivate);
    assert!(!render_model(&state).tiles[grid_index(3, 5)]);
}

#[test]
fn a_cleared_row_drops_a_new_row_on_top_and_pushes_the_rest_down_silently() {
    for (mode, spawned) in [
        ("new", TileTriggerType::Activate),
        ("stable", TileTriggerType::Stable),
    ] {
        let mut state = init(json!({ "rowSpawn": mode })).unwrap();
        state.rows = vec![row(255, 0), row(255, 1), row(1 << 3, 2)];
        let state = on_tick(
            with_ball(
                state,
                Ball {
                    x: 3,
                    y: 4,
                    vx: 0,
                    vy: 1,
                },
            ),
            &mut context(),
        );
        assert_eq!(state.rows, vec![row(255, 5), row(255, 0), row(255, 1)]);
        assert_eq!(state.rows_cleared, 1);
        assert_eq!(trigger(&state, 3, 5), TileTriggerType::Activate);
        assert_eq!(trigger(&state, 0, 5), TileTriggerType::Stable);
        assert_eq!(trigger(&state, 0, 6), TileTriggerType::Stable);
        assert_eq!(trigger(&state, 0, 7), spawned, "{mode}");
        let colors = render_model(&state).tile_colors.unwrap();
        assert_eq!(colors[grid_index(0, 7)], ROW_HUES[5]);
    }
}

#[test]
fn a_paddle_hit_bounces_up_and_refires_the_struck_tile() {
    let mut state = with_ball(
        init(Value::Null).unwrap(),
        Ball {
            x: 3,
            y: 1,
            vx: 1,
            vy: -1,
        },
    );
    state.paddle = 3;
    state.plan = CpuPlan {
        miss: false,
        aim: 1,
    };
    let state = on_tick(state, &mut context());
    assert_eq!(state.paddle, 3);
    assert_eq!(
        state.ball,
        Some(Ball {
            x: 3,
            y: 2,
            vx: 0,
            vy: 1
        })
    );
    assert_eq!(trigger(&state, 4, 0), TileTriggerType::Activate);
    assert_eq!(trigger(&state, 3, 0), TileTriggerType::Stable);
}

#[test]
fn a_missed_ball_serves_again_after_a_pause() {
    let mut state = with_ball(
        init(Value::Null).unwrap(),
        Ball {
            x: 0,
            y: 1,
            vx: -1,
            vy: -1,
        },
    );
    state.paddle = 5;
    state.plan = CpuPlan { miss: true, aim: 0 };
    state = on_tick(state, &mut context());
    assert_eq!(state.ball.map(|ball| (ball.x, ball.y)), Some((1, 0)));
    state = on_tick(state, &mut context());
    assert_eq!(state.ball, None);
    assert_eq!(trigger(&state, 1, 0), TileTriggerType::Deactivate);
    state = on_tick(state, &mut context());
    assert_eq!(state.ball, None);
    state = on_tick(state, &mut context());
    let ball = state.ball.unwrap();
    assert_eq!((ball.x, ball.y), (state.paddle + 1, 1));
}

#[test]
fn a_perfect_cpu_never_drops_the_ball() {
    for start_rows in 1..=MAX_ROWS {
        LayerRandom::new(Some(5)).scope(|| {
            let mut state = init(json!({ "cpuSkillPct": 100, "startRows": start_rows })).unwrap();
            for tick in 0..3000 {
                state = on_tick(state, &mut context());
                assert!(
                    state.ball.is_some_and(|ball| ball.y > 0),
                    "{start_rows} rows, tick {tick}"
                );
            }
            if start_rows == 3 {
                assert!(state.rows_cleared > 5, "{}", state.rows_cleared);
            }
        });
    }
}

#[test]
fn seeded_games_replay_exactly() {
    let run = || {
        LayerRandom::new(Some(11)).scope(|| {
            let mut state = init(Value::Null).unwrap();
            (0..500)
                .map(|_| {
                    state = on_tick(state.clone(), &mut context());
                    state.clone()
                })
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(run(), run());
}

#[test]
fn presses_add_blocks_in_the_wall_and_nudge_the_paddle_below_it() {
    let mut state = init(Value::Null).unwrap();
    state.rows[1].blocks = 0b1111_0111;
    let state = on_input(state, DeviceInput::GridPress { x: 3, y: 6 }, &mut context());
    assert_eq!(state.rows[1].blocks, 255);
    assert_eq!(trigger(&state, 3, 6), TileTriggerType::Activate);
    let state = on_input(state, DeviceInput::GridPress { x: 7, y: 2 }, &mut context());
    assert_eq!(state.paddle, 3);
    assert_eq!(trigger(&state, 5, 0), TileTriggerType::Activate);
    assert_eq!(trigger(&state, 2, 0), TileTriggerType::Deactivate);
}

#[test]
fn new_wall_refills_and_serves() {
    let mut state = init(json!({ "startRows": 2 })).unwrap();
    state.rows = vec![row(1, 4), row(0b10, 5)];
    let state = on_input(
        state,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "newWall".into(),
        }),
        &mut context(),
    );
    assert_eq!(state.rows, vec![row(255, 0), row(255, 1)]);
    assert_eq!(trigger(&state, 4, 7), TileTriggerType::Activate);
}

#[test]
fn settings_clamp_and_round_trip() {
    let state = init(json!({ "startRows": 9, "rowSpawn": "sideways" })).unwrap();
    assert_eq!((state.start_rows, state.row_spawn.as_str()), (3, "new"));
    assert_eq!(state.rows.len(), 3);
    let restored = deserialize(serialize(&state).unwrap()).unwrap();
    assert_eq!(restored, state);
}
