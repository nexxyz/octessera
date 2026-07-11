use super::*;
use crate::interpretation::{
    AxisStrategy, InterpretationEventProfile, InterpretationStateProfile, TickStrategy,
};
use crate::mapping::default_mapping_config;
use crate::transforms::{GlobalSoundConfig, VelocityCurve};

#[test]
fn ticks_life_behavior_end_to_end() {
    let mut engine = NativeLayerEngine::new(NativeLayerEngineConfig {
        behavior: NativeBehavior::Life,
        behavior_config: Value::Null,
        interpretation_profile: InterpretationProfile {
            id: "menu_profile".into(),
            event: InterpretationEventProfile { enabled: true },
            state: InterpretationStateProfile {
                enabled: true,
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
    .unwrap();

    engine
        .on_input(DeviceInput::GridPress { x: 2, y: 3 }, 120.0)
        .unwrap();
    engine
        .on_input(DeviceInput::GridPress { x: 3, y: 3 }, 120.0)
        .unwrap();
    engine
        .on_input(DeviceInput::GridPress { x: 4, y: 3 }, 120.0)
        .unwrap();

    let tick = engine.tick(120.0).unwrap();
    assert!(tick.model.cells[crate::grid_index(3, 2)]);
    assert!(!tick.events.is_empty());
}

#[test]
fn scan_interpretation_advances_with_engine_ticks() {
    let mut engine = NativeLayerEngine::new(NativeLayerEngineConfig {
        behavior: NativeBehavior::Sequencer,
        behavior_config: Value::Null,
        interpretation_profile: InterpretationProfile {
            id: "scan_profile".into(),
            event: InterpretationEventProfile { enabled: false },
            state: InterpretationStateProfile {
                enabled: true,
                tick: TickStrategy::ScanRowActive {
                    sections: None,
                    reverse: false,
                },
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
    .unwrap();
    engine
        .on_input(DeviceInput::GridPress { x: 0, y: 1 }, 120.0)
        .unwrap();

    let first = engine.tick(120.0).unwrap();
    let second = engine.tick(120.0).unwrap();

    assert!(first.events.is_empty());
    assert!(!second.events.is_empty());
}
