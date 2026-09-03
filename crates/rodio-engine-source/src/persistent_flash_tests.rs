use super::*;
use realtime_engine::synth::AudioLoadStatus;
use std::time::Duration;

const TEST_BLOCK_FRAMES: usize = 2048;
const OUTPUT_CHANNELS: usize = 2;

fn runtime(source: &mut EngineSource) -> &mut realtime_engine::synth::SourceWorkerRuntime {
    &mut source
        .worker_state
        .worker
        .as_mut()
        .expect("persistent worker")
        .runtime
}

fn drain_statuses(receiver: &AudioLoadStatusReceiver, statuses: &mut Vec<AudioLoadStatus>) {
    while let Ok(status) = receiver.try_recv() {
        statuses.push(status);
    }
}

fn consume_frames(
    source: &mut EngineSource,
    receiver: &AudioLoadStatusReceiver,
    statuses: &mut Vec<AudioLoadStatus>,
    frames: u64,
) {
    for _ in 0..frames {
        assert!(source.next().is_some());
        assert!(source.next().is_some());
        if source
            .idx
            .is_multiple_of(TEST_BLOCK_FRAMES * OUTPUT_CHANNELS)
        {
            drain_statuses(receiver, statuses);
        }
    }
    drain_statuses(receiver, statuses);
}

fn false_flash_count(statuses: &[AudioLoadStatus], deadline_misses: u64) -> usize {
    statuses
        .iter()
        .filter(|status| status.deadline_misses == deadline_misses && !status.missed_quantum_flash)
        .count()
}

#[test]
fn iterator_consumed_flash_boundaries_are_exact_at_supported_rates() {
    for sample_rate in [44_100_u32, 48_000] {
        let (tx, rx) = event_queue();
        let (load_tx, load_rx) = audio_load_status_channel();
        let (mut source, shutdown) = EngineSource::with_persistent_workers(
            rx,
            sample_rate,
            TEST_BLOCK_FRAMES,
            Some(load_tx),
        )
        .unwrap();
        let full_duration = u64::from(sample_rate) * 5;
        let mut statuses = Vec::new();

        runtime(&mut source).set_deadline_for_test(Duration::from_secs(1));
        tx.send(EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 100,
            duration_ms: 10_000,
        })
        .unwrap();
        consume_frames(
            &mut source,
            &load_rx,
            &mut statuses,
            TEST_BLOCK_FRAMES as u64,
        );
        assert_eq!(source.persistent_output.rendered_quantums, 1);

        runtime(&mut source).set_pause_for_parity_for_test(0, true);
        runtime(&mut source).set_pause_for_parity_for_test(1, true);
        runtime(&mut source).set_deadline_for_test(Duration::ZERO);
        source.refill();
        drain_statuses(&load_rx, &mut statuses);
        assert_eq!(
            source.persistent_output.flash_frames_remaining,
            full_duration
        );
        assert!(statuses
            .iter()
            .any(|status| { status.deadline_misses == 1 && status.missed_quantum_flash }));

        consume_frames(&mut source, &load_rx, &mut statuses, 1);
        source.refill();
        assert_eq!(
            source.persistent_output.flash_frames_remaining,
            full_duration - 1
        );
        consume_frames(
            &mut source,
            &load_rx,
            &mut statuses,
            TEST_BLOCK_FRAMES as u64,
        );

        runtime(&mut source).set_pause_for_parity_for_test(0, false);
        runtime(&mut source).set_pause_for_parity_for_test(1, false);
        runtime(&mut source).set_deadline_for_test(Duration::from_secs(1));
        let mut consumed = 1 + TEST_BLOCK_FRAMES as u64;
        let mut recovered = false;
        for _ in 0..TEST_BLOCK_FRAMES * 64 {
            consume_frames(&mut source, &load_rx, &mut statuses, 1);
            consumed += 1;
            if source.persistent_output.rendered_quantums == 2 {
                recovered = true;
                break;
            }
        }
        assert!(recovered);
        assert_eq!(source.persistent_output.repeated_quantums, 1);
        assert!(source.persistent_output.dropped_quantums >= 1);
        assert_eq!(
            source.persistent_output.flash_frames_remaining,
            full_duration - consumed
        );

        let before_boundary = full_duration - consumed;
        consume_frames(&mut source, &load_rx, &mut statuses, before_boundary - 1);
        assert_eq!(source.persistent_output.flash_frames_remaining, 1);
        assert_eq!(false_flash_count(&statuses, 1), 0);

        consume_frames(&mut source, &load_rx, &mut statuses, 1);
        assert_eq!(source.persistent_output.flash_frames_remaining, 0);
        assert_eq!(false_flash_count(&statuses, 1), 1);
        consume_frames(&mut source, &load_rx, &mut statuses, 1);
        assert_eq!(false_flash_count(&statuses, 1), 1);

        runtime(&mut source).set_pause_for_parity_for_test(0, true);
        runtime(&mut source).set_pause_for_parity_for_test(1, true);
        runtime(&mut source).set_deadline_for_test(Duration::ZERO);
        source.refill();
        drain_statuses(&load_rx, &mut statuses);
        assert_eq!(
            source.persistent_output.flash_frames_remaining,
            full_duration
        );
        assert!(statuses
            .iter()
            .any(|status| { status.deadline_misses == 2 && status.missed_quantum_flash }));

        runtime(&mut source).set_pause_for_parity_for_test(0, false);
        runtime(&mut source).set_pause_for_parity_for_test(1, false);
        drop(source);
        assert_eq!(shutdown.shutdown().joined_workers, 2);
    }
}
