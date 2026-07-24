use super::*;
use crate::interpretation::{
    AxisStrategy, CellTriggerIntent, CellTriggerKind, InterpretationEventProfile,
    InterpretationStateProfile, TickStrategy,
};
use crate::mapping::default_mapping_config;
use crate::transforms::{GlobalSoundConfig, VelocityCurve};
use std::collections::BTreeSet;

#[path = "engine_twinkle_tests.rs"]
mod twinkle_tests;

#[test]
fn ticks_life_behavior_end_to_end() {
    let mut engine = NativeLayerEngine::new(NativeLayerEngineConfig {
        behavior: NativeBehavior::Life,
        behavior_config: serde_json::json!({ "cells": [] }),
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
fn cyclic_consecutive_presses_keep_both_activation_intents() {
    let mut engine = NativeLayerEngine::new(NativeLayerEngineConfig {
        behavior: NativeBehavior::Cyclic,
        behavior_config: serde_json::json!({
            "cells": [],
            "states": 4,
            "threshold": 2,
            "range": 1
        }),
        interpretation_profile: InterpretationProfile {
            id: "cyclic_input_events".into(),
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
    .unwrap();

    let first = engine
        .on_input_with_events(DeviceInput::GridPress { x: 2, y: 3 }, 120.0)
        .unwrap();
    let second = engine
        .on_input_with_events(DeviceInput::GridPress { x: 2, y: 3 }, 120.0)
        .unwrap();

    assert_eq!(first.mapped_intents.len(), 1);
    assert_eq!(second.mapped_intents.len(), 1);
    assert_eq!(first.mapped_intents[0].kind, CellTriggerKind::Activate);
    assert_eq!(second.mapped_intents[0].kind, CellTriggerKind::Activate);
}

#[test]
fn held_note_drain_is_bounded_and_returns_note_off_events() {
    let mut engine = NativeLayerEngine::new(NativeLayerEngineConfig {
        behavior: NativeBehavior::Cyclic,
        behavior_config: serde_json::json!({ "cells": [] }),
        interpretation_profile: InterpretationProfile {
            id: "held_note_drain".into(),
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
        note_behaviors: vec![NoteBehavior::Hold; 16],
        layer_index: 0,
    })
    .unwrap();

    engine
        .on_input_with_events(DeviceInput::GridPress { x: 0, y: 0 }, 120.0)
        .unwrap();
    engine
        .on_input_with_events(DeviceInput::GridPress { x: 1, y: 0 }, 120.0)
        .unwrap();

    assert_eq!(
        engine.drain_held_notes(1),
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 24,
        }]
    );
    assert_eq!(
        engine.drain_held_notes(1),
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 27,
        }]
    );
    assert!(engine.drain_held_notes(1).is_empty());
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
        &mut BTreeSet::from([HeldNote {
            channel: 0,
            note: 60,
        }]),
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
        &mut BTreeSet::new(),
    );

    assert_eq!(result.events.len(), 2);
    assert_eq!(result.event_intents, vec![Some(activate), Some(scanned)]);
}

#[test]
fn forest_fire_tree_to_burning_and_grid_press_emit_activate_intents() {
    let mut cells = vec![0; crate::grid::GRID_WIDTH * crate::grid::GRID_HEIGHT];
    cells[crate::grid_index(1, 1)] = 2;
    cells[crate::grid_index(2, 1)] = 1;
    cells[crate::grid_index(5, 5)] = 1;
    let mut engine = NativeLayerEngine::new(NativeLayerEngineConfig {
        behavior: NativeBehavior::ForestFire,
        behavior_config: serde_json::json!({
            "cells": cells,
            "treeDensityPct": 0,
            "reseedThresholdPct": 0,
            "growChancePct": 0,
            "spreadChancePct": 100,
            "lightningChancePerThousand": 0
        }),
        interpretation_profile: InterpretationProfile {
            id: "forest_fire_events".into(),
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
    .unwrap();

    let tick = engine.tick(120.0).unwrap();
    assert!(tick.mapped_intents.iter().any(|intent| {
        intent.x == 2 && intent.y == 1 && intent.kind == CellTriggerKind::Activate
    }));
    assert!(!tick.events.is_empty());

    let input = engine
        .on_input_with_events(DeviceInput::GridPress { x: 5, y: 5 }, 120.0)
        .unwrap();
    assert_eq!(
        input.mapped_intents,
        vec![CellTriggerIntent {
            x: 5,
            y: 5,
            degree: 15,
            kind: CellTriggerKind::Activate
        }]
    );
    assert!(!input.events.is_empty());
}

#[test]
fn life_grid_press_still_emits_input_transition_with_trigger_types() {
    let mut engine = NativeLayerEngine::new(NativeLayerEngineConfig {
        behavior: NativeBehavior::Life,
        behavior_config: Value::Null,
        interpretation_profile: InterpretationProfile {
            id: "life_input_events".into(),
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
    .unwrap();

    let input = engine
        .on_input_with_events(DeviceInput::GridPress { x: 2, y: 3 }, 120.0)
        .unwrap();

    assert_eq!(
        input.mapped_intents,
        vec![CellTriggerIntent {
            x: 2,
            y: 3,
            degree: 8,
            kind: CellTriggerKind::Activate,
        }]
    );
    assert!(!input.events.is_empty());
}

#[test]
fn life_grid_press_with_legacy_stale_trigger_types_falls_back_to_boolean_transition() {
    let mut trigger_types =
        vec![crate::CellTriggerType::None; crate::grid::GRID_WIDTH * crate::grid::GRID_HEIGHT];
    trigger_types[crate::grid_index(0, 0)] = crate::CellTriggerType::Activate;
    let mut engine = NativeLayerEngine::from_serialized_state(
        NativeLayerEngineConfig {
            behavior: NativeBehavior::Life,
            behavior_config: Value::Null,
            interpretation_profile: InterpretationProfile {
                id: "life_stale_input_events".into(),
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
        },
        serde_json::json!({
            "cells": vec![false; crate::grid::GRID_WIDTH * crate::grid::GRID_HEIGHT],
            "height": crate::grid::GRID_HEIGHT,
            "width": crate::grid::GRID_WIDTH,
            "triggerTypes": trigger_types,
            "generation": 0,
            "randomCellsPerTick": 0,
            "randomTickInterval": 1,
            "gliderSpawnInterval": 0,
            "spawnStep": 0
        }),
    )
    .unwrap();

    let input = engine
        .on_input_with_events(DeviceInput::GridPress { x: 2, y: 3 }, 120.0)
        .unwrap();

    assert_eq!(
        input.mapped_intents,
        vec![CellTriggerIntent {
            x: 2,
            y: 3,
            degree: 8,
            kind: CellTriggerKind::Activate,
        }]
    );
}

#[test]
fn brain_grid_press_clears_stale_trigger_types_before_interpretation() {
    let mut trigger_types =
        vec![crate::CellTriggerType::None; crate::grid::GRID_WIDTH * crate::grid::GRID_HEIGHT];
    trigger_types[crate::grid_index(0, 0)] = crate::CellTriggerType::Activate;
    let mut engine = NativeLayerEngine::from_serialized_state(
        NativeLayerEngineConfig {
            behavior: NativeBehavior::Brain,
            behavior_config: Value::Null,
            interpretation_profile: InterpretationProfile {
                id: "brain_input_events".into(),
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
        },
        serde_json::json!({
            "cells": vec![0; crate::grid::GRID_WIDTH * crate::grid::GRID_HEIGHT],
            "triggerTypes": trigger_types,
            "fireThreshold": 2,
            "randomSeedCells": 0,
            "seedInterval": 0,
            "spawnStep": 0
        }),
    )
    .unwrap();

    let input = engine
        .on_input_with_events(DeviceInput::GridPress { x: 4, y: 4 }, 120.0)
        .unwrap();

    assert_eq!(
        input.mapped_intents,
        vec![CellTriggerIntent {
            x: 4,
            y: 4,
            degree: 12,
            kind: CellTriggerKind::Activate,
        }]
    );
    assert!(!input.events.is_empty());
}
