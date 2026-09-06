use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_fx_bus_slot, prepare_instruments_config,
    AudioLoadStatus, FxBusConfig, FxBusSlotConfig, InstrumentSlotConfig, InstrumentsConfig,
    MasterFxConfig, MixerConfig, MomentaryFxTarget, SampleBankConfig, SampleBuffer,
    SampleSlotConfig, SourceWorkerHealth, SynthProfileSnapshot, DEFAULT_PAN_POSITIONS,
};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const BLOCK_SAMPLES: usize = 256;
const RATE: u32 = 44_100;

#[test]
fn routing_tree_status_waits_for_fresh_profile_after_controls() {
    let (tx, rx) = event_queue();
    let (load_tx, load_rx) = audio_load_status_channel();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, 128, Some(load_tx))
            .expect("routing-tree runtime");
    let expired = expired_report();
    source.last_load_report = expired;
    tx.send(EngineEvent::SetPreparedAudioConfig(routing_status_config()))
        .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 10_000,
    })
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 1,
        note: 36,
        velocity: 100,
        duration_ms: 10_000,
    })
    .unwrap();
    tx.send(EngineEvent::PreparedMomentaryFxStart(
        realtime_engine::synth::prepare_momentary_fx_start(
            "global-freeze".into(),
            "freeze".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Global,
            44_100,
        )
        .expect("global momentary FX"),
    ))
    .unwrap();

    for _ in 0..256 {
        source.next();
    }
    assert!(load_rx.try_recv().is_err());
    assert!(source.pending_load_status_after_fresh);
    assert_eq!(source.last_load_report, expired);
    let first_profile = source.profile_snapshot();
    assert_eq!(first_profile.active_synth_voices, 0);
    assert_eq!(first_profile.active_sample_voices, 0);
    assert_eq!(first_profile.active_bus_fx_slots, 0);
    assert_eq!(first_profile.active_global_fx_slots, 1);
    assert_eq!(first_profile.active_momentary_fx, 1);

    for _ in 0..256 {
        source.next();
    }
    let status = load_rx.try_recv().expect("fresh routing status");
    assert_eq!(status.control_events, 4);
    assert_eq!(status.config_events, 2);
    assert!(!source.pending_load_status_after_fresh);
    assert_eq!(
        source.profile_snapshot(),
        SynthProfileSnapshot {
            active_synth_voices: 1,
            active_sample_voices: 1,
            active_preview_sample_voices: 0,
            active_momentary_fx: 1,
            active_bus_fx_slots: 1,
            active_global_fx_slots: 1,
            cumulative_voice_steals: 0,
            cumulative_voice_admission_drops: 0,
        }
    );

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn routing_tree_status_survives_deadline_miss_and_publishes_after_recovery() {
    let (tx, mut source, shutdown, _load_tx, load_rx) = routing_source();
    let expired = expired_report();
    source.last_load_report = expired;
    tx.send(note_on(60)).unwrap();
    force_deadline_miss(&mut source);
    assert!(load_rx.try_recv().is_err());
    assert!(source.pending_load_status_after_fresh);
    assert_eq!(source.last_load_report, expired);

    recover_routing_source(&mut source);
    assert!(!source.pending_load_status_after_fresh);
    assert!(source.last_load_report > expired);
    let statuses: Vec<_> = std::iter::from_fn(|| load_rx.try_recv().ok()).collect();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].deadline_misses, 1);
    assert_eq!(statuses[0].deadline_recoveries, 1);

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn routing_tree_status_does_not_publish_deferred_evidence_after_fatal() {
    let (tx, mut source, shutdown, _load_tx, load_rx) = routing_source();
    force_deadline_miss(&mut source);
    recover_routing_source(&mut source);
    while load_rx.try_recv().is_ok() {}
    assert!(!source.pending_load_status_after_fresh);

    let expired = expired_report();
    source.last_load_report = expired;
    tx.send(note_on(62)).unwrap();
    tx.send(EngineEvent::SetPreparedFxBusSlot {
        bus_index: 0,
        slot_index: 0,
        config: prepare_fx_bus_slot("reverb".into(), BTreeMap::new(), RATE),
    })
    .unwrap();
    block(&mut source);

    assert_eq!(
        source.source_worker_health(),
        SourceWorkerHealth::CompletionFailed
    );
    assert!(source.pending_load_status_after_fresh);
    assert_eq!(source.last_load_report, expired);
    assert!(load_rx.try_recv().is_err());

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn inline_status_attempt_advances_cadence_when_queue_is_full() {
    let (_tx, rx) = event_queue();
    let (load_tx, load_rx) = audio_load_status_channel();
    for _ in 0..super::telemetry::TELEMETRY_QUEUE_CAPACITY {
        assert!(load_tx.try_send(empty_status()));
    }
    let mut source = EngineSource::with_load_status_tx(rx, RATE, Some(load_tx));
    let expired = expired_report();
    source.last_load_report = expired;

    source.next();

    assert!(source.last_load_report > expired);
    drop(source);
    drop(load_rx);
}

#[test]
fn inline_status_attempt_advances_cadence_when_queue_is_disconnected() {
    let (_tx, rx) = event_queue();
    let (load_tx, load_rx) = audio_load_status_channel();
    drop(load_rx);
    let mut source = EngineSource::with_load_status_tx(rx, RATE, Some(load_tx));
    let expired = expired_report();
    source.last_load_report = expired;

    source.next();

    assert!(source.last_load_report > expired);
    drop(source);
}

#[test]
fn persistent_status_attempt_advances_cadence_when_queue_is_full() {
    let (_tx, rx) = event_queue();
    let (load_tx, load_rx) = audio_load_status_channel();
    for _ in 0..super::telemetry::TELEMETRY_QUEUE_CAPACITY {
        assert!(load_tx.try_send(empty_status()));
    }
    let (mut source, shutdown) =
        EngineSource::with_persistent_workers_for_benchmark(rx, RATE, 128, Some(load_tx))
            .expect("persistent runtime");
    let expired = expired_report();
    source.last_load_report = expired;

    block(&mut source);

    assert!(source.last_load_report > expired);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
    drop(load_rx);
}

#[test]
fn persistent_status_attempt_advances_cadence_when_queue_is_disconnected() {
    let (_tx, rx) = event_queue();
    let (load_tx, load_rx) = audio_load_status_channel();
    drop(load_rx);
    let (mut source, shutdown) =
        EngineSource::with_persistent_workers_for_benchmark(rx, RATE, 128, Some(load_tx))
            .expect("persistent runtime");
    let expired = expired_report();
    source.last_load_report = expired;

    block(&mut source);

    assert!(source.last_load_report > expired);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn routing_tree_status_ack_retains_pending_for_controls_on_fresh_publication() {
    let (tx, mut source, shutdown, _load_tx, load_rx) = routing_source();
    queue_pending_control(&tx, &mut source, 60);
    tx.send(note_on(62)).unwrap();
    block(&mut source);

    let status = load_rx.try_recv().expect("old routing status");
    assert_eq!(status.control_events, 3);
    assert!(source.pending_load_status_after_fresh);
    assert_eq!(source.profile_snapshot().active_synth_voices, 1);

    block(&mut source);
    let status = load_rx.try_recv().expect("new routing status");
    assert_eq!(status.control_events, 3);
    assert!(!source.pending_load_status_after_fresh);
    assert_eq!(source.profile_snapshot().active_synth_voices, 2);

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn routing_tree_status_retries_one_pending_update_after_full_queue() {
    let (tx, mut source, shutdown, load_tx, load_rx) = routing_source();
    for _ in 0..super::telemetry::TELEMETRY_QUEUE_CAPACITY {
        assert!(load_tx.try_send(empty_status()));
    }
    tx.send(note_on(60)).unwrap();

    block(&mut source);
    block(&mut source);
    assert!(source.pending_load_status_after_fresh);

    while load_rx.try_recv().is_ok() {}
    block(&mut source);
    assert!(!source.pending_load_status_after_fresh);
    assert!(load_rx.try_recv().is_ok());

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn routing_tree_status_publication_is_allocation_free() {
    let (tx, mut source, shutdown, _load_tx, load_rx) = routing_source();
    tx.send(note_on(60)).unwrap();
    let (allocations, deallocations) = super::allocations_and_deallocations(|| {
        block(&mut source);
        block(&mut source);
    });

    assert_eq!((allocations, deallocations), (0, 0));
    assert!(load_rx.try_recv().is_ok());
    assert!(!source.pending_load_status_after_fresh);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

fn routing_source() -> (
    EngineEventSender,
    EngineSource,
    EngineSourceWorkerShutdownOwner,
    AudioLoadStatusSender,
    AudioLoadStatusReceiver,
) {
    let (tx, rx) = event_queue();
    let (load_tx, load_rx) = audio_load_status_channel();
    tx.send(EngineEvent::SetPreparedInstruments(
        prepare_instruments_config(initial_routing_config(), RATE),
    ))
    .unwrap();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, RATE, 128, Some(load_tx.clone()))
            .expect("routing-tree runtime");
    source.last_load_report = Instant::now();
    block(&mut source);
    block(&mut source);
    while load_rx.try_recv().is_ok() {}
    assert!(!source.pending_load_status_after_fresh);
    source.last_load_report = Instant::now();
    (tx, source, shutdown, load_tx, load_rx)
}

fn initial_routing_config() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(realtime_engine::synth::InstrumentMixerConfig {
                route: "direct".into(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn queue_pending_control(tx: &EngineEventSender, source: &mut EngineSource, note: u8) {
    tx.send(note_on(note)).unwrap();
    block(source);
    assert!(source.pending_load_status_after_fresh);
}

fn note_on(note: u8) -> EngineEvent {
    EngineEvent::NoteOn {
        instrument_slot: 0,
        note,
        velocity: 100,
        duration_ms: 10_000,
    }
}

fn block(source: &mut EngineSource) {
    for _ in 0..BLOCK_SAMPLES {
        let _ = source.next();
    }
}

fn runtime(source: &mut EngineSource) -> &mut realtime_engine::synth::SourceWorkerRuntime {
    &mut source
        .worker_state
        .worker
        .as_mut()
        .expect("routing-tree worker")
        .runtime
}

fn force_deadline_miss(source: &mut EngineSource) {
    let completion_deadline = Instant::now() + Duration::from_secs(1);
    while !runtime(source)
        .completion_states_for_test()
        .iter()
        .all(|is_complete| *is_complete)
    {
        assert!(
            Instant::now() < completion_deadline,
            "routing workers did not complete"
        );
        std::thread::yield_now();
    }
    {
        let runtime = runtime(source);
        runtime.set_pause_for_parity_for_test(0, true);
        runtime.set_pause_for_parity_for_test(1, true);
        runtime.set_deadline_for_test(Duration::from_secs(1));
    }
    block(source);
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut paused = [false; 2];
    while !paused.iter().all(|is_paused| *is_paused) {
        let runtime = runtime(source);
        for (parity, is_paused) in paused.iter_mut().enumerate() {
            if !*is_paused {
                *is_paused = runtime.wait_until_paused_for_test(parity, Duration::from_millis(10));
            }
        }
        assert!(Instant::now() < deadline, "routing workers did not pause");
        if !paused.iter().all(|is_paused| *is_paused) {
            block(source);
        }
    }
    runtime(source).set_deadline_for_test(Duration::ZERO);
    block(source);
    assert_eq!(
        source.source_worker_health(),
        SourceWorkerHealth::DeadlineMiss
    );
}

fn expired_report() -> Instant {
    Instant::now() - LOAD_REPORT_INTERVAL - Duration::from_secs(1)
}

fn recover_routing_source(source: &mut EngineSource) {
    {
        let runtime = runtime(source);
        runtime.set_pause_for_parity_for_test(0, false);
        runtime.set_pause_for_parity_for_test(1, false);
        runtime.set_deadline_for_test(Duration::from_secs(1));
    }
    let deadline = Instant::now() + Duration::from_secs(1);
    while source.source_worker_health() != SourceWorkerHealth::Healthy
        || source.pending_load_status_after_fresh
    {
        block(source);
        assert!(Instant::now() < deadline, "routing source did not recover");
        std::thread::yield_now();
    }
}

fn empty_status() -> AudioLoadStatus {
    AudioLoadStatus {
        ratio: 0.0,
        voice_steal: false,
        worker_utilization: None,
        high_cpu_steady: false,
        missed_quantum_flash: false,
        block_ratio_p95: 0.0,
        block_ratio_max: 0.0,
        blocks: 0,
        control_events: 0,
        config_events: 0,
        rendered_quantums: 0,
        repeated_quantums: 0,
        dropped_quantums: 0,
        deadline_misses: 0,
        deadline_recoveries: 0,
    }
}

fn routing_status_config() -> realtime_engine::synth::PreparedAudioConfig {
    let instruments = InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "synth".into(),
                synth: default_synth_config(),
                mixer: Some(realtime_engine::synth::InstrumentMixerConfig {
                    route: "bus_1".into(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            },
            InstrumentSlotConfig {
                kind: "sampler".into(),
                synth: default_synth_config(),
                mixer: Some(realtime_engine::synth::InstrumentMixerConfig {
                    route: "direct".into(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            },
        ],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Kind("reverb".into())],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume_pct: 100.0,
            }],
            master: Some(MasterFxConfig {
                slots: vec![FxBusSlotConfig::Kind("eq".into())],
            }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    };
    let mut sample_bank = SampleBankConfig::default();
    sample_bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![0.5; 44_100].into(),
            channels: 1,
            sample_rate: 44_100,
        }),
    };
    prepare_audio_config(
        instruments,
        Some(vec![SampleBankConfig::default(), sample_bank]),
        None,
        44_100,
    )
}
