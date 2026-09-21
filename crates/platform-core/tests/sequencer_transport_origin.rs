use platform_core::{
    default_mapping_config, AxisStrategy, InterpretationEventProfile, InterpretationProfile,
    InterpretationStateProfile, NativeBehavior, NativeLayerEngine, NativeLayerEngineConfig,
    NoteBehavior, TickStrategy,
};
use serde_json::Value;

fn scanning_engine() -> NativeLayerEngine {
    NativeLayerEngine::new(NativeLayerEngineConfig {
        behavior: NativeBehavior::Sequencer,
        behavior_config: Value::Null,
        interpretation_profile: InterpretationProfile {
            id: "sequencer_transport_origin".into(),
            event: InterpretationEventProfile { enabled: false },
            state: InterpretationStateProfile {
                enabled: true,
                tick: TickStrategy::ScanRowActive {
                    sections: None,
                    reverse: false,
                },
            },
            x: AxisStrategy::TimingOnly,
            y: AxisStrategy::TimingOnly,
        },
        mapping_config: default_mapping_config(),
        global_sound: platform_core::GlobalSoundConfig {
            velocity_scale_pct: 100,
            velocity_curve: platform_core::VelocityCurve::Linear,
            note_length_ms: 120,
        },
        note_behaviors: vec![NoteBehavior::Oneshot; 16],
        layer_index: 0,
    })
    .unwrap()
}

#[test]
fn interpretation_tick_setter_preserves_phase_until_transport_reset() {
    let mut engine = scanning_engine();

    engine.set_interpretation_tick(3);
    let continued = engine.tick(120.0).unwrap();
    assert!(continued.mapped_intents.iter().all(|intent| intent.y == 3));

    engine.reset_transport_phase();
    let reset = engine.tick(120.0).unwrap();
    assert!(reset.mapped_intents.iter().all(|intent| intent.y == 0));
}
