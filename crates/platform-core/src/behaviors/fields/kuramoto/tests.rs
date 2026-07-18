use super::*;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

fn base() -> KuramotoState {
    kuramoto_init(serde_json::json!({
        "couplingPct": 0,
        "frequencySpread": 0,
        "jitterPct": 0,
        "phases": [0],
        "frequencies": [4]
    }))
    .unwrap()
}

fn assert_tail_activity(mut state: KuramotoState) {
    let mut context = context();
    let mut consecutive_empty = 0;
    let mut consecutive_full = 0;
    let mut consecutive_same = 0;
    let mut previous = Vec::new();
    for tick in 1..=300 {
        state = kuramoto_on_tick(state, &mut context);
        let cells = kuramoto_render_model(&state).cells;
        if tick <= 240 {
            previous = cells;
            continue;
        }
        consecutive_empty = if cells.iter().all(|cell| !*cell) {
            consecutive_empty + 1
        } else {
            0
        };
        consecutive_full = if cells.iter().all(|cell| *cell) {
            consecutive_full + 1
        } else {
            0
        };
        consecutive_same = if cells == previous {
            consecutive_same + 1
        } else {
            0
        };
        assert!(consecutive_empty <= 2, "empty run at tick {tick}");
        assert!(consecutive_full <= 2, "full run at tick {tick}");
        assert!(consecutive_same <= 2, "same-frame run at tick {tick}");
        previous = cells;
    }
}

#[test]
fn menu_palette_and_normalization_contract() {
    let menu = kuramoto_config_menu();
    assert_eq!(menu[0].key, "couplingPct");
    assert_eq!(menu[0].max, Some(100));
    assert_eq!(menu[1].key, "frequencySpread");
    assert_eq!(menu[1].max, Some(32));
    assert_eq!(menu[2].key, "jitterPct");
    assert_eq!(menu[2].max, Some(100));
    assert_eq!(menu[3].key, "desyncPulse");

    let state = kuramoto_deserialize(serde_json::json!({
        "phases": [999, 2],
        "frequencies": [999],
        "couplingPct": 999,
        "frequencySpread": 999,
        "jitterPct": 999,
        "jitterState": 0,
        "triggerTypes": ["activate"]
    }))
    .unwrap();
    assert_eq!(state.phases[0], 255);
    assert_eq!(state.frequencies[0], 255);
    assert_eq!(state.coupling_pct, 100);
    assert_eq!(state.frequency_spread, 32);
    assert_eq!(state.jitter_pct, 100);
    assert_eq!(state.jitter_state, 1);
    assert!(!state.trigger_types.contains(&CellTriggerType::Activate));

    let model = kuramoto_render_model(&state);
    assert_eq!(model.name, "kuramoto");
    assert!(model.status_line.starts_with("sync:"));
    assert_eq!(model.palette.inactive, crate::palette::BLACK);
    assert_eq!(model.palette.active, [255, 255, 200]);
}

#[test]
fn grid_press_exact_world_cell_activates() {
    let mut context = context();
    let state = base();
    let state = kuramoto_on_input(state, DeviceInput::GridPress { x: 2, y: 3 }, &mut context);
    assert_eq!(state.phases[grid_index(2, 3)], 255);
    assert_eq!(
        state.trigger_types[grid_index(2, 3)],
        CellTriggerType::Activate
    );
}

#[test]
fn wrap_activates_and_stable_window_does_not_repeat_activate() {
    let mut context = context();
    let mut state = base();
    state.phases[grid_index(1, 1)] = 254;
    state.frequencies[grid_index(1, 1)] = 4;
    let wrapped = kuramoto_on_tick(state, &mut context);
    assert_eq!(
        wrapped.trigger_types[grid_index(1, 1)],
        CellTriggerType::Activate
    );

    let stable = kuramoto_on_tick(wrapped, &mut context);
    assert_ne!(
        stable.trigger_types[grid_index(1, 1)],
        CellTriggerType::Activate
    );
}

#[test]
fn desync_changes_phase_without_immediate_activate() {
    let mut context = context();
    let state = base();
    let old = state.phases.clone();
    let state = kuramoto_on_input(
        state,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "desyncPulse".into(),
        }),
        &mut context,
    );
    assert_ne!(state.phases, old);
    assert!(!state.trigger_types.contains(&CellTriggerType::Activate));
}

#[test]
fn serialization_is_stable_and_skips_triggers() {
    let state = kuramoto_deserialize(serde_json::json!({
        "phases": [1, 2, 3],
        "frequencies": [4, 5, 6],
        "triggerTypes": ["activate"]
    }))
    .unwrap();
    let serialized = kuramoto_serialize(&state).unwrap();
    assert!(serialized.get("triggerTypes").is_none());
    let restored = kuramoto_deserialize(serialized.clone()).unwrap();
    assert_eq!(kuramoto_serialize(&restored).unwrap(), serialized);
    assert!(!restored.trigger_types.contains(&CellTriggerType::Activate));
}

#[test]
fn defaults_keep_tail_activity_bounded_through_300_ticks() {
    assert_tail_activity(kuramoto_init(serde_json::json!({})).unwrap());
}
