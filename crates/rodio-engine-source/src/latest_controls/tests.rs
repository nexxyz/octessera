use super::{LatestCell, LatestControls, LatestCursor, MASTER_CELL};
use crate::queue_types::{QueueKind, QueueSendError};
use crate::EngineEvent;
use realtime_engine::synth::{
    prepare_momentary_fx_update, DrumParamId, FmParamId, PluckParamId, SynthParamId,
};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

fn momentary_event(epoch: u64) -> EngineEvent {
    EngineEvent::MomentaryFxUpdate(
        prepare_momentary_fx_update(epoch, "stutter".into(), BTreeMap::new(), 44_100)
            .expect("momentary update"),
    )
}

#[test]
fn fm_index_coalesces_in_its_own_instrument_cell() {
    let (sender, mut receiver) = crate::event_queue();
    for value in [30.0, 85.0] {
        sender
            .send(EngineEvent::SetFmParam {
                instrument_slot: 2,
                generation: 7,
                param: FmParamId::Index,
                value,
            })
            .unwrap();
    }
    sender
        .send(EngineEvent::SetSynthParam {
            instrument_slot: 2,
            generation: 7,
            param: SynthParamId::AmpGainPct,
            value: 60.0,
        })
        .unwrap();
    let mut candidates = Vec::new();
    while let Some(candidate) = receiver.take_latest_candidate() {
        candidates.push((
            candidate.key,
            candidate.generation,
            f32::from_bits(candidate.value as u32),
        ));
        receiver.mark_latest_applied(candidate);
    }
    assert_eq!(candidates.len(), 2);
    assert!(candidates.contains(&(super::LatestKey::FmParam(2, FmParamId::Index), 7, 85.0)));
    assert!(candidates.contains(&(
        super::LatestKey::SynthParam(2, SynthParamId::AmpGainPct),
        7,
        60.0
    )));
}

#[test]
fn pluck_decay_coalesces_without_colliding_with_brightness_or_other_slots() {
    let (sender, mut receiver) = crate::event_queue();
    for value in [200.0, 1_500.0] {
        sender
            .send(EngineEvent::SetPluckParam {
                instrument_slot: 2,
                generation: 7,
                param: PluckParamId::DecayMs,
                value,
            })
            .unwrap();
    }
    sender
        .send(EngineEvent::SetPluckParam {
            instrument_slot: 2,
            generation: 7,
            param: PluckParamId::BrightnessPct,
            value: 83.0,
        })
        .unwrap();
    let mut candidates = Vec::new();
    while let Some(candidate) = receiver.take_latest_candidate() {
        candidates.push((
            candidate.key,
            candidate.generation,
            f32::from_bits(candidate.value as u32),
        ));
        receiver.mark_latest_applied(candidate);
    }
    assert_eq!(candidates.len(), 2);
    assert!(candidates.contains(&(
        super::LatestKey::PluckParam(2, PluckParamId::DecayMs),
        7,
        1_500.0
    )));
    assert!(candidates.contains(&(
        super::LatestKey::PluckParam(2, PluckParamId::BrightnessPct),
        7,
        83.0
    )));
}

#[test]
fn drum_scalar_latest_wins_only_within_same_slot_voice_and_parameter() {
    let (sender, mut receiver) = crate::event_queue();
    for (slot, voice, param, value) in [
        (0, 2, DrumParamId::DecayMs, 100.0),
        (0, 2, DrumParamId::DecayMs, 220.0),
        (0, 3, DrumParamId::DecayMs, 300.0),
        (1, 2, DrumParamId::DecayMs, 400.0),
        (0, 2, DrumParamId::TonePct, 70.0),
    ] {
        sender
            .send(EngineEvent::SetDrumParam {
                instrument_slot: slot,
                voice,
                generation: 7,
                param,
                value,
            })
            .unwrap();
    }
    let mut candidates = Vec::new();
    while let Some(candidate) = receiver.take_latest_candidate() {
        candidates.push((
            candidate.key,
            candidate.generation,
            f32::from_bits(candidate.value as u32),
        ));
        receiver.mark_latest_applied(candidate);
    }
    assert_eq!(candidates.len(), 4);
    assert!(candidates.contains(&(
        super::LatestKey::DrumParam(0, 2, DrumParamId::DecayMs),
        7,
        220.0
    )));
    assert!(candidates.contains(&(
        super::LatestKey::DrumParam(0, 3, DrumParamId::DecayMs),
        7,
        300.0
    )));
    assert!(candidates.contains(&(
        super::LatestKey::DrumParam(1, 2, DrumParamId::DecayMs),
        7,
        400.0
    )));
    assert!(candidates.contains(&(
        super::LatestKey::DrumParam(0, 2, DrumParamId::TonePct),
        7,
        70.0
    )));
}

