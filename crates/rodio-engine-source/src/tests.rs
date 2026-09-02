use super::*;
use crossbeam_channel::bounded;
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_momentary_fx_start, InstrumentSlotConfig,
    InstrumentsConfig, MomentaryFxTarget, SampleBankConfig, SampleBuffer,
    DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES, DEFAULT_PAN_POSITIONS, MAX_CONTROL_EVENTS_PER_CALLBACK,
};
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::mpsc;
use std::time::Instant;

impl EngineSource {
    fn with_test_retirement_receiver(
        control_rx: EngineEventReceiver,
        sample_rate: u32,
    ) -> (Self, crossbeam_channel::Receiver<RetiredAudioItem>) {
        let (retired_tx, retired_rx) = bounded(RETIREMENT_QUEUE_CAPACITY);
        let (shutdown_tx, shutdown_rx) = bounded::<SourceShutdownEnvelope>(1);
        std::thread::spawn(move || {
            if let Ok(envelope) = shutdown_rx.recv() {
                envelope.backlog.drain();
            }
        });
        (
            Self::with_retirement_sender(
                control_rx,
                sample_rate,
                audio_render_quantum_frames(DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES),
                None,
                retired_tx,
                shutdown_tx,
            ),
            retired_rx,
        )
    }
}

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
        count_deallocation();
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

