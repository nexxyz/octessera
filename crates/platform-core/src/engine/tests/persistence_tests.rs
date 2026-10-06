use super::*;

#[test]
fn captured_persistence_state_preserves_behavior_serialization_and_owns_snapshot() {
    fn assert_send<T: Send>(_: &T) {}

    let make_engine = |behavior, behavior_config| {
        NativeLayerEngine::new(NativeLayerEngineConfig {
            behavior,
            behavior_config,
            interpretation_profile: InterpretationProfile {
                id: "capture_test".into(),
                event: InterpretationEventProfile { enabled: true },
                state: InterpretationStateProfile {
                    enabled: true,
                    tick: TickStrategy::WholeGridTransitions,
                },
                x: AxisStrategy::ScaleStep { step: 1 },
                y: AxisStrategy::ScaleStep { step: 1 },
            },
            ..base_config()
        })
        .unwrap()
    };

    let mut life = make_engine(NativeBehavior::Life, serde_json::json!({ "cells": [] }));
    life.on_input(DeviceInput::GridPress { x: 2, y: 3 }, 120.0)
        .unwrap();
    let expected_life = life.serialized_state().unwrap();
    let (life_behavior, life_state) = life.capture_persistence_state();
    assert_send(&life_state);
    assert_eq!(life_behavior.serialize(&life_state).unwrap(), expected_life);

    life.on_input(DeviceInput::GridPress { x: 3, y: 3 }, 120.0)
        .unwrap();
    life.tick(120.0).unwrap();
    assert_eq!(life_behavior.serialize(&life_state).unwrap(), expected_life);

    let sequencer = make_engine(NativeBehavior::Sequencer, Value::Null);
    let expected_sequencer = sequencer.serialized_state().unwrap();
    let (sequencer_behavior, sequencer_state) = sequencer.capture_persistence_state();
    assert_send(&sequencer_state);
    assert_eq!(
        sequencer_behavior.serialize(&sequencer_state).unwrap(),
        expected_sequencer
    );
}