#[test]
fn concurrent_latest_publishers_never_torn_generation_value() {
    const WRITERS: usize = 8;
    const ITERATIONS: usize = 10_000;
    const VALUE_MASK: u64 = 0xa5a5_5a5a_1234_5678;
    let cell = Arc::new(LatestCell::new());
    let start = Arc::new(Barrier::new(WRITERS + 1));
    let running = Arc::new(AtomicBool::new(true));
    let reader_cell = Arc::clone(&cell);
    let reader_start = Arc::clone(&start);
    let reader_running = Arc::clone(&running);
    let reader = thread::spawn(move || {
        reader_start.wait();
        while reader_running.load(Ordering::Acquire) {
            if let Some((_, generation, value)) = reader_cell.snapshot(0) {
                assert_eq!(value, generation ^ VALUE_MASK);
            }
        }
    });
    let writers: Vec<_> = (0..WRITERS)
        .map(|writer| {
            let cell = Arc::clone(&cell);
            let start = Arc::clone(&start);
            thread::spawn(move || {
                start.wait();
                for iteration in 0..ITERATIONS {
                    let generation = ((writer as u64) << 32) | iteration as u64;
                    cell.publish(generation, generation ^ VALUE_MASK);
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    running.store(false, Ordering::Release);
    reader.join().unwrap();
}

#[test]
fn latest_publish_contention_is_bounded_and_typed() {
    let controls = LatestControls::new();
    controls.cells[MASTER_CELL]
        .revision
        .store(1, Ordering::Release);
    let started = std::time::Instant::now();
    let result = controls.publish_event(EngineEvent::SetMasterVolume {
        generation: 1,
        volume_pct: 80.0,
    });
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
    assert_eq!(
        result,
        Err(QueueSendError::Full {
            queue: QueueKind::Latest
        })
    );
}

#[test]
fn concurrent_momentary_publishers_claim_one_epoch_slot() {
    const WRITERS: usize = 8;
    const EPOCH: u64 = 17;
    let controls = Arc::new(LatestControls::new());
    assert_eq!(controls.reserve_momentary_epoch(EPOCH), Ok(true));
    let start = Arc::new(Barrier::new(WRITERS + 1));
    let writers: Vec<_> = (0..WRITERS)
        .map(|_| {
            let controls = Arc::clone(&controls);
            let start = Arc::clone(&start);
            thread::spawn(move || {
                start.wait();
                controls.publish_event(EngineEvent::MomentaryFxUpdate(
                    prepare_momentary_fx_update(EPOCH, "stutter".into(), BTreeMap::new(), 44_100)
                        .expect("momentary update"),
                ))
            })
        })
        .collect();
    start.wait();
    for writer in writers {
        let result = writer.join().unwrap();
        assert!(
            result.is_ok()
                || matches!(
                    result,
                    Err(QueueSendError::Full {
                        queue: QueueKind::Latest
                    })
                )
        );
    }
    assert_eq!(controls.momentary.claimed_epoch_count(), 1);
}

#[test]
fn momentary_epochs_use_exact_cells_and_release_cancelled_epochs() {
    let controls = LatestControls::new();
    assert_eq!(controls.reserve_momentary_epoch(1), Ok(true));
    assert!(controls.publish_event(momentary_event(1)).is_ok());
    for epoch in 2..=5 {
        assert_eq!(controls.reserve_momentary_epoch(epoch), Ok(true));
        assert!(controls.publish_event(momentary_event(epoch)).is_ok());
        controls.cancel_epoch(epoch);
    }
    for epoch in 2..=5 {
        assert_eq!(
            controls.publish_event(momentary_event(epoch)),
            Err(QueueSendError::Full {
                queue: QueueKind::Latest
            })
        );
    }
    assert_eq!(controls.reserve_momentary_epoch(6), Ok(true));
    assert!(controls.publish_event(momentary_event(6)).is_ok());
    assert!(controls.publish_event(momentary_event(1)).is_ok());

    let mut cursor = LatestCursor::new();
    let mut epochs = Vec::new();
    while let Some(candidate) = controls.candidate(&mut cursor) {
        epochs.push(candidate.generation);
        controls.mark_applied(&mut cursor, candidate);
    }
    epochs.sort_unstable();
    assert_eq!(epochs, vec![1, 6]);
}

#[test]
fn receiver_cancellation_releases_exact_epoch_before_restart() {
    let (sender, mut receiver) = crate::event_queue();
    sender
        .send(EngineEvent::PreparedMomentaryFxStart {
            config: prepared_start(1),
        })
        .unwrap();
    sender.send(momentary_event(1)).unwrap();
    sender
        .send(EngineEvent::PreparedMomentaryFxStart {
            config: prepared_start(2),
        })
        .unwrap();
    sender.send(momentary_event(2)).unwrap();
    receiver.cancel_latest_epoch(2);
    sender
        .send(EngineEvent::PreparedMomentaryFxStart {
            config: prepared_start(3),
        })
        .unwrap();
    sender.send(momentary_event(3)).unwrap();

    assert_eq!(
        sender.send(momentary_event(2)),
        Err(QueueSendError::Full {
            queue: QueueKind::Latest
        })
    );
    let mut epochs = Vec::new();
    while let Some(candidate) = receiver.take_latest_candidate() {
        epochs.push(candidate.generation);
        receiver.mark_latest_applied(candidate);
    }
    epochs.sort_unstable();
    assert_eq!(epochs, vec![1, 3]);
}

#[test]
fn cancelled_epoch_retry_converges_after_writer_contention() {
    let controls = LatestControls::new();
    assert_eq!(controls.reserve_momentary_epoch(1), Ok(true));
    assert!(controls.publish_event(momentary_event(1)).is_ok());
    assert_eq!(controls.reserve_momentary_epoch(2), Ok(true));
    assert!(controls.publish_event(momentary_event(2)).is_ok());
    assert!(controls.momentary.force_writer_for_epoch(2));

    controls.cancel_epoch(2);
    assert_eq!(
        controls.publish_event(momentary_event(3)),
        Err(QueueSendError::Full {
            queue: QueueKind::Latest
        })
    );
    assert!(controls.momentary.release_for_epoch(2));
    assert_eq!(controls.reserve_momentary_epoch(3), Ok(true));
    assert!(controls.publish_event(momentary_event(3)).is_ok());
    assert_eq!(
        controls.publish_event(momentary_event(2)),
        Err(QueueSendError::Full {
            queue: QueueKind::Latest
        })
    );
    assert!(controls.momentary.claimed_epoch(3));
}

#[test]
fn ten_thousand_concurrent_momentary_publishers_remain_bounded() {
    const WRITERS: usize = 10;
    const ITERATIONS: usize = 1_000;
    let controls = Arc::new(LatestControls::new());
    assert_eq!(controls.reserve_momentary_epoch(1), Ok(true));
    let start = Arc::new(Barrier::new(WRITERS + 1));
    let writers: Vec<_> = (0..WRITERS)
        .map(|_| {
            let controls = Arc::clone(&controls);
            let start = Arc::clone(&start);
            thread::spawn(move || {
                start.wait();
                for _ in 0..ITERATIONS {
                    let _ = controls.publish_event(momentary_event(1));
                }
            })
        })
        .collect();
    start.wait();
    for writer in writers {
        writer.join().unwrap();
    }

    assert!(controls.momentary.claimed_epoch_count() <= super::MOMENTARY_SLOT_COUNT);
}

#[test]
fn cancelled_publisher_cannot_land_in_a_reused_cell() {
    const EPOCH: u64 = 17;
    let controls = Arc::new(LatestControls::new());
    assert_eq!(controls.reserve_momentary_epoch(EPOCH), Ok(true));
    let start = Arc::new(Barrier::new(2));
    let publisher_controls = Arc::clone(&controls);
    let publisher_start = Arc::clone(&start);
    let publisher = thread::spawn(move || {
        publisher_start.wait();
        for _ in 0..1_000 {
            let _ = publisher_controls.publish_event(momentary_event(EPOCH));
        }
    });
    start.wait();
    controls.cancel_epoch(EPOCH);
    assert_eq!(controls.reserve_momentary_epoch(EPOCH + 1), Ok(true));
    publisher.join().unwrap();

    assert!(!controls.momentary.claimed_epoch(EPOCH));
    assert!(controls.momentary.claimed_epoch(EPOCH + 1));
    assert_eq!(
        controls.publish_event(momentary_event(EPOCH)),
        Err(QueueSendError::Full {
            queue: QueueKind::Latest
        })
    );
    assert!(controls.publish_event(momentary_event(EPOCH + 1)).is_ok());
}

fn prepared_start(epoch: u64) -> realtime_engine::synth::PreparedMomentaryFxStart {
    realtime_engine::synth::prepare_momentary_fx_start_with_epoch(
        "fx".into(),
        epoch,
        "stutter".into(),
        BTreeMap::new(),
        realtime_engine::synth::MomentaryFxTarget::Global,
        44_100,
    )
    .expect("momentary start")
}