fn count_deallocation() {
    COUNT_ALLOCATIONS.with(|enabled| {
        if enabled.get() {
            DEALLOCATIONS.with(|deallocations| deallocations.set(deallocations.get() + 1));
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
fn realloc_activity_counts_allocation_and_deallocation_effects() {
    let (allocations, deallocations) = allocations_and_deallocations(|| {
        let mut values = Vec::<u8>::with_capacity(1);
        values.reserve_exact(64);
        drop(values);
    });

    assert!(allocations >= 2);
    assert!(deallocations >= 1);
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
    for note in 0..(MAX_CONTROL_EVENTS_PER_CALLBACK + 11) {
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
    assert_eq!(
        drained.control_events,
        MAX_CONTROL_EVENTS_PER_CALLBACK as u64
    );
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
fn mixed_lifecycle_callback_path_does_not_allocate_or_drop_heap_state() {
    let (tx, rx) = event_queue();
    let mut source = EngineSource::new(rx, 44_100);
    for _ in 0..512 {
        let _ = source.next();
    }
    let config = prepare_audio_config(
        InstrumentsConfig {
            instruments: vec![InstrumentSlotConfig {
                kind: "synth".into(),
                synth: default_synth_config(),
                mixer: None,
            }],
            mixer: None,
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        },
        Some(vec![SampleBankConfig::default()]),
        None,
        44_100,
    );
    let replacement = config.clone();
    let preview = SampleBuffer {
        samples: vec![0.25; 256].into_boxed_slice().into(),
        channels: 1,
        sample_rate: 44_100,
    };
    let momentary_start = prepare_momentary_fx_start(
        "filter".into(),
        "filter_sweep".into(),
        BTreeMap::from([
            ("sweepInMs".into(), json!(1.0)),
            ("sweepOutMs".into(), json!(1.0)),
        ]),
        MomentaryFxTarget::Global,
        44_100,
    )
    .unwrap();
    let momentary_update = BTreeMap::from([("sweepOutMs".into(), json!(1.0))]);
    let (report_tx, report_rx) = mpsc::sync_channel(1);
    let report_waiter = std::thread::spawn(move || report_rx.recv().unwrap());
    let probe_event = EngineEvent::ProbeMark {
        sent_at: Instant::now(),
        report_tx,
    };
    tx.send(EngineEvent::SetPreparedAudioConfig(config))
        .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 10_000,
    })
    .unwrap();
    tx.send(EngineEvent::PreviewSample {
        instrument_slot: 0,
        buffer: preview,
        velocity: 100,
    })
    .unwrap();
    tx.send(EngineEvent::PreparedMomentaryFxStart(momentary_start))
        .unwrap();
    tx.send(EngineEvent::MomentaryFxUpdate {
        id: "filter".into(),
        params: momentary_update,
    })
    .unwrap();
    tx.send(EngineEvent::SetSynthParam {
        instrument_slot: 0,
        path: "synth.amp.gainPct".into(),
        value: 90.0,
    })
    .unwrap();
    tx.send(EngineEvent::SetSampleBankParam {
        instrument_slot: 0,
        path: "sample.amp.gainPct".into(),
        value: 90.0,
    })
    .unwrap();
    tx.send(probe_event).unwrap();
    tx.send(EngineEvent::SetPreparedAudioConfig(replacement))
        .unwrap();
    tx.send(EngineEvent::MomentaryFxStop {
        id: "filter".into(),
    })
    .unwrap();
    tx.send(EngineEvent::AllNotesOff).unwrap();

    let (allocation_count, deallocation_count) = allocations_and_deallocations(|| {
        for _ in 0..1024 {
            let _ = source.next();
        }
    });

    assert_eq!(allocation_count, 0);
    assert_eq!(deallocation_count, 0);
    let _ = report_waiter.join().unwrap();
}

#[path = "retirement_tests.rs"]
mod retirement_tests;

#[path = "shutdown_handoff_tests.rs"]
mod shutdown_handoff_tests;

#[path = "retirement_storage_tests.rs"]
mod retirement_storage_tests;

#[path = "persistent_tests.rs"]
mod persistent_tests;

#[path = "persistent_profile_tests.rs"]
mod persistent_profile_tests;

#[path = "persistent_terminal_tests.rs"]
mod persistent_terminal_tests;

#[test]
fn explicit_profile_block_sizes_reach_source_configuration() {
    for block_frames in [64, 128, 256] {
        let (_tx, rx) = event_queue();
        let source = EngineSource::with_block_frames(rx, 44_100, block_frames);

        assert_eq!(source.block_frames(), block_frames);
    }
    let (_tx, rx) = event_queue();
    let source = EngineSource::with_block_frames(rx, 44_100, 1);
    assert_eq!(source.block_frames(), 32);
}

#[test]
fn default_and_explicit_block_apis_use_inline_source_path() {
    let (default_tx, default_rx) = event_queue();
    let (explicit_tx, explicit_rx) = event_queue();
    let note_on = EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 1_000,
    };
    default_tx.send(note_on.clone()).unwrap();
    explicit_tx.send(note_on).unwrap();
    let mut default_source = EngineSource::new(default_rx, 44_100);
    let mut explicit_source =
        EngineSource::with_block_frames(explicit_rx, 44_100, DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES);
    assert_eq!(
        default_source.block_frames(),
        DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES
    );
    assert_eq!(
        explicit_source.block_frames(),
        DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES
    );

    for _ in 0..256 {
        assert_eq!(
            default_source.next().unwrap().to_bits(),
            explicit_source.next().unwrap().to_bits()
        );
    }
}

#[test]
fn explicit_block_size_respects_render_quantum_override_parser() {
    assert_eq!(resolve_audio_render_quantum_frames(Some("128"), 64), 128);
    assert_eq!(resolve_audio_render_quantum_frames(Some("invalid"), 64), 64);
    assert_eq!(resolve_audio_render_quantum_frames(Some("1"), 64), 32);
}

#[test]
fn benchmark_persistent_constructor_uses_exact_requested_frames_without_env_override() {
    if std::env::var_os("OCTESSERA_BENCHMARK_QUANTUM_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::benchmark_persistent_constructor_uses_exact_requested_frames_without_env_override",
                "--nocapture",
            ])
            .env("OCTESSERA_BENCHMARK_QUANTUM_CHILD", "1")
            .env("OCTESSERA_AUDIO_RENDER_QUANTUM_FRAMES", "2048")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    for block_frames in [64, 128, 256, 512, 1024, 2048] {
        let (_tx, rx) = event_queue();
        let (source, shutdown) =
            EngineSource::with_persistent_workers_for_benchmark(rx, 44_100, block_frames, None)
                .unwrap();
        assert_eq!(source.block_frames(), block_frames);
        drop(source);
        assert_eq!(shutdown.shutdown().joined_workers, 2);
    }
}

#[test]
fn benchmark_persistent_constructor_rejects_invalid_frames_before_setup() {
    for block_frames in [31, 2049] {
        let reaper_spawn_failure = source_worker_reaper::fail_next_reaper_spawn_for_test();
        let (_tx, rx) = event_queue();
        let result =
            EngineSource::with_persistent_workers_for_benchmark(rx, 44_100, block_frames, None);
        assert!(matches!(
            result,
            Err(SourceWorkerSetupError::InvalidBlockFrames {
                requested,
                min: MIN_BLOCK_FRAMES,
                max: MAX_BLOCK_FRAMES,
            }) if requested == block_frames
        ));
        assert_eq!(reaper_spawn_failure.attempts_for_test(), 0);
    }
}

#[cfg(feature = "source-worker-benchmark-timing")]
#[path = "persistent_timing_tests.rs"]
mod persistent_timing_tests;
