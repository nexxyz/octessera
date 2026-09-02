use super::*;
use std::mem::size_of;

const MAX_PRACTICAL_RETIREMENT_ITEM_BYTES: usize = 12 * 1024;
const MAX_PRACTICAL_RETIREMENT_STORAGE_BYTES: usize = 4 * 1024 * 1024;

const _: () = assert!(size_of::<RetiredAudioItem>() <= MAX_PRACTICAL_RETIREMENT_ITEM_BYTES);
const _: () = assert!(
    size_of::<RetiredAudioItem>() * (RETIREMENT_QUEUE_CAPACITY + RETIREMENT_BACKLOG_CAPACITY)
        <= MAX_PRACTICAL_RETIREMENT_STORAGE_BYTES
);

#[test]
fn sample_voice_retirement_capacity_matches_callback_budget() {
    assert_eq!(
        SAMPLE_VOICE_RETIREMENT_CAPACITY,
        SAMPLE_VOICE_LANE_CAPACITY + (2 * MAX_CONTROL_EVENTS_PER_CALLBACK)
    );
}

#[test]
fn retired_audio_item_storage_stays_practical() {
    let item_bytes = size_of::<RetiredAudioItem>();
    assert!(item_bytes <= MAX_PRACTICAL_RETIREMENT_ITEM_BYTES);
    assert!(
        item_bytes * (RETIREMENT_QUEUE_CAPACITY + RETIREMENT_BACKLOG_CAPACITY)
            <= MAX_PRACTICAL_RETIREMENT_STORAGE_BYTES
    );
}

#[test]
fn full_sample_burst_replacements_preserve_fifo_and_retirement_ownership() {
    let (tx, mut source, retired_rx) = full_sample_source();
    for index in 0..MAX_CONTROL_EVENTS_PER_CALLBACK - 1 {
        tx.send(EngineEvent::NoteOn {
            instrument_slot: (index % INSTRUMENT_SLOT_COUNT) as u8,
            note: 36,
            velocity: 100,
            duration_ms: 10_000,
        })
        .unwrap();
    }
    tx.send(EngineEvent::SetPreparedAudioConfig(full_sample_config(2.0)))
        .unwrap();

    let (allocation_count, deallocation_count) = callback_memory_activity(&mut source);
    assert_eq!(allocation_count, 0);
    assert_eq!(deallocation_count, 0);
    assert!(source.control_rx.try_recv_ordered().is_err());
    let snapshot = source.engine.profile_snapshot();
    assert_eq!(snapshot.active_sample_voices, 0);
    assert_eq!(
        snapshot.cumulative_voice_steals,
        (MAX_CONTROL_EVENTS_PER_CALLBACK - 1) as u64
    );
    assert_eq!(snapshot.cumulative_voice_admission_drops, 0);

    let bulk_retired = receive_retired_state(&retired_rx);
    let burst_retired = receive_retired_state(&retired_rx);
    assert_eq!(
        bulk_retired.sample_voice_count(),
        SAMPLE_VOICE_LANE_CAPACITY
    );
    assert_eq!(
        burst_retired.sample_voice_count(),
        MAX_CONTROL_EVENTS_PER_CALLBACK - 1
    );
    assert!(retired_rx.try_recv().is_err());
    let (_, source_deallocations) = allocations_and_deallocations(|| drop(source));
    assert!(source_deallocations > 0);
    let (_, bulk_deallocations) = allocations_and_deallocations(|| drop(bulk_retired));
    assert!(bulk_deallocations > 0);
    let (_, burst_deallocations) = allocations_and_deallocations(|| drop(burst_retired));
    assert!(burst_deallocations > 0);
}
