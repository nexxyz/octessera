use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_momentary_fx_start, InstrumentSlotConfig,
    InstrumentsConfig, MomentaryFxTarget, SampleBankConfig, SampleBuffer, SampleSlotConfig,
    DEFAULT_PAN_POSITIONS,
};
use std::collections::BTreeMap;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const RATE: u32 = 44_100;
const SUPPORTED_BLOCKS: [usize; 3] = [64, 128, 256];

impl EngineSource {
    pub(crate) fn retire_workers_for_test(&mut self) {
        let _ = self.retire_workers();
    }
}

pub(super) fn persistent_source(
    block_frames: usize,
) -> (
    EngineEventSender,
    EngineSource,
    EngineSourceWorkerShutdownOwner,
) {
    let (tx, rx) = event_queue();
    let (mut source, shutdown) =
        EngineSource::with_persistent_workers(rx, RATE, block_frames, None).unwrap();
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

#[test]
fn persistent_source_publishes_status_after_paired_worker_completion() {
    let (tx, rx) = event_queue();
    let (load_tx, load_rx) = audio_load_status_channel();
    let (mut source, shutdown) =
        EngineSource::with_persistent_workers(rx, RATE, 128, Some(load_tx)).unwrap();
    source.last_load_report = Instant::now() - LOAD_REPORT_INTERVAL;
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 1_000,
    })
    .unwrap();

    for _ in 0..256 {
        source.next();
    }

    let status = load_rx.try_recv().expect("paired completion status");
    assert!(status
        .worker_utilization
        .is_some_and(|value| value.is_finite()));
    assert!(!status.missed_quantum_flash);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
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
                    kind: "synth".into(),
                    synth: default_synth_config(),
                    mixer: None,
                },
                InstrumentSlotConfig {
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

fn assert_block_parity(inline: &mut EngineSource, persistent: &mut EngineSource, frames: usize) {
    assert_eq!(block_bits(inline, frames), block_bits(persistent, frames));
}

fn run_mixed_flow(frames: usize) {
    let (inline_tx, mut inline) = inline_source(frames);
    let (persistent_tx, mut persistent, shutdown) = persistent_source(frames);
    assert_eq!(inline.source_worker_health(), SourceWorkerHealth::Disabled);
    assert_eq!(
        persistent.source_worker_health(),
        SourceWorkerHealth::Healthy
    );

    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::SetPreparedAudioConfig(mixed_config(0.8)),
    );
    assert_block_parity(&mut inline, &mut persistent, frames);

    let (inline_report_tx, inline_report_rx) = mpsc::sync_channel(1);
    let (persistent_report_tx, persistent_report_rx) = mpsc::sync_channel(1);
    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 112,
            duration_ms: 2_000,
        },
    );
    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::NoteOn {
            instrument_slot: 1,
            note: 36,
            velocity: 100,
            duration_ms: 2_000,
        },
    );
    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::PreviewSample {
            instrument_slot: 1,
            buffer: sample_bank(0.8).slots[0].buffer.clone().unwrap(),
            velocity: 100,
        },
    );
    inline_tx
        .send(EngineEvent::ProbeMark {
            sent_at: Instant::now(),
            report_tx: inline_report_tx,
        })
        .unwrap();
    persistent_tx
        .send(EngineEvent::ProbeMark {
            sent_at: Instant::now(),
            report_tx: persistent_report_tx,
        })
        .unwrap();
    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::PreparedMomentaryFxStart(
            prepare_momentary_fx_start(
                "test".into(),
                "stutter".into(),
                BTreeMap::new(),
                MomentaryFxTarget::Global,
                RATE,
            )
            .unwrap(),
        ),
    );
    assert_block_parity(&mut inline, &mut persistent, frames);
    assert!(inline_report_rx
        .recv_timeout(Duration::from_secs(1))
        .is_ok());
    assert!(persistent_report_rx
        .recv_timeout(Duration::from_secs(1))
        .is_ok());

    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::SetSynthParam {
            instrument_slot: 0,
            path: "synth.amp.gainPct".into(),
            value: 72.0,
        },
    );
    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::SetSampleBankParam {
            instrument_slot: 1,
            path: "sample.amp.gainPct".into(),
            value: 65.0,
        },
    );
    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::MomentaryFxUpdate {
            id: "test".into(),
            params: BTreeMap::new(),
        },
    );
    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::SetPreparedSampleBank {
            instrument_slot: 1,
            bank: sample_bank(0.4),
        },
    );
    assert_block_parity(&mut inline, &mut persistent, frames);

    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::NoteOff {
            instrument_slot: 0,
            note: 60,
        },
    );
    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::MomentaryFxStop { id: "test".into() },
    );
    send_to_both(&inline_tx, &persistent_tx, EngineEvent::AllNotesOff);
    for _ in 0..4 {
        assert_block_parity(&mut inline, &mut persistent, frames);
    }
    assert_eq!(
        persistent.source_worker_health(),
        SourceWorkerHealth::Healthy
    );

    drop(inline);
    drop(persistent);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn persistent_factory_prewarm_workers_and_keeps_inline_disabled() {
    let (tx, source, shutdown) = persistent_source(128);
    assert_eq!(source.block_frames(), 128);
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    drop(tx);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);

    let (_tx, inline) = inline_source(128);
    assert_eq!(inline.source_worker_health(), SourceWorkerHealth::Disabled);
}

#[test]
fn persistent_mixed_flow_matches_inline_for_supported_blocks() {
    for frames in SUPPORTED_BLOCKS {
        run_mixed_flow(frames);
    }
}

#[test]
fn persistent_refill_has_no_callback_memory_activity() {
    let (inline_tx, mut inline) = inline_source(128);
    let (persistent_tx, mut persistent, shutdown) = persistent_source(128);
    send_to_both(
        &inline_tx,
        &persistent_tx,
        EngineEvent::SetPreparedAudioConfig(mixed_config(0.8)),
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
    assert_eq!(inline_bits, persistent_bits);

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
fn source_worker_retirement_is_allocation_free_and_single_use() {
    let (_tx, mut source, shutdown) = persistent_source(128);
    let (allocations, deallocations) = allocations_and_deallocations(|| {
        source.retire_workers_for_test();
    });
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Disabled);
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
    thread::sleep(Duration::from_millis(5));
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
