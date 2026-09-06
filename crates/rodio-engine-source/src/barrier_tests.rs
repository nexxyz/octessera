use super::*;
use std::sync::mpsc;
use std::time::Instant;

const BLOCK_FRAMES: usize = 128;
const OUTPUT_SAMPLES: usize = BLOCK_FRAMES * 2;

#[test]
fn inline_probe_marks_fence_two_refills() {
    let (tx, rx) = event_queue();
    let source = EngineSource::with_block_frames(rx, 44_100, BLOCK_FRAMES);
    assert_two_probe_marks_fence(tx, source, None);
}

#[test]
fn persistent_probe_marks_fence_two_refills() {
    let (tx, rx) = event_queue();
    let (source, shutdown) =
        EngineSource::with_persistent_workers_for_benchmark(rx, 44_100, BLOCK_FRAMES, None)
            .unwrap();
    assert_two_probe_marks_fence(tx, source, Some(shutdown));
}

#[cfg(feature = "routing-tree-executor")]
#[test]
fn routing_tree_probe_marks_fence_two_refills() {
    let (tx, rx) = event_queue();
    let (source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, BLOCK_FRAMES, None).unwrap();
    assert_two_probe_marks_fence(tx, source, Some(shutdown));
}

fn assert_two_probe_marks_fence(
    tx: EngineEventSender,
    mut source: EngineSource,
    shutdown: Option<EngineSourceWorkerShutdownOwner>,
) {
    let (first_tx, first_rx) = mpsc::sync_channel(1);
    let (second_tx, second_rx) = mpsc::sync_channel(1);
    tx.send(probe(first_tx)).unwrap();
    tx.send(probe(second_tx)).unwrap();

    source.next();
    assert_eq!(source.refill_generation_for_test(), 1);
    assert!(first_rx.try_recv().is_ok());
    assert!(second_rx.try_recv().is_err());

    for _ in 1..OUTPUT_SAMPLES {
        source.next();
    }
    source.next();
    assert_eq!(source.refill_generation_for_test(), 2);
    assert!(second_rx.try_recv().is_ok());

    drop(source);
    if let Some(shutdown) = shutdown {
        assert_eq!(shutdown.shutdown().joined_workers, 2);
    }
}

fn probe(report_tx: mpsc::SyncSender<u128>) -> EngineEvent {
    EngineEvent::ProbeMark {
        sent_at: Instant::now(),
        report_tx,
    }
}
