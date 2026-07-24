use super::*;
use realtime_engine::synth::{
    prepare_audio_config, InstrumentsConfig, SampleBankConfig, DEFAULT_PAN_POSITIONS,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static COUNT_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    static DEALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = System.alloc(layout);
        count_allocation();
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        COUNT_ALLOCATIONS.with(|enabled| {
            if enabled.get() {
                DEALLOCATIONS.with(|deallocations| deallocations.set(deallocations.get() + 1));
            }
        });
        System.dealloc(pointer, layout);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let pointer = System.realloc(pointer, layout, new_size);
        count_allocation();
        pointer
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn count_allocation() {
    COUNT_ALLOCATIONS.with(|enabled| {
        if enabled.get() {
            ALLOCATIONS.with(|allocations| allocations.set(allocations.get() + 1));
        }
    });
}

fn allocations_and_deallocations<F: FnOnce()>(operation: F) -> (usize, usize) {
    ALLOCATIONS.with(|allocations| allocations.set(0));
    DEALLOCATIONS.with(|deallocations| deallocations.set(0));
    COUNT_ALLOCATIONS.with(|enabled| enabled.set(true));
    operation();
    COUNT_ALLOCATIONS.with(|enabled| enabled.set(false));
    (ALLOCATIONS.with(Cell::get), DEALLOCATIONS.with(Cell::get))
}

#[test]
fn idle_source_refills_with_silence() {
    let (_tx, rx) = event_queue();
    let mut source = EngineSource::new(rx, 44_100);

    for _ in 0..128 {
        assert_eq!(source.next(), Some(0.0));
    }
}

#[test]
fn note_on_after_idle_renders_audio() {
    let (tx, rx) = event_queue();
    let mut source = EngineSource::new(rx, 44_100);
    for _ in 0..64 {
        assert_eq!(source.next(), Some(0.0));
    }

    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 1_000,
    })
    .unwrap();

    let mut saw_audio = false;
    for _ in 0..4096 {
        if source.next().unwrap_or(0.0).abs() > f32::EPSILON {
            saw_audio = true;
            break;
        }
    }
    assert!(saw_audio);
}

#[test]
fn all_notes_off_event_clears_engine_voices() {
    let (tx, rx) = event_queue();
    let mut source = EngineSource::new(rx, 44_100);
    source.engine.note_on(0, 60, 100, 1_000);
    source.engine.preview_sample(
        0,
        realtime_engine::synth::SampleBuffer {
            samples: vec![0.25; 256].into_boxed_slice().into(),
            channels: 1,
            sample_rate: 44_100,
        },
        100,
    );
    tx.send(EngineEvent::AllNotesOff).unwrap();

    for _ in 0..20_000 {
        let _ = source.next();
    }

    let snapshot = source.engine.profile_snapshot();
    assert_eq!(snapshot.active_synth_voices, 0);
    assert_eq!(snapshot.active_preview_sample_voices, 0);
}

#[test]
fn control_drain_has_a_fixed_per_block_budget() {
    let (tx, rx) = event_queue();
    for note in 0..(MAX_CONTROL_EVENTS_PER_BLOCK + 11) {
        tx.send(EngineEvent::NoteOn {
            instrument_slot: 0,
            note: (note % 128) as u8,
            velocity: 96,
            duration_ms: 100,
        })
        .unwrap();
    }
    let mut source = EngineSource::new(rx, 44_100);
    let drained = source.drain_control_events();
    assert_eq!(drained.control_events, MAX_CONTROL_EVENTS_PER_BLOCK as u64);
    assert!(source.control_rx.try_recv_ordered().is_ok());
}

#[test]
fn prepared_control_path_does_not_allocate_while_refilling() {
    let prepared = prepare_audio_config(
        InstrumentsConfig {
            instruments: Vec::new(),
            mixer: None,
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        },
        Some(vec![SampleBankConfig::default()]),
        None,
        44_100,
    );
    let prepared_again = prepared.clone();
    let (tx, rx) = event_queue();
    tx.send(EngineEvent::SetPreparedAudioConfig(prepared))
        .unwrap();
    tx.send(EngineEvent::SetPreparedAudioConfig(prepared_again))
        .unwrap();
    let mut source = EngineSource::new(rx, 44_100);
    let (allocation_count, deallocation_count) = allocations_and_deallocations(|| {
        for _ in 0..512 {
            let _ = source.next();
        }
    });
    assert_eq!(allocation_count, 0);
    assert_eq!(deallocation_count, 0);
}

#[test]
fn capability_audio_defaults_enable_high_headroom_mode() {
    assert_eq!(audio_block_frames(), DEFAULT_AUDIO_BLOCK_FRAMES);
    assert_eq!(synth_slot_worker_count(), Some(DEFAULT_SYNTH_SLOT_WORKERS));
}
