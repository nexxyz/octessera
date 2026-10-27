use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, InstrumentSlotConfig, InstrumentsConfig,
    SampleBankConfig, SampleBuffer, SampleSlotConfig, DEFAULT_PAN_POSITIONS,
};
use std::thread;
use std::time::Duration;

const RATE: u32 = 44_100;

pub(super) fn persistent_source(
    block_frames: usize,
) -> (
    EngineEventSender,
    EngineSource,
    EngineSourceWorkerShutdownOwner,
) {
    let (tx, rx) = event_queue();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, RATE, block_frames, None).unwrap();
    source
        .worker_state
        .worker
        .as_mut()
        .expect("persistent worker")
        .runtime
        .set_deadline_for_test(Duration::from_secs(1));
    (tx, source, shutdown)
}

fn inline_source(block_frames: usize) -> (EngineEventSender, EngineSource) {
    let (tx, rx) = event_queue();
    (tx, EngineSource::with_block_frames(rx, RATE, block_frames))
}

fn block_bits(source: &mut EngineSource, frames: usize) -> Vec<u32> {
    (0..frames * 2)
        .map(|_| source.next().unwrap().to_bits())
        .collect()
}

fn send_to_both(
    inline_tx: &EngineEventSender,
    persistent_tx: &EngineEventSender,
    event: EngineEvent,
) {
    inline_tx.send(event.clone()).unwrap();
    persistent_tx.send(event).unwrap();
}

fn sample_bank(value: f32) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![value, -value, value * 0.5, -value * 0.25].into(),
            channels: 1,
            sample_rate: RATE,
        }),
    };
    bank
}

fn mixed_config(value: f32) -> realtime_engine::synth::PreparedAudioConfig {
    prepare_audio_config(
        InstrumentsConfig {
            instruments: vec![
                InstrumentSlotConfig {
                    fm: None,
                    pluck: None,
                    drum: None,
                    kind: "synth".into(),
                    synth: default_synth_config(),
                    mixer: None,
                },
                InstrumentSlotConfig {
                    fm: None,
                    pluck: None,
                    drum: None,
                    kind: "sampler".into(),
                    synth: default_synth_config(),
                    mixer: None,
                },
            ],
            mixer: None,
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        },
        Some(vec![sample_bank(value)]),
        None,
        RATE,
    )
}

#[test]
fn routing_tree_factory_prewarms_workers_with_one_quantum_of_lookahead() {
    let (tx, source, shutdown) = persistent_source(128);
    assert_eq!(source.block_frames(), 128);
    assert_eq!(source.lookahead_frames(), 128);
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    drop(tx);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);

    let (_tx, inline) = inline_source(128);
    assert_eq!(inline.source_worker_health(), SourceWorkerHealth::Disabled);
    assert_eq!(inline.lookahead_frames(), 0);
}

#[test]
fn persistent_refill_has_no_callback_memory_activity() {
    let (inline_tx, mut inline) = inline_source(128);
    let (persistent_tx, mut persistent, shutdown) = persistent_source(128);
    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::SetPreparedAudioConfig {
            generation: 0,
            config: mixed_config(0.8),
        },
    );
    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 100,
            duration_ms: 10_000,
        },
    );
    let _ = block_bits(&mut inline, 128);
    let _ = block_bits(&mut persistent, 128);

    let total = 128 * 2 * 8;
    let mut inline_bits = Vec::with_capacity(total);
    let mut persistent_bits = Vec::with_capacity(total);
    let (inline_allocations, inline_deallocations) = allocations_and_deallocations(|| {
        for _ in 0..8 {
            for _ in 0..128 * 2 {
                inline_bits.push(inline.next().unwrap().to_bits());
            }
        }
    });
    let (persistent_allocations, persistent_deallocations) = allocations_and_deallocations(|| {
        for _ in 0..8 {
            for _ in 0..128 * 2 {
                persistent_bits.push(persistent.next().unwrap().to_bits());
            }
        }
    });
    assert_eq!((inline_allocations, inline_deallocations), (0, 0));
    assert_eq!((persistent_allocations, persistent_deallocations), (0, 0));

    drop(inline);
    drop(persistent);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn persistent_idle_refill_stays_silent_through_worker_path() {
    let (_tx, mut source, shutdown) = persistent_source(128);
    let (allocations, deallocations) = allocations_and_deallocations(|| {
        for _ in 0..8 * 128 * 2 {
            assert_eq!(source.next(), Some(0.0));
        }
    });
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn source_drop_retires_workers_and_shutdown_owner_joins_off_callback() {
    let (_tx, source, shutdown) = persistent_source(128);
    let owner_thread = thread::spawn(move || shutdown.shutdown().joined_workers);
    thread::sleep(Duration::from_millis(5));
    assert!(!owner_thread.is_finished());
    drop(source);
    assert_eq!(owner_thread.join().unwrap(), 2);
}

#[test]
fn shutdown_owner_drop_returns_without_waiting_for_source_on_another_thread() {
    let (_tx, source, shutdown) = persistent_source(128);
    let owner_thread = thread::spawn(move || drop(shutdown));
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !owner_thread.is_finished() && std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    assert!(owner_thread.is_finished());
    drop(source);
    assert!(owner_thread.join().is_ok());
}

#[test]
fn shutdown_owner_handles_source_dropped_on_another_thread() {
    let (_tx, source, shutdown) = persistent_source(128);
    let source_thread = thread::spawn(move || drop(source));
    source_thread.join().unwrap();
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}
