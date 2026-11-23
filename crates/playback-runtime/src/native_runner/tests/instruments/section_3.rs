use super::*;
use crate::native_runner::modulation::sampler::apply_sampler_assignments_for_instruments_routed as route;

#[test]
pub(crate) fn sampler_assignment_remaps_note_off_and_suppresses_unmapped_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = InstrumentType::Sampler;
    runner.instruments[0].sample_assignments = vec![NativeSampleAssignment {
        x: 2,
        y: 3,
        sample_slot: 4,
        level: None,
        tune_semis: 0,
        tune_cents: 0,
    }];
    let instruments = vec![runner.instruments[0].clone()];
    let assigned_intent = TileTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind: platform_core::TileTriggerKind::Deactivate,
    };
    let unmapped_intent = TileTriggerIntent {
        x: 1,
        y: 1,
        degree: 0,
        kind: platform_core::TileTriggerKind::Activate,
    };

    let mut held = crate::native_runner::held_keys::HeldKeys::default();
    let pressed = route(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: Some(120),
        }],
        &[TileTriggerIntent {
            kind: platform_core::TileTriggerKind::Activate,
            ..assigned_intent.clone()
        }],
        0,
        &instruments,
        None,
        0,
    )
    .sounded_through(&mut held, &instruments);
    assert_eq!(pressed.tiles.len(), 1);
    let assigned = route(
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 60,
        }],
        &[assigned_intent],
        0,
        &instruments,
        None,
        0,
    )
    .sounded_through(&mut held, &instruments);
    let unmapped = route(
        vec![
            MusicalEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
                duration_ms: Some(120),
            },
            MusicalEvent::NoteOff {
                channel: 0,
                note: 60,
            },
        ],
        &[unmapped_intent.clone(), unmapped_intent],
        0,
        &instruments,
        None,
        0,
    )
    .sounded(&instruments);

    assert!(assigned.audio.is_empty());
    assert!(matches!(
        assigned.tiles.as_slice(),
        [crate::TileHit {
            instrument_slot: 0,
            voice: 4,
            tile: 26,
            velocity: 0,
            ..
        }]
    ));
    assert!(unmapped.is_empty());
}
