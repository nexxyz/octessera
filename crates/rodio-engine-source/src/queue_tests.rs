use super::*;
use realtime_engine::synth::{
    prepare_instruments_config, prepare_momentary_fx_start, InstrumentsConfig, MomentaryFxTarget,
};
use std::collections::BTreeMap;

#[test]
fn coalesced_queue_is_bounded_and_keeps_latest_control() {
    let (sender, mut receiver) = event_queue();
    for value in 0..(COALESCED_QUEUE_CAPACITY + 17) {
        sender
            .send(EngineEvent::SetMasterVolume {
                volume_pct: value as f32,
            })
            .unwrap();
    }
    let mut events = Vec::new();
    while let Ok(event) = receiver.try_recv_coalesced() {
        events.push(event);
    }
    assert!(events.len() <= COALESCED_QUEUE_CAPACITY);
    assert!(matches!(
        events.last(),
        Some(EngineEvent::SetMasterVolume { volume_pct }) if *volume_pct == (COALESCED_QUEUE_CAPACITY + 16) as f32
    ));
}

#[test]
fn ordered_queue_preserves_note_panic_and_momentary_fx_order() {
    let (sender, mut receiver) = event_queue();
    sender
        .send(EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 100,
            duration_ms: 100,
        })
        .unwrap();
    sender.send(EngineEvent::AllNotesOff).unwrap();
    sender
        .send(EngineEvent::MomentaryFxStop { id: "fx".into() })
        .unwrap();
    assert!(matches!(
        receiver.try_recv_ordered().unwrap(),
        EngineEvent::NoteOn { note: 60, .. }
    ));
    assert!(matches!(
        receiver.try_recv_ordered().unwrap(),
        EngineEvent::AllNotesOff
    ));
    assert!(matches!(
        receiver.try_recv_ordered().unwrap(),
        EngineEvent::MomentaryFxStop { id } if id == "fx"
    ));
    let _ = MomentaryFxTarget::Global;
    let _ = BTreeMap::<String, serde_json::Value>::new();
}

#[test]
fn ordered_queue_full_is_typed_and_does_not_silently_drop_panic() {
    let (sender, mut receiver) = event_queue();
    for _ in 0..ORDERED_QUEUE_CAPACITY {
        sender
            .send(EngineEvent::NoteOn {
                instrument_slot: 0,
                note: 60,
                velocity: 100,
                duration_ms: 100,
            })
            .unwrap();
    }

    let error = sender
        .send(EngineEvent::NoteOff {
            instrument_slot: 0,
            note: 60,
        })
        .unwrap_err();
    assert!(error.is_full());
    assert!(error.is_ordered_full());
    assert!(matches!(
        error,
        QueueSendError::Full {
            queue: QueueKind::Ordered
        }
    ));
    assert!(matches!(
        receiver.try_recv_ordered(),
        Ok(EngineEvent::NoteOn { .. })
    ));
}

#[test]
fn ordered_queue_keeps_preview_and_momentary_events_fifo() {
    let (sender, mut receiver) = event_queue();
    sender
        .send(EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 100,
            duration_ms: 100,
        })
        .unwrap();
    sender
        .send(EngineEvent::NoteOff {
            instrument_slot: 0,
            note: 60,
        })
        .unwrap();
    sender
        .send(EngineEvent::PreviewSample {
            instrument_slot: 0,
            buffer: realtime_engine::synth::SampleBuffer {
                samples: vec![0.0].into_boxed_slice().into(),
                channels: 1,
                sample_rate: 44_100,
            },
            velocity: 100,
        })
        .unwrap();
    sender
        .send(EngineEvent::PreparedMomentaryFxStart(
            prepare_momentary_fx_start(
                "fx".into(),
                "stutter".into(),
                BTreeMap::new(),
                MomentaryFxTarget::Global,
                44_100,
            )
            .unwrap(),
        ))
        .unwrap();

    assert!(matches!(
        receiver.try_recv_ordered().unwrap(),
        EngineEvent::NoteOn { .. }
    ));
    assert!(matches!(
        receiver.try_recv_ordered().unwrap(),
        EngineEvent::NoteOff { .. }
    ));
    assert!(matches!(
        receiver.try_recv_ordered().unwrap(),
        EngineEvent::PreviewSample { .. }
    ));
    assert!(matches!(
        receiver.try_recv_ordered().unwrap(),
        EngineEvent::PreparedMomentaryFxStart(_)
    ));
}

#[test]
fn structural_and_control_events_keep_cross_category_order() {
    let (sender, mut receiver) = event_queue();
    let prepared = prepare_instruments_config(
        InstrumentsConfig {
            instruments: Vec::new(),
            mixer: None,
            pan_positions: 33,
            master_volume: 100.0,
        },
        44_100,
    );
    sender
        .send(EngineEvent::SetMasterVolume { volume_pct: 80.0 })
        .unwrap();
    sender
        .send(EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 100,
            duration_ms: 100,
        })
        .unwrap();
    sender
        .send(EngineEvent::SetPreparedInstruments(prepared))
        .unwrap();

    assert!(matches!(
        receiver.try_recv().unwrap(),
        EngineEvent::SetMasterVolume { .. }
    ));
    assert!(matches!(
        receiver.try_recv().unwrap(),
        EngineEvent::NoteOn { .. }
    ));
    assert!(matches!(
        receiver.try_recv().unwrap(),
        EngineEvent::SetPreparedInstruments(_)
    ));
}

#[test]
fn emergency_all_notes_off_bypasses_a_full_ordered_queue() {
    let (sender, mut receiver) = event_queue();
    for _ in 0..ORDERED_QUEUE_CAPACITY {
        sender
            .send(EngineEvent::NoteOn {
                instrument_slot: 0,
                note: 60,
                velocity: 100,
                duration_ms: 100,
            })
            .unwrap();
    }
    sender.send(EngineEvent::AllNotesOff).unwrap();

    for _ in 0..ORDERED_QUEUE_CAPACITY {
        assert!(matches!(
            receiver.try_recv().unwrap(),
            EngineEvent::NoteOn { .. }
        ));
    }
    assert!(matches!(
        receiver.try_recv().unwrap(),
        EngineEvent::AllNotesOff
    ));
}

#[test]
fn queue_disconnect_is_typed_for_coalesced_controls() {
    let (sender, receiver) = event_queue();
    drop(receiver);
    let error = sender
        .send(EngineEvent::SetMasterVolume { volume_pct: 80.0 })
        .unwrap_err();
    assert!(matches!(
        error,
        QueueSendError::Disconnected {
            queue: QueueKind::Coalesced
        }
    ));
}
