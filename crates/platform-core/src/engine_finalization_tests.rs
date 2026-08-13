use super::*;

#[test]
fn finalize_events_applies_global_sound_before_hold_and_preserves_intents() {
    let mut context = BehaviorContext::new(120.0);
    context.emit(MusicalEvent::Cc {
        channel: 1,
        controller: 74,
        value: 90,
    });
    let emitted = context.emitted_events;
    let first_intent = CellTriggerIntent {
        x: 2,
        y: 3,
        degree: 8,
        kind: CellTriggerKind::Activate,
    };
    let second_intent = CellTriggerIntent {
        x: 3,
        y: 3,
        degree: 9,
        kind: CellTriggerKind::Scanned,
    };
    let mapped = MappingResult {
        events: vec![
            MusicalEvent::NoteOn {
                channel: 0,
                note: 36,
                velocity: 80,
                duration_ms: None,
            },
            MusicalEvent::NoteOn {
                channel: 0,
                note: 36,
                velocity: 96,
                duration_ms: None,
            },
        ],
        intents: vec![first_intent.clone(), second_intent.clone()],
        event_intents: vec![first_intent.clone(), second_intent],
    };
    let config = NativeLayerEngineConfig {
        behavior: NativeBehavior::None,
        behavior_config: Value::Null,
        interpretation_profile: InterpretationProfile {
            id: "finalization_parity".into(),
            event: InterpretationEventProfile { enabled: false },
            state: InterpretationStateProfile {
                enabled: false,
                tick: TickStrategy::WholeGridTransitions,
            },
            x: AxisStrategy::ScaleStep { step: 1 },
            y: AxisStrategy::ScaleStep { step: 2 },
        },
        mapping_config: default_mapping_config(),
        global_sound: GlobalSoundConfig {
            velocity_scale_pct: 150,
            velocity_curve: VelocityCurve::Linear,
            note_length_ms: 777,
        },
        note_behaviors: vec![NoteBehavior::Hold; 16],
        layer_index: 0,
    };
    let mut engine = NativeLayerEngine::new(config).unwrap();

    let finalized = engine.finalize_events(&emitted, Some(&mapped)).unwrap();
    assert_eq!(
        finalized.events,
        vec![
            MusicalEvent::Cc {
                channel: 1,
                controller: 74,
                value: 90,
            },
            MusicalEvent::NoteOn {
                channel: 0,
                note: 36,
                velocity: 120,
                duration_ms: None,
            },
        ]
    );
    assert_eq!(finalized.event_intents, vec![None, Some(first_intent)]);
    assert!(engine.held_notes.contains(&HeldNote {
        channel: 0,
        note: 36,
    }));

    let emitted_only = engine.finalize_events(&emitted, None).unwrap();
    assert_eq!(
        emitted_only.events,
        vec![MusicalEvent::Cc {
            channel: 1,
            controller: 74,
            value: 90,
        }]
    );
    assert_eq!(emitted_only.event_intents, vec![None]);
    assert!(engine.held_notes.contains(&HeldNote {
        channel: 0,
        note: 36,
    }));
}
