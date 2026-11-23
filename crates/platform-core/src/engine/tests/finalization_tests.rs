use super::*;

#[test]
fn finalize_events_applies_global_sound_and_preserves_intents() {
    let mut context = BehaviorContext::new(120.0);
    context.emit(MusicalEvent::Cc {
        channel: 1,
        controller: 74,
        value: 90,
    });
    let emitted = context.emitted_events;
    let first_intent = TileTriggerIntent {
        x: 2,
        y: 3,
        degree: 8,
        kind: TileTriggerKind::Activate,
    };
    let second_intent = TileTriggerIntent {
        x: 3,
        y: 3,
        degree: 9,
        kind: TileTriggerKind::Stable,
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
        event_intents: vec![first_intent.clone(), second_intent.clone()],
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
        global_sound: GlobalSoundConfig {
            velocity_scale_pct: 150,
            velocity_curve: VelocityCurve::Linear,
        },
        ..base_config()
    };
    let mut engine = NativeLayerEngine::new(config).unwrap();

    let (events, event_intents) = engine.finalize_events(&emitted, Some(&mapped)).unwrap();
    assert_eq!(
        events,
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
            MusicalEvent::NoteOn {
                channel: 0,
                note: 36,
                velocity: 127,
                duration_ms: None,
            },
        ]
    );
    assert_eq!(
        event_intents,
        vec![None, Some(first_intent), Some(second_intent)]
    );

    let (emitted_only, emitted_only_intents) = engine.finalize_events(&emitted, None).unwrap();
    assert_eq!(
        emitted_only,
        vec![MusicalEvent::Cc {
            channel: 1,
            controller: 74,
            value: 90,
        }]
    );
    assert_eq!(emitted_only_intents, vec![None]);
}
