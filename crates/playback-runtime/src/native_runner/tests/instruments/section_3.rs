use super::*;
use crate::native_runner::modulation_sampler::apply_sampler_assignments_for_instruments;

#[test]
pub(crate) fn sampler_assignment_remaps_note_off_and_suppresses_unmapped_notes() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].sample_assignments = vec![NativeSampleAssignment {
        x: 2,
        y: 3,
        sample_slot: 4,
        level: None,
    }];
    let instruments = vec![runner.instruments[0].clone()];
    let assigned_intent = CellTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind: platform_core::CellTriggerKind::Deactivate,
    };
    let unmapped_intent = CellTriggerIntent {
        x: 1,
        y: 1,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };

    let assigned = apply_sampler_assignments_for_instruments(
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 60,
        }],
        &[assigned_intent],
        0,
        &instruments,
        None,
    );
    let unmapped = apply_sampler_assignments_for_instruments(
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
    );

    assert!(matches!(
        assigned.as_slice(),
        [MusicalEvent::NoteOff {
            channel: 0,
            note: 40
        }]
    ));
    assert!(unmapped.is_empty());
}
