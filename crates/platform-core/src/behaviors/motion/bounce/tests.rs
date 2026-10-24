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
                .filter(|trigger| **trigger == CellTriggerType::Activate)
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
        cells: vec![false; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        max_balls: 1,
        spawn_interval: 0,
        spawn_step: 0,
        tick_counter: 0,
    };
    let ticked = bounce_on_tick(state, &mut context);
    assert_eq!(ticked.balls[0].x, 2.0);
    assert_eq!(ticked.balls[0].y, 1.5);
    assert!(ticked.cells[grid_index(2, 2)]);

    let state = BounceState {
        balls: vec![Ball {
            x: 7.5,
            y: 1.0,
            vx: 1.0,
            vy: 0.0,
        }],
        cells: vec![false; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
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

#[test]
fn bounce_render_and_config_menu_match_contract() {
    let state = bounce_init(serde_json::json!({ "balls": [] })).unwrap();
    let model = bounce_render_model(&state);
    assert_eq!(model.name, "bounce");
    assert_eq!(model.status_line, "0 balls");
    assert_eq!(model.trigger_types.as_ref().unwrap().len(), CELL_COUNT);
    assert_eq!(
        bounce_config_menu()
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        vec!["spawnInterval", "spawnStep", "maxBalls", "addBall"]
    );
}
