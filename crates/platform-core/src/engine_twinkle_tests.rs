use super::*;

fn twinkle_engine() -> NativeLayerEngine {
    let mut cells = vec![false; GRID_WIDTH * GRID_HEIGHT];
    cells[0] = true;
    NativeLayerEngine::new(NativeLayerEngineConfig {
        behavior: NativeBehavior::Twinkle,
        behavior_config: serde_json::json!({
            "cells": cells,
            "ages": [0],
            "density": 1,
            "birthChancePct": 100,
            "fadeChancePct": 100,
            "starLife": 1,
            "clusterBiasPct": 0,
            "seed": 19
        }),
        interpretation_profile: InterpretationProfile {
            id: "twinkle_events".into(),
            event: InterpretationEventProfile { enabled: true },
            state: InterpretationStateProfile {
                enabled: false,
                tick: TickStrategy::WholeGridTransitions,
            },
            x: AxisStrategy::ScaleStep { step: 1 },
            y: AxisStrategy::ScaleStep { step: 2 },
        },
        mapping_config: default_mapping_config(),
        global_sound: GlobalSoundConfig {
            velocity_scale_pct: 100,
            velocity_curve: VelocityCurve::Linear,
            note_length_ms: 120,
        },
        note_behaviors: vec![NoteBehavior::Oneshot; 16],
        layer_index: 0,
    })
    .unwrap()
}

#[test]
fn auto_death_birth_then_cap_press_has_only_fresh_events() {
    let mut engine = twinkle_engine();
    let tick = engine.tick(120.0).unwrap();
    let tick_intents = tick
        .event_intents
        .iter()
        .filter_map(Option::as_ref)
        .collect::<Vec<_>>();
    assert_eq!(tick_intents.len(), 2);
    assert!(tick_intents
        .iter()
        .any(|intent| intent.kind == CellTriggerKind::Deactivate));
    assert!(tick_intents
        .iter()
        .any(|intent| intent.kind == CellTriggerKind::Activate));

    let model = engine.model().unwrap();
    let replacement_target = model
        .cells
        .iter()
        .enumerate()
        .position(|(_, active)| !*active)
        .unwrap();
    let existing_star = model
        .cells
        .iter()
        .enumerate()
        .position(|(_, active)| *active)
        .unwrap();
    let input = engine
        .on_input_with_events(
            DeviceInput::GridPress {
                x: replacement_target % GRID_WIDTH,
                y: replacement_target / GRID_WIDTH,
            },
            120.0,
        )
        .unwrap();
    let input_intents = input
        .event_intents
        .iter()
        .filter_map(Option::as_ref)
        .collect::<Vec<_>>();
    assert_eq!(input_intents.len(), 2);
    assert!(input_intents.iter().any(|intent| {
        intent.x == existing_star % GRID_WIDTH
            && intent.y == existing_star / GRID_WIDTH
            && intent.kind == CellTriggerKind::Deactivate
    }));
    assert!(input_intents.iter().any(|intent| {
        intent.x == replacement_target % GRID_WIDTH
            && intent.y == replacement_target / GRID_WIDTH
            && intent.kind == CellTriggerKind::Activate
    }));
}
