use super::*;
use crate::native_runner::modulation_sampler::apply_sampler_assignments_for_instruments_routed;

#[test]
pub(crate) fn synth_and_sampler_events_route_to_audio_while_midi_routes_to_midi() {
    let intent = CellTriggerIntent {
        x: 0,
        y: 0,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };

    for kind in ["synth", "sampler"] {
        let mut instrument = NativeInstrumentSlot {
            kind: kind.into(),
            ..NativeInstrumentSlot::new(0)
        };
        if kind == "sampler" {
            instrument.sample_assignments = vec![NativeSampleAssignment {
                x: 0,
                y: 0,
                sample_slot: 0,
                level: None,
            }];
        }
        let instruments = vec![instrument];
        let routed = apply_sampler_assignments_for_instruments_routed(
            vec![MusicalEvent::NoteOn {
                channel: 0,
                note: 64,
                velocity: 100,
                duration_ms: Some(120),
            }],
            std::slice::from_ref(&intent),
            0,
            &instruments,
            None,
            0,
            None,
        );

        assert!(matches!(
            routed.audio.as_slice(),
            [MusicalEvent::NoteOn { .. }]
        ));
        assert!(routed.midi.is_empty());
    }

    let instruments = vec![NativeInstrumentSlot {
        kind: "midi".into(),
        midi_enabled: true,
        midi_channel: 2,
        ..NativeInstrumentSlot::new(0)
    }];
    let routed = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 64,
            velocity: 100,
            duration_ms: Some(120),
        }],
        &[intent],
        0,
        &instruments,
        None,
        0,
        None,
    );

    assert!(routed.audio.is_empty());
    assert!(matches!(
        routed.midi.as_slice(),
        [MusicalEvent::NoteOn {
            channel: 1,
            note: 64,
            ..
        }]
    ));
}
