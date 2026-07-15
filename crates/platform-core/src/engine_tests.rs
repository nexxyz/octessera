use super::*;
use crate::interpretation::{
    AxisStrategy, CellTriggerIntent, CellTriggerKind, InterpretationEventProfile,
    InterpretationStateProfile, TickStrategy,
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

#[test]
fn note_behavior_suppression_keeps_event_intents_aligned() {
    let duplicate = CellTriggerIntent {
        x: 1,
        y: 1,
        degree: 0,
        kind: CellTriggerKind::Activate,
    };
    let release = CellTriggerIntent {
        x: 1,
        y: 1,
        degree: 0,
        kind: CellTriggerKind::Deactivate,
    };

    let result = apply_note_behavior_with_event_intents(
        &[
            MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 96,
                duration_ms: Some(120),
            },
            MusicalEvent::NoteOff {
                channel: 0,
                note: 60,
            },
        ],
        vec![Some(duplicate), Some(release.clone())],
        &[NoteBehavior::Hold],
        0,
        &["0:0:60".into()],
    );

    assert_eq!(
        result.events,
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 60,
        }]
    );
    assert_eq!(result.event_intents, vec![Some(release)]);
}

#[test]
fn duplicate_note_ons_keep_distinct_event_intents_for_link_timing() {
    let activate = CellTriggerIntent {
        x: 1,
        y: 1,
        degree: 0,
        kind: CellTriggerKind::Activate,
    };
    let scanned = CellTriggerIntent {
        x: 1,
        y: 1,
        degree: 0,
        kind: CellTriggerKind::Scanned,
    };

    let result = apply_note_behavior_with_event_intents(
        &[
            MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 80,
                duration_ms: Some(120),
            },
            MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 96,
                duration_ms: Some(120),
            },
        ],
        vec![Some(activate.clone()), Some(scanned.clone())],
        &[NoteBehavior::Oneshot],
        0,
        &[],
    );

    assert_eq!(result.events.len(), 2);
    assert_eq!(result.event_intents, vec![Some(activate), Some(scanned)]);
}
