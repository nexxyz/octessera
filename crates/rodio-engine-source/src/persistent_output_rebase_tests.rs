use super::*;
use realtime_engine::synth::SourceWorkerRuntime;
use std::time::Duration;

const BLOCK_FRAMES: usize = 128;

fn runtime(source: &mut EngineSource) -> &mut SourceWorkerRuntime {
    &mut source
        .worker_state
        .worker
        .as_mut()
        .expect("persistent worker")
        .runtime
}

fn recovery_source() -> (EngineSource, EngineSourceWorkerShutdownOwner) {
    let (_tx, mut source, shutdown, hold_control) =
        super::persistent_terminal_tests::gated_source(true, None, false);
    runtime(&mut source).set_deadline_for_test(Duration::from_secs(1));
    hold_control.release();
    for _ in 0..BLOCK_FRAMES * 2 {
        source.next();
    }
    assert_eq!(source.persistent_output_counters().rendered_quantums, 1);
    assert_eq!(source.persistent_output_counters().dropped_quantums, 0);
    runtime(&mut source).set_pause_for_parity_for_test(0, true);
    runtime(&mut source).set_pause_for_parity_for_test(1, true);
    runtime(&mut source).set_deadline_for_test(Duration::ZERO);
    (source, shutdown)
}

fn finish_recovery_source(mut source: EngineSource, shutdown: EngineSourceWorkerShutdownOwner) {
    runtime(&mut source).set_pause_for_parity_for_test(0, false);
    runtime(&mut source).set_pause_for_parity_for_test(1, false);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn repeated_rebase_counts_carry_in_once_and_restarts_frames() {
    let (mut source, shutdown) = recovery_source();
    source.next();
    source.next();
    let repeated_counters = source.persistent_output_counters();
    assert_eq!(repeated_counters.repeated_quantums, 1);
    assert_eq!(repeated_counters.dropped_quantums, 0);
    assert_eq!(
        source
            .persistent_output_provenance_snapshot()
            .repeated_pcm_frames,
        1
    );

    source.rebase_persistent_output_provenance();
    assert_eq!(
        source.persistent_output_provenance_snapshot(),
        Default::default()
    );
    for _ in 0..(BLOCK_FRAMES - 1) * 2 {
        source.next();
    }
    let snapshot = source.persistent_output_provenance_snapshot();
    assert_eq!(snapshot.repeated_quantum_incidents, 1);
    assert_eq!(snapshot.repeated_pcm_frames, (BLOCK_FRAMES - 1) as u64);
    assert_eq!(snapshot.silent_quantum_incidents, 0);
    finish_recovery_source(source, shutdown);
}

#[test]
fn silent_rebase_counts_carry_in_once_and_restarts_frames() {
    let (mut source, shutdown) = recovery_source();
    for _ in 0..BLOCK_FRAMES * 2 {
        source.next();
    }
    let repeated_counters = source.persistent_output_counters();
    assert_eq!(repeated_counters.repeated_quantums, 1);
    assert_eq!(repeated_counters.dropped_quantums, 0);
    source.next();
    source.next();
    let silence_counters = source.persistent_output_counters();
    assert_eq!(silence_counters.repeated_quantums, 1);
    assert_eq!(silence_counters.dropped_quantums, 1);
    assert_eq!(
        source
            .persistent_output_provenance_snapshot()
            .silent_pcm_frames,
        1
    );

    source.rebase_persistent_output_provenance();
    for _ in 0..(BLOCK_FRAMES - 1) * 2 {
        source.next();
    }
    let snapshot = source.persistent_output_provenance_snapshot();
    assert_eq!(snapshot.silent_quantum_incidents, 1);
    assert_eq!(snapshot.silent_pcm_frames, (BLOCK_FRAMES - 1) as u64);
    assert_eq!(snapshot.repeated_quantum_incidents, 0);
    finish_recovery_source(source, shutdown);
}

#[test]
fn fresh_rebase_stays_zero_and_partial_source_drop_does_not_count_more() {
    let (tx, rx) = event_queue();
    let (mut source, shutdown) =
        EngineSource::with_persistent_workers(rx, 44_100, BLOCK_FRAMES, None).unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 10_000,
    })
    .unwrap();
    source.next();
    source.rebase_persistent_output_provenance();
    assert_eq!(
        source.persistent_output_provenance_snapshot(),
        Default::default()
    );
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn repeated_block_drop_after_rebase_does_not_invent_an_incident() {
    let (mut source, shutdown) = recovery_source();
    source.next();
    source.next();
    source.rebase_persistent_output_provenance();
    assert_eq!(
        source.persistent_output_provenance_snapshot(),
        Default::default()
    );
    finish_recovery_source(source, shutdown);
}
