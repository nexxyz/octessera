use platform_core::{
    default_mapping_config, get_native_behavior, list_native_behavior_ids, AxisStrategy,
    BehaviorActionInput, BehaviorConfigItemType, BehaviorContext, DeviceInput, GlobalSoundConfig,
    InterpretationEventProfile, InterpretationProfile, InterpretationStateProfile, NativeBehavior,
    NativeLayerEngine, NativeLayerEngineConfig, NoteBehavior, TickStrategy, VelocityCurve,
    GRID_HEIGHT, GRID_WIDTH,
};
use serde_json::Value;

#[test]
fn behavior_actions_and_grid_edges_route_through_public_runtime_contract() {
    let cell_count = GRID_WIDTH * GRID_HEIGHT;
    for id in list_native_behavior_ids() {
        let behavior = get_native_behavior(id).unwrap();
        let base_state = behavior.init(Value::Null).unwrap();
        let menu = behavior
            .config_menu(&base_state)
            .unwrap()
            .unwrap_or_default();

        for item in menu
            .iter()
            .filter(|item| item.item_type == BehaviorConfigItemType::Action)
        {
            let mut context = BehaviorContext::new(120.0);
            let state = behavior
                .on_input(
                    base_state.clone(),
                    DeviceInput::BehaviorAction(BehaviorActionInput {
                        action_type: item.key.clone(),
                    }),
                    &mut context,
                )
                .unwrap();
            let ticked = behavior.on_tick(state, &mut context).unwrap();
            assert_eq!(
                behavior.render_model(&ticked).unwrap().cells.len(),
                cell_count
            );
        }

        for (x, y) in [
            (0, 0),
            (GRID_WIDTH - 1, 0),
            (0, GRID_HEIGHT - 1),
            (GRID_WIDTH - 1, GRID_HEIGHT - 1),
            (GRID_WIDTH, 0),
            (0, GRID_HEIGHT),
            (GRID_WIDTH, GRID_HEIGHT),
        ] {
            let mut context = BehaviorContext::new(120.0);
            let state = behavior
                .on_input(
                    base_state.clone(),
                    DeviceInput::GridPress { x, y },
                    &mut context,
                )
                .unwrap();
            let model = behavior.render_model(&state).unwrap();
            assert_eq!(model.cells.len(), cell_count);
        }
    }
}

#[test]
fn native_layer_engine_mutators_and_scan_modes_are_publicly_stable() {
    let mut engine = NativeLayerEngine::new(NativeLayerEngineConfig {
        behavior: NativeBehavior::Life,
        behavior_config: Value::Null,
        interpretation_profile: InterpretationProfile {
            id: "sweep".into(),
            event: InterpretationEventProfile { enabled: true },
            state: InterpretationStateProfile {
                enabled: true,
                tick: TickStrategy::WholeGridActive,
            },
            x: AxisStrategy::ScaleStep { step: 1 },
            y: AxisStrategy::ScaleStep { step: 2 },
        },
        mapping_config: default_mapping_config(),
        global_sound: GlobalSoundConfig {
            velocity_scale_pct: 100,
            velocity_curve: VelocityCurve::Linear,
            note_length_ms: 42,
        },
        note_behaviors: vec![NoteBehavior::Oneshot; 16],
        layer_index: 2,
    })
    .unwrap();

    engine.set_mapping_config(default_mapping_config());
    engine.set_interpretation_profile(platform_core::InterpretationProfile {
        id: "sweep-active".into(),
        event: InterpretationEventProfile { enabled: true },
        state: platform_core::InterpretationStateProfile {
            enabled: true,
            tick: TickStrategy::WholeGridActive,
        },
        x: AxisStrategy::ScaleStep { step: 1 },
        y: AxisStrategy::ScaleStep { step: 2 },
    });
    engine.set_global_sound(GlobalSoundConfig {
        velocity_scale_pct: 100,
        velocity_curve: VelocityCurve::Linear,
        note_length_ms: 42,
    });
    engine.set_note_behaviors(vec![NoteBehavior::Hold; 16]);

    let _ = engine
        .on_input(DeviceInput::GridPress { x: 1, y: 1 }, 120.0)
        .unwrap();
    let first = engine.tick(120.0).unwrap();
    assert!(!first.events.is_empty());
    assert_eq!(
        engine.model().unwrap().cells.len(),
        GRID_WIDTH * GRID_HEIGHT
    );
    assert!(matches!(
        engine.state(),
        platform_core::NativeBehaviorState::Life(_)
    ));

    engine.reset_transport_phase();
    let second = engine.tick(120.0).unwrap();
    assert_eq!(second.model.cells.len(), GRID_WIDTH * GRID_HEIGHT);
}
