use super::*;

#[test]
fn empty_config_seeds_deterministic_ball_and_autospawns_are_bounded() {
    let mut context = BehaviorContext::new(120.0);
    let mut a = bounce_init(serde_json::json!({})).unwrap();
    let mut b = bounce_init(serde_json::json!({})).unwrap();
    assert_eq!(a.balls, b.balls);
    assert_eq!(a.balls.len(), 1);
    let mut max_activates = 0;
    for _ in 0..40 {
        a = bounce_on_tick(a, &mut context);
        b = bounce_on_tick(b, &mut context);
        assert_eq!(a.balls, b.balls);
        max_activates = max_activates.max(
            a.trigger_types
                .iter()
                .filter(|trigger| **trigger == TileTriggerType::Activate)
                .count(),
        );
    }
    assert!(max_activates <= 8);
}

#[test]
fn ball_moves_bounces_and_max_balls_limits_spawns() {
    let mut context = BehaviorContext::new(120.0);
    let state = BounceState {
        balls: vec![Ball {
            x: 1.0,
            y: 1.0,
            vx: 1.0,
            vy: 0.5,
        }],
        tiles: vec![false; TILE_COUNT],
        trigger_types: vec![TileTriggerType::None; TILE_COUNT],
        max_balls: 1,
        spawn_interval: 0,
        spawn_step: 0,
        tick_counter: 0,
    };
    let ticked = bounce_on_tick(state, &mut context);
    assert_eq!(ticked.balls[0].x, 2.0);
    assert_eq!(ticked.balls[0].y, 1.5);
    assert!(ticked.tiles[grid_index(2, 2)]);

    let state = BounceState {
        balls: vec![Ball {
            x: 7.5,
            y: 1.0,
            vx: 1.0,
            vy: 0.0,
        }],
        tiles: vec![false; TILE_COUNT],
        trigger_types: vec![TileTriggerType::None; TILE_COUNT],
        max_balls: 1,
        spawn_interval: 0,
        spawn_step: 0,
        tick_counter: 0,
    };
    let ticked = bounce_on_tick(state, &mut context);
    assert!(ticked.balls[0].vx < 0.0);
    assert!(ticked.balls[0].x <= (GRID_WIDTH - 1) as f32);

    let limited = bounce_on_input(
        ticked.clone(),
        DeviceInput::GridPress { x: 2, y: 2 },
        &mut context,
    );
    assert_eq!(limited.balls.len(), ticked.balls.len());
}

fn one_ball(x: f32, y: f32, vx: f32, vy: f32) -> BounceState {
    BounceState {
        balls: vec![Ball { x, y, vx, vy }],
        tiles: vec![false; TILE_COUNT],
        trigger_types: vec![TileTriggerType::None; TILE_COUNT],
        max_balls: 1,
        spawn_interval: 0,
        spawn_step: 0,
        tick_counter: 0,
    }
}

#[test]
fn both_walls_of_each_axis_reflect_alike() {
    let mut context = BehaviorContext::new(120.0);
    let last_x = (GRID_WIDTH - 1) as f32;
    let last_y = (GRID_HEIGHT - 1) as f32;
    let mut low = one_ball(0.125, 0.375, -0.75, -0.5);
    let mut high = one_ball(last_x - 0.125, last_y - 0.375, 0.75, 0.5);
    for step in 0..24 {
        low = bounce_on_tick(low, &mut context);
        high = bounce_on_tick(high, &mut context);
        let (low_ball, high_ball) = (&low.balls[0], &high.balls[0]);
        assert_eq!(
            (high_ball.x, high_ball.y, high_ball.vx, high_ball.vy),
            (
                last_x - low_ball.x,
                last_y - low_ball.y,
                -low_ball.vx,
                -low_ball.vy
            ),
            "step {step}"
        );
        let tile = |state: &BounceState| state.tiles.iter().position(|tile| *tile).unwrap();
        assert_eq!(tile(&high), TILE_COUNT - 1 - tile(&low), "step {step}");
    }
}

#[test]
fn bounce_render_and_config_menu_match_contract() {
    let state = bounce_init(serde_json::json!({ "balls": [] })).unwrap();
    let model = bounce_render_model(&state);
    assert_eq!(model.name, "bounce");
    assert_eq!(model.status_line, "0 balls");
    assert_eq!(model.trigger_types.as_ref().unwrap().len(), TILE_COUNT);
    assert_eq!(
        bounce_config_menu()
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        vec!["spawnInterval", "spawnStep", "maxBalls", "addBall"]
    );
}
