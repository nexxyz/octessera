use super::*;
use crate::queue::{QueueEventClass, MUSICAL_QUEUE_CAPACITY, STRUCTURAL_QUEUE_CAPACITY};
use realtime_engine::synth::{
    prepare_instruments_config, prepare_momentary_fx_start_with_epoch, InstrumentsConfig,
    MomentaryFxTarget, SynthParamId, DEFAULT_PAN_POSITIONS,
};

fn empty_prepared() -> realtime_engine::synth::PreparedInstrumentsConfig {
    prepare_instruments_config(
        InstrumentsConfig {
            instruments: Vec::new(),
            mixer: None,
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        },
        44_100,
    )
}

#[test]
fn latest_controls_replace_without_consuming_fifo_capacity() {
    let (sender, mut receiver) = event_queue();
    for value in 0..10_000 {
        sender
            .send(EngineEvent::SetSynthParam {
                instrument_slot: 0,
                generation: 0,
                param: SynthParamId::AmpGainPct,
                value: value as f32,
            })
            .unwrap();
    }
    assert!(matches!(
        receiver.try_recv_classified(),
        Err(crossbeam_channel::TryRecvError::Empty)
    ));
    let candidate = receiver.take_latest_candidate().expect("latest cell");
    assert_eq!(candidate.value as u32, (9_999_f32).to_bits());
}

#[test]
fn distinct_drum_hits_stay_ordered_in_musical_fifo_and_not_latest_cells() {
    let (sender, mut receiver) = event_queue();
    for (voice, tune_semis) in [(0, -24), (2, 12)] {
        sender
            .send(EngineEvent::DrumHit {
                instrument_slot: 3,
                voice,
                tune_semis,
                velocity: 100,
            })
            .unwrap();
    }
    for (voice, tune_semis) in [(0, -24), (2, 12)] {
        assert!(matches!(receiver.try_recv().unwrap(),
            EngineEvent::DrumHit { instrument_slot: 3, voice: actual_voice,
                tune_semis: actual_tune, velocity: 100 }
            if actual_voice == voice && actual_tune == tune_semis));
    }
    assert!(receiver.take_latest_candidate().is_none());
}

#[test]
fn musical_fifo_is_exact_under_latest_flood() {
    let (sender, mut receiver) = event_queue();
    for note in 0..MUSICAL_QUEUE_CAPACITY {
        sender
            .send(EngineEvent::NoteOn {
                instrument_slot: 0,
                note: (note % 128) as u8,
                velocity: 100,
                duration_ms: 100,
            })
            .unwrap();
        sender
            .send(EngineEvent::SetMasterVolume {
                generation: 0,
                volume_pct: note as f32,
            })
            .unwrap();
    }
    for note in 0..MUSICAL_QUEUE_CAPACITY {
        assert!(matches!(
            receiver.try_recv().unwrap(),
            EngineEvent::NoteOn { note: received, .. } if received == (note % 128) as u8
        ));
    }
    assert!(matches!(
        receiver.try_recv(),
        Err(crossbeam_channel::TryRecvError::Empty)
    ));
}

#[test]
fn structural_queue_is_bounded_and_reports_full() {
    let (sender, _receiver) = event_queue();
    for _ in 0..STRUCTURAL_QUEUE_CAPACITY {
        sender
            .send(EngineEvent::SetPreparedInstruments {
                generation: 0,
                config: empty_prepared(),
            })
            .unwrap();
    }
    let error = sender
        .send(EngineEvent::SetPreparedInstruments {
            generation: 0,
            config: empty_prepared(),
        })
        .unwrap_err();
    assert!(error.is_structural_full());
}

#[test]
fn panic_is_prioritized_and_cancels_older_musical_events() {
    let (sender, mut receiver) = event_queue();
    for note in 0..MUSICAL_QUEUE_CAPACITY {
        sender
            .send(EngineEvent::NoteOn {
                instrument_slot: 0,
                note: note as u8,
                velocity: 100,
                duration_ms: 100,
            })
            .unwrap();
    }
    sender.send(EngineEvent::AllNotesOff).unwrap();
    let (class, event) = receiver.try_recv_classified().unwrap();
    assert_eq!(class, QueueEventClass::Emergency);
    assert!(matches!(event, EngineEvent::AllNotesOff));
    assert!(matches!(
        receiver.try_recv(),
        Err(crossbeam_channel::TryRecvError::Empty)
    ));
}

#[test]
fn structural_and_musical_events_share_sequence_order() {
    let (sender, mut receiver) = event_queue();
    sender
        .send(EngineEvent::SetPreparedInstruments {
            generation: 0,
            config: empty_prepared(),
        })
        .unwrap();
    sender
        .send(EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 100,
            duration_ms: 100,
        })
        .unwrap();
    assert!(matches!(
        receiver.try_recv().unwrap(),
        EngineEvent::SetPreparedInstruments { .. }
    ));
    assert!(matches!(
        receiver.try_recv().unwrap(),
        EngineEvent::NoteOn { .. }
    ));
}

#[test]
fn disconnected_latest_sink_is_not_reported_as_full() {
    let (sender, receiver) = event_queue();
    drop(receiver);
    assert!(matches!(
        sender.send(EngineEvent::SetMasterVolume {
            generation: 0,
            volume_pct: 80.0,
        }),
        Err(QueueSendError::Disconnected {
            queue: QueueKind::Latest
        })
    ));
}

