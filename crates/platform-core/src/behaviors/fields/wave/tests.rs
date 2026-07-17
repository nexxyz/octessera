use super::*;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

fn base() -> WaveState {
    wave_init(serde_json::json!({ "dampingPct": 0, "tensionPct": 50, "impulseStrength": 80 }))
        .unwrap()
}

#[test]
fn menu_palette_and_normalization_contract() {
    let menu = wave_config_menu();
    assert_eq!(menu[0].key, "dampingPct");
    assert_eq!(menu[0].max, Some(100));
    assert_eq!(menu[1].key, "tensionPct");
    assert_eq!(menu[2].key, "impulseStrength");
    assert_eq!(menu[2].min, Some(1));
    assert_eq!(menu[2].max, Some(127));
    assert_eq!(menu[3].key, "dropImpulse");

    let state = wave_deserialize(serde_json::json!({
        "displacement": [999, -999], "velocity": [999],
        "dampingPct": 999, "tensionPct": 999, "impulseStrength": 999,
        "triggerTypes": ["activate"]
    }))
    .unwrap();
    assert_eq!(state.displacement[0], CLAMP);
    assert_eq!(state.displacement[1], -CLAMP);
    assert_eq!(state.velocity[0], CLAMP);
    assert_eq!(state.damping_pct, 100);
    assert_eq!(state.tension_pct, 100);
    assert_eq!(state.impulse_strength, 127);
    assert_eq!(state.trigger_types[0], CellTriggerType::Stable);

    let model = wave_render_model(&state);
    assert_eq!(model.name, "wave");
    assert!(model.status_line.starts_with("energy:"));
    assert_eq!(model.palette.inactive, crate::palette::BLACK);
    assert_eq!(model.palette.active, [180, 240, 255]);
}

#[test]
fn grid_press_exact_world_cell_activates() {
    let mut context = context();
    let state = base();
    let state = wave_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    assert_eq!(state.displacement[grid_index(2, 3)], 80);
    assert_eq!(
        state.trigger_types[grid_index(2, 3)],
        CellTriggerType::Activate
    );
}

#[test]
fn impulse_propagates_and_edges_reflect() {
    let mut context = context();
    let mut state = base();
    state.displacement[grid_index(0, 0)] = 100;
    let ticked = wave_on_tick(state, &mut context);
    assert!(ticked.displacement[grid_index(1, 0)] > 0);
    assert!(ticked.displacement[grid_index(0, 1)] > 0);
    assert_eq!(ticked.displacement[grid_index(GRID_WIDTH - 1, 0)], 0);
}

#[test]
fn damping_quiets_and_threshold_deactivates() {
    let mut context = context();
    let mut state = wave_init(serde_json::json!({ "dampingPct": 100, "tensionPct": 0 })).unwrap();
    state.displacement[0] = THRESHOLD;
    state.velocity[0] = 0;
    let ticked = wave_on_tick(state, &mut context);
    assert!(ticked.displacement[0].abs() < THRESHOLD);
    assert_eq!(ticked.trigger_types[0], CellTriggerType::Deactivate);
}

#[test]
fn drop_impulse_forces_activate_and_serialization_is_stable() {
    let mut context = context();
    let state = base();
    let state = wave_on_input(
        state,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "dropImpulse".into(),
        }),
        &mut context,
    );
    assert!(state.trigger_types.contains(&CellTriggerType::Activate));

    let serialized = wave_serialize(&state).unwrap();
    assert!(serialized.get("triggerTypes").is_none());
    let restored = wave_deserialize(serialized.clone()).unwrap();
    assert_eq!(wave_serialize(&restored).unwrap(), serialized);
    assert!(!restored.trigger_types.contains(&CellTriggerType::Activate));
}
