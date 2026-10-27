use super::*;
use std::time::Duration;

const BLOCK_FRAMES: usize = 128;

fn append_block_bits(source: &mut EngineSource, output: &mut Vec<u32>) {
    for _ in 0..BLOCK_FRAMES * 2 {
        output.push(source.next().unwrap().to_bits());
    }
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
fn miss_and_recovery_silence_paths_have_no_callback_memory_activity() {
    let (tx, rx) = event_queue();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, BLOCK_FRAMES, None).unwrap();
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