#[test]
fn prepared_momentary_start_remains_structural() {
    let prepared = realtime_engine::synth::prepare_momentary_fx_start(
        "fx".into(),
        "stutter".into(),
        std::collections::BTreeMap::new(),
        MomentaryFxTarget::Global,
        44_100,
    )
    .unwrap();
    let (sender, mut receiver) = event_queue();
    sender
        .send(EngineEvent::PreparedMomentaryFxStart { config: prepared })
        .unwrap();
    assert_eq!(
        receiver.try_recv_classified().unwrap().0,
        QueueEventClass::StructuralRetiring
    );
}

#[test]
fn accepted_momentary_start_is_idempotent() {
    let prepared = prepare_momentary_fx_start_with_epoch(
        "fx".into(),
        1,
        "stutter".into(),
        std::collections::BTreeMap::new(),
        MomentaryFxTarget::Global,
        44_100,
    )
    .unwrap();
    let (sender, mut receiver) = event_queue();
    sender
        .send(EngineEvent::PreparedMomentaryFxStart {
            config: prepared.clone(),
        })
        .unwrap();
    sender
        .send(EngineEvent::PreparedMomentaryFxStart { config: prepared })
        .unwrap();

    assert!(matches!(
        receiver.try_recv_structural(),
        Ok(EngineEvent::PreparedMomentaryFxStart { config }) if config.epoch() == 1
    ));
    assert!(matches!(
        receiver.try_recv_structural(),
        Err(crossbeam_channel::TryRecvError::Empty)
    ));
}

#[test]
fn failed_momentary_start_enqueue_rolls_back_reservation() {
    let (sender, mut receiver) = event_queue();
    for _ in 0..STRUCTURAL_QUEUE_CAPACITY {
        sender
            .send(EngineEvent::SetPreparedInstruments {
                generation: 0,
                config: empty_prepared(),
            })
            .unwrap();
    }
    let start = prepare_momentary_fx_start_with_epoch(
        "fx".into(),
        41,
        "stutter".into(),
        std::collections::BTreeMap::new(),
        MomentaryFxTarget::Global,
        44_100,
    )
    .unwrap();
    assert!(sender
        .send(EngineEvent::PreparedMomentaryFxStart { config: start })
        .unwrap_err()
        .is_structural_full());
    assert!(sender
        .send(EngineEvent::MomentaryFxUpdate(
            realtime_engine::synth::prepare_momentary_fx_update(
                41,
                "stutter".into(),
                std::collections::BTreeMap::new(),
                44_100,
            )
            .unwrap(),
        ))
        .unwrap_err()
        .is_full());

    receiver.try_recv_structural().unwrap();
    let start = prepare_momentary_fx_start_with_epoch(
        "fx".into(),
        41,
        "stutter".into(),
        std::collections::BTreeMap::new(),
        MomentaryFxTarget::Global,
        44_100,
    )
    .unwrap();
    sender
        .send(EngineEvent::PreparedMomentaryFxStart { config: start })
        .unwrap();
    assert!(sender
        .send(EngineEvent::MomentaryFxUpdate(
            realtime_engine::synth::prepare_momentary_fx_update(
                41,
                "stutter".into(),
                std::collections::BTreeMap::new(),
                44_100,
            )
            .unwrap(),
        ))
        .is_ok());
}

#[test]
fn failed_momentary_stop_enqueue_keeps_reservation() {
    let (sender, mut receiver) = event_queue();
    sender
        .send(EngineEvent::PreparedMomentaryFxStart {
            config: prepare_momentary_fx_start_with_epoch(
                "fx".into(),
                42,
                "stutter".into(),
                std::collections::BTreeMap::new(),
                MomentaryFxTarget::Global,
                44_100,
            )
            .unwrap(),
        })
        .unwrap();
    for _ in 1..STRUCTURAL_QUEUE_CAPACITY {
        sender
            .send(EngineEvent::SetPreparedInstruments {
                generation: 0,
                config: empty_prepared(),
            })
            .unwrap();
    }
    assert!(sender
        .send(EngineEvent::MomentaryFxStop { epoch: 42 })
        .unwrap_err()
        .is_structural_full());
    assert!(sender
        .send(EngineEvent::MomentaryFxUpdate(
            realtime_engine::synth::prepare_momentary_fx_update(
                42,
                "stutter".into(),
                std::collections::BTreeMap::new(),
                44_100,
            )
            .unwrap(),
        ))
        .is_ok());

    receiver.try_recv_structural().unwrap();
    sender
        .send(EngineEvent::MomentaryFxStop { epoch: 42 })
        .unwrap();
    assert!(sender
        .send(EngineEvent::MomentaryFxUpdate(
            realtime_engine::synth::prepare_momentary_fx_update(
                42,
                "stutter".into(),
                std::collections::BTreeMap::new(),
                44_100,
            )
            .unwrap(),
        ))
        .is_err());
}

#[test]
fn prepared_instrument_owner_is_one_retiring_structural_event() {
    let (sender, mut receiver) = event_queue();
    sender
        .send(EngineEvent::SetPreparedInstrumentOwner {
            instrument_slot: 0,
            generation: 1,
            config: realtime_engine::synth::prepare_instrument_slot_config(
                realtime_engine::synth::InstrumentSlotConfig {
                    fm: None,
                    pluck: None,
                    drum: None,
                    kind: "synth".into(),
                    synth: realtime_engine::synth::default_synth_config(),
                    mixer: None,
                },
            ),
            sample_bank: None,
        })
        .unwrap();
    assert_eq!(
        receiver.try_recv_classified().unwrap().0,
        QueueEventClass::StructuralRetiring
    );
}
