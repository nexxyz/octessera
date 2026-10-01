use super::*;

#[test]
fn cached_output_repeats_once_and_geometry_mismatch_stays_invalid() {
    let mut cache = PreviousMasterQuantum::new();
    let fresh = [
        1.0_f32.to_bits(),
        (-0.0_f32).to_bits(),
        0x7fc0_1234,
        4.0_f32.to_bits(),
    ];
    let fresh: Vec<f32> = fresh.into_iter().map(f32::from_bits).collect();
    assert_eq!(cache.fresh(2, &fresh), PersistentOutputKind::Fresh);

    let mut repeated = vec![9.0; 4];
    assert_eq!(
        cache.deadline_miss(44_100, 2, &mut repeated),
        PersistentOutputKind::Repeated
    );
    assert_eq!(
        repeated
            .iter()
            .map(|sample| sample.to_bits())
            .collect::<Vec<_>>(),
        fresh
            .iter()
            .map(|sample| sample.to_bits())
            .collect::<Vec<_>>()
    );

    let mut pending = vec![9.0; 4];
    assert_eq!(
        cache.recovery_silence(&mut pending),
        PersistentOutputKind::DroppedSilence
    );
    assert!(pending.iter().all(|sample| sample.to_bits() == 0));

    let mut mismatch = vec![9.0; 6];
    assert_eq!(
        cache.deadline_miss(44_100, 3, &mut mismatch),
        PersistentOutputKind::DroppedSilence
    );
    cache.deadline_recovery();
    let mut no_stale_revival = vec![9.0; 4];
    assert_eq!(
        cache.deadline_miss(44_100, 2, &mut no_stale_revival),
        PersistentOutputKind::DroppedSilence
    );
}

#[test]
fn counters_snapshot_contains_only_previous_master_quantum_totals() {
    let mut cache = PreviousMasterQuantum::new();
    let fresh = [1.0_f32, 0.0, 0.0, 1.0];
    assert_eq!(cache.fresh(2, &fresh), PersistentOutputKind::Fresh);
    let mut repeated = vec![0.0; 4];
    assert_eq!(
        cache.deadline_miss(44_100, 2, &mut repeated),
        PersistentOutputKind::Repeated
    );
    let counters = cache.counters();
    assert_eq!(
        counters,
        PersistentOutputCounters {
            rendered_quantums: 1,
            repeated_quantums: 1,
            dropped_quantums: 0,
            deadline_misses: 1,
            deadline_recoveries: 0,
        }
    );
}

#[cfg(feature = "output-provenance")]
#[test]
fn provenance_counts_consumed_frames_by_refill_kind() {
    let mut cache = PreviousMasterQuantum::new();
    let fresh = [1.0_f32, 0.0, 0.0, 1.0];
    assert_eq!(cache.fresh(2, &fresh), PersistentOutputKind::Fresh);
    cache.consume_frame();
    cache.consume_frame();
    assert_eq!(cache.provenance_snapshot(), Default::default());

    let mut repeated = vec![0.0; 4];
    assert_eq!(
        cache.deadline_miss(44_100, 2, &mut repeated),
        PersistentOutputKind::Repeated
    );
    cache.consume_frame();
    assert_eq!(cache.provenance_snapshot().repeated_quantum_incidents, 1);
    assert_eq!(cache.provenance_snapshot().repeated_pcm_frames, 1);
    cache.consume_frame();
    assert_eq!(cache.provenance_snapshot().repeated_pcm_frames, 2);

    let mut silence = vec![1.0; 4];
    assert_eq!(
        cache.recovery_silence(&mut silence),
        PersistentOutputKind::DroppedSilence
    );
    cache.consume_frame();
    cache.consume_frame();
    assert_eq!(cache.provenance_snapshot().silent_quantum_incidents, 1);
    assert_eq!(cache.provenance_snapshot().silent_pcm_frames, 2);

    let mut fatal = vec![1.0; 2];
    assert_eq!(
        cache.fatal_silence(&mut fatal),
        PersistentOutputKind::FatalSilence
    );
    cache.consume_frame();
    assert_eq!(cache.provenance_snapshot().silent_quantum_incidents, 2);
    assert_eq!(cache.provenance_snapshot().silent_pcm_frames, 3);
}
