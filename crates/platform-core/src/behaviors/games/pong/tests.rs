use super::*;
use crate::behavior_random::LayerRandom;
use serde_json::json;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

fn rally(ball: Ball, paddles: [u8; 2], plan: CpuPlan) -> PongState {
    let mut state = init(json!({})).unwrap();
    state.ball = Some(ball);
    state.paddles = paddles;
    state.plan = plan;
    state
}

fn trigger(state: &PongState, x: usize, y: usize) -> TileTriggerType {
    state.trigger_types[grid_index(x, y)]
}

#[test]
fn default_court_has_centered_paddles_and_a_served_ball() {
    let state = init(Value::Null).unwrap();
    assert_eq!(state.paddles, [2, 2]);
    assert_eq!(state.paddle_size, 3);
    assert_eq!(state.cpu_skill_pct, 80);
    let ball = state.ball.unwrap();
    assert!(matches!((ball.x, ball.vx), (3, 1) | (4, -1)));
    assert!((2..=5).contains(&ball.y));
    let model = render_model(&state);
    assert_eq!(model.tiles.iter().filter(|tile| **tile).count(), 7);
    assert_eq!(model.palette.inactive, BLACK);
    assert_eq!(model.palette.active, WHITE);
    assert_eq!(model.palette.stable, WHITE);
}

#[test]
fn color_schemes_change_the_court_and_ball() {
    let lawn = render_model(&init(json!({ "colorScheme": "lawn" })).unwrap()).palette;
    assert_eq!(
        (lawn.active, lawn.stable, lawn.inactive),
        (YELLOW, WHITE, BEHAVIOR_DIM_GREEN)
    );
    let sand = render_model(&init(json!({ "colorScheme": "sandcourt" })).unwrap()).palette;
    assert_eq!(
        (sand.active, sand.stable, sand.inactive),
        (YELLOW, WHITE, SAND_COURT)
    );
    let state = init(json!({ "colorScheme": "lawn" })).unwrap();
    let colors = render_model(&state).tile_colors.unwrap();
    let ball = state.ball.unwrap();
    assert_eq!(
        colors[grid_index(usize::from(ball.x), usize::from(ball.y))],
        YELLOW
    );
    assert_eq!(colors[grid_index(0, 2)], WHITE);
    assert_eq!(colors[grid_index(1, 0)], BEHAVIOR_DIM_GREEN);
    let unknown = init(json!({ "colorScheme": "plaid" })).unwrap();
    assert_eq!(unknown.color_scheme, "classic");
}

#[test]
fn a_middle_hit_sends_the_ball_back_flat_and_refires_the_struck_tile() {
    let ball = Ball {
        x: 1,
        y: 3,
        vx: -1,
        vy: 1,
    };
    let plan = CpuPlan {
        miss: false,
        aim: 1,
    };
    let state = on_tick(rally(ball, [3, 2], plan), &mut context());
    assert_eq!(
        state.ball,
        Some(Ball {
            x: 2,
            y: 3,
            vx: 1,
            vy: 0
        })
    );
    assert_eq!(trigger(&state, 0, 4), TileTriggerType::Activate);
    assert_eq!(trigger(&state, 0, 3), TileTriggerType::Stable);
    assert_eq!(trigger(&state, 2, 3), TileTriggerType::Activate);
    assert_eq!(trigger(&state, 1, 3), TileTriggerType::Deactivate);
}

#[test]
fn an_edge_hit_sends_the_ball_toward_that_edge() {
    let ball = Ball {
        x: 6,
        y: 5,
        vx: 1,
        vy: 0,
    };
    let plan = CpuPlan {
        miss: false,
        aim: 2,
    };
    let state = on_tick(rally(ball, [2, 3], plan), &mut context());
    assert_eq!(
        state.ball,
        Some(Ball {
            x: 5,
            y: 6,
            vx: -1,
            vy: 1
        })
    );
    assert_eq!(trigger(&state, 7, 5), TileTriggerType::Activate);
}

#[test]
fn a_miss_scores_pauses_and_serves_toward_the_scorer() {
    let ball = Ball {
        x: 6,
        y: 3,
        vx: 1,
        vy: 0,
    };
    let plan = CpuPlan { miss: true, aim: 0 };
    let mut state = on_tick(rally(ball, [2, 5], plan), &mut context());
    assert_eq!(state.ball.map(|ball| (ball.x, ball.y)), Some((7, 3)));
    assert_eq!(state.score, [1, 0]);
    state = on_tick(state, &mut context());
    assert_eq!(state.ball, None);
    state = on_tick(state, &mut context());
    assert_eq!(state.ball, None);
    state = on_tick(state, &mut context());
    let served = state.ball.unwrap();
    assert_eq!((served.x, served.vx), (4, -1));
    assert_eq!(
        trigger(&state, 4, usize::from(served.y)),
        TileTriggerType::Activate
    );
}

#[test]
fn a_perfect_cpu_never_misses() {
    LayerRandom::new(Some(7)).scope(|| {
        let mut state = init(json!({ "cpuSkillPct": 100 })).unwrap();
        for _ in 0..3000 {
            state = on_tick(state, &mut context());
        }
        assert_eq!(state.score, [0, 0]);
    });
}

#[test]
fn rallies_end_at_the_default_skill() {
    LayerRandom::new(Some(3)).scope(|| {
        let mut state = init(Value::Null).unwrap();
        for _ in 0..3000 {
            state = on_tick(state, &mut context());
        }
        assert!(state.score[0] + state.score[1] > 20, "{:?}", state.score);
    });
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
fn a_press_nudges_the_nearer_paddle_toward_it() {
    let state = init(Value::Null).unwrap();
    let state = on_input(state, DeviceInput::GridPress { x: 1, y: 7 }, &mut context());
    assert_eq!(state.paddles, [3, 2]);
    assert_eq!(trigger(&state, 0, 5), TileTriggerType::Activate);
    assert_eq!(trigger(&state, 0, 2), TileTriggerType::Deactivate);
    let state = on_input(state, DeviceInput::GridPress { x: 6, y: 0 }, &mut context());
    assert_eq!(state.paddles, [3, 1]);
    let state = on_input(state, DeviceInput::GridPress { x: 6, y: 2 }, &mut context());
    assert_eq!(state.paddles, [3, 1]);
}

#[test]
fn settings_clamp_and_round_trip() {
    let state = init(json!({ "paddleSize": 9, "cpuSkillPct": 400 })).unwrap();
    assert_eq!((state.paddle_size, state.cpu_skill_pct), (4, 100));
    assert_eq!(state.paddles, [2, 2]);
    let restored = deserialize(serialize(&state).unwrap()).unwrap();
    assert_eq!(restored, state);
}
