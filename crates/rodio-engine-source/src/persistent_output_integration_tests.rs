use super::*;
use realtime_engine::synth::{SourceWorkerHealth, VoiceStealingMode};
use std::time::{Duration, Instant};

const BLOCK_FRAMES: usize = 128;

fn block_bits(source: &mut EngineSource) -> Vec<u32> {
    (0..BLOCK_FRAMES * 2)
        .map(|_| source.next().unwrap().to_bits())
        .collect()
}

fn append_block_bits(source: &mut EngineSource, output: &mut Vec<u32>) {
    for _ in 0..BLOCK_FRAMES * 2 {
        output.push(source.next().unwrap().to_bits());
    }
}

fn mirror_block_bits(consumer: &mut PcmMirrorConsumer) -> Vec<u32> {
    assert!(consumer.begin_callback());
    (0..BLOCK_FRAMES * 2)
        .map(|_| consumer.next_sample().unwrap().to_bits())
        .collect()
}

fn runtime(source: &mut EngineSource) -> &mut realtime_engine::synth::SourceWorkerRuntime {
    &mut source
        .worker_state
        .worker
        .as_mut()
        .expect("persistent worker")
        .runtime
}

#[test]
fn persistent_miss_repeats_once_then_silences_until_recovery_and_drains_controls() {
    let (tx, mut source, shutdown) = super::persistent_tests::persistent_source(BLOCK_FRAMES);
    let pair = new_pcm_mirror();
    let mut mirror = pair.consumer;
    source.set_pcm_mirror_producers([Some(pair.producer), None]);
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 10_000,
    })
    .unwrap();
    let fresh = block_bits(&mut source);
    assert_eq!(mirror_block_bits(&mut mirror), fresh);
    assert!(fresh.iter().any(|sample| *sample != 0));
    assert_eq!(source.persistent_output.rendered_quantums, 1);

    runtime(&mut source).set_pause_for_parity_for_test(0, true);
    runtime(&mut source).set_pause_for_parity_for_test(1, true);
    runtime(&mut source).set_deadline_for_test(Duration::ZERO);
    let repeated = block_bits(&mut source);
    assert_eq!(mirror_block_bits(&mut mirror), repeated);
    assert_eq!(repeated, fresh);
    assert_eq!(source.persistent_output.repeated_quantums, 1);
    assert_eq!(source.persistent_output.dropped_quantums, 0);
    assert_eq!(source.persistent_output.deadline_misses, 1);

    let pending = block_bits(&mut source);
    assert_eq!(mirror_block_bits(&mut mirror), pending);
    assert!(pending.iter().all(|sample| *sample == 0));
    assert_eq!(source.persistent_output.repeated_quantums, 1);
    assert_eq!(source.persistent_output.dropped_quantums, 1);
    assert_eq!(
        source.source_worker_health(),
        SourceWorkerHealth::DeadlineMiss
    );

    runtime(&mut source).set_pause_for_parity_for_test(0, false);
    runtime(&mut source).set_pause_for_parity_for_test(1, false);
    runtime(&mut source).set_deadline_for_test(Duration::from_secs(1));
    tx.send(EngineEvent::SetVoiceStealingMode(VoiceStealingMode::None))
        .unwrap();
    let mut recovered = false;
    for _ in 0..1_000 {
        let block = block_bits(&mut source);
        let mirrored = mirror_block_bits(&mut mirror);
        assert_eq!(mirrored, block);
        if source.source_worker_health() == SourceWorkerHealth::Healthy {
            recovered = true;
            break;
        }
        std::thread::yield_now();
    }
    assert!(recovered);
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    assert_eq!(source.persistent_output.rendered_quantums, 2);
    assert_eq!(source.persistent_output.deadline_recoveries, 1);
    assert_eq!(source.persistent_output.deadline_misses, 1);
    assert!(!source.persistent_output.repeat_used_for_current_recovery);

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn forced_status_reports_output_counters_and_flash_transitions() {
    let (tx, rx) = event_queue();
    let (load_tx, load_rx) = audio_load_status_channel();
    let (mut source, shutdown) =
        EngineSource::with_persistent_workers(rx, 48_000, BLOCK_FRAMES, Some(load_tx)).unwrap();
    source.last_load_report = Instant::now() - LOAD_REPORT_INTERVAL;
    runtime(&mut source).set_pause_for_parity_for_test(0, true);
    runtime(&mut source).set_pause_for_parity_for_test(1, true);
    runtime(&mut source).set_deadline_for_test(Duration::ZERO);
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 10_000,
    })
    .unwrap();

    let _ = block_bits(&mut source);
    let miss = load_rx.try_recv().expect("miss status");
    assert_eq!(miss.rendered_quantums, 0);
    assert_eq!(miss.repeated_quantums, 0);
    assert_eq!(miss.dropped_quantums, 1);
    assert_eq!(miss.deadline_misses, 1);
    assert_eq!(miss.deadline_recoveries, 0);
    assert!(miss.missed_quantum_flash);

    source.last_load_report = Instant::now() - LOAD_REPORT_INTERVAL;
    let _ = block_bits(&mut source);
    let pending = load_rx.try_recv().expect("pending status");
    assert_eq!(pending.dropped_quantums, 2);
    assert!(pending.missed_quantum_flash);

    runtime(&mut source).set_pause_for_parity_for_test(0, false);
    runtime(&mut source).set_pause_for_parity_for_test(1, false);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn miss_and_recovery_silence_paths_have_no_callback_memory_activity() {
    let (tx, rx) = event_queue();
    let (mut source, shutdown) =
        EngineSource::with_persistent_workers(rx, 44_100, BLOCK_FRAMES, None).unwrap();
    runtime(&mut source).set_pause_for_parity_for_test(0, true);
    runtime(&mut source).set_pause_for_parity_for_test(1, true);
    runtime(&mut source).set_deadline_for_test(Duration::ZERO);
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 10_000,
    })
    .unwrap();
    let mut output = Vec::with_capacity(BLOCK_FRAMES * 2);
    let (allocations, deallocations) = super::allocations_and_deallocations(|| {
        append_block_bits(&mut source, &mut output);
        output.clear();
        append_block_bits(&mut source, &mut output);
    });
    assert_eq!((allocations, deallocations), (0, 0));
    runtime(&mut source).set_pause_for_parity_for_test(0, false);
    runtime(&mut source).set_pause_for_parity_for_test(1, false);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn pending_recovery_status_publication_stays_edge_bounded() {
    let (tx, rx) = event_queue();
    let (load_tx, load_rx) = audio_load_status_channel();
    let (mut source, shutdown) =
        EngineSource::with_persistent_workers(rx, 44_100, BLOCK_FRAMES, Some(load_tx)).unwrap();
    runtime(&mut source).set_pause_for_parity_for_test(0, true);
    runtime(&mut source).set_pause_for_parity_for_test(1, true);
    runtime(&mut source).set_deadline_for_test(Duration::ZERO);
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 10_000,
    })
    .unwrap();

    let _ = block_bits(&mut source);
    for _ in 0..8 {
        let _ = block_bits(&mut source);
    }

    runtime(&mut source).set_pause_for_parity_for_test(0, false);
    runtime(&mut source).set_pause_for_parity_for_test(1, false);
    runtime(&mut source).set_deadline_for_test(Duration::from_secs(1));
    let mut recovered = false;
    for _ in 0..1_000 {
        let _ = block_bits(&mut source);
        if source.source_worker_health() == SourceWorkerHealth::Healthy {
            recovered = true;
            break;
        }
        std::thread::yield_now();
    }
    assert!(recovered);

    let mut statuses = Vec::new();
    while let Ok(status) = load_rx.try_recv() {
        statuses.push(status);
    }
    assert!(statuses.len() <= 2, "statuses={}", statuses.len());
    assert_eq!(
        statuses
            .iter()
            .filter(|status| status.deadline_misses == 1 && status.deadline_recoveries == 0)
            .count(),
        1
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|status| status.deadline_recoveries == 1)
            .count(),
        1
    );

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}
