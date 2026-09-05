use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_instruments_config, FxBusConfig, FxBusSlotConfig,
    InstrumentSlotConfig, InstrumentsConfig, MixerConfig, RoutingTreePipelineProbe,
    SourceWorkerHealth, DEFAULT_PAN_POSITIONS,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn reactivates_bus_after_more_than_250ms_of_quiet_output() {
    let (tx, rx) = event_queue();
    let instruments = InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(realtime_engine::synth::InstrumentMixerConfig {
                route: "bus_1".into(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Kind("reverb".into())],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume_pct: 100.0,
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    };
    tx.send(EngineEvent::SetPreparedInstruments(
        prepare_instruments_config(instruments, 44_100),
    ))
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 5_000,
    })
    .unwrap();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers_for_benchmark(rx, 44_100, 128, None)
            .expect("routing-tree runtime");

    let mut warmup = Vec::new();
    for _ in 0..(128 * 2 * 6) {
        warmup.push(source.next().unwrap());
    }
    assert!(warmup.iter().any(|sample| sample.abs() > 0.0001));
    tx.send(EngineEvent::NoteOff {
        instrument_slot: 0,
        note: 60,
    })
    .unwrap();
    for _ in 0..(128 * 2 * 180) {
        let _ = source.next().unwrap();
    }

    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 67,
        velocity: 100,
        duration_ms: 5_000,
    })
    .unwrap();
    tx.send(EngineEvent::SetPreparedGlobalFxSlot {
        slot_index: 0,
        config: realtime_engine::synth::prepare_global_fx_slot(
            "compressor".into(),
            BTreeMap::new(),
        ),
    })
    .unwrap();
    let mut reactivated = Vec::new();
    for _ in 0..(128 * 2 * 6) {
        reactivated.push(source.next().unwrap());
    }
    assert!(reactivated.iter().any(|sample| sample.abs() > 0.0001));
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    let counters = source.persistent_output_counters();
    assert!(counters.rendered_quantums > 0);
    assert_eq!(counters.repeated_quantums, 0);
    assert_eq!(counters.dropped_quantums, 0);

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn recovers_routing_tree_with_current_global_controls_and_next_quantum_notes() {
    let (tx, rx) = event_queue();
    tx.send(EngineEvent::SetPreparedInstruments(
        prepare_instruments_config(
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
            44_100,
        ),
    ))
    .unwrap();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers_for_benchmark(rx, 44_100, 128, None)
            .expect("routing-tree runtime");
    let probe = Arc::new(RoutingTreePipelineProbe::default());
    source
        .worker_state
        .worker
        .as_mut()
        .expect("routing-tree worker")
        .runtime
        .set_routing_tree_probe_for_test(Arc::clone(&probe));
    {
        let runtime = &mut source
            .worker_state
            .worker
            .as_mut()
            .expect("routing-tree worker")
            .runtime;
        runtime.set_pause_for_parity_for_test(0, true);
        runtime.set_deadline_for_test(Duration::ZERO);
    }
    for _ in 0..(128 * 2) {
        let _ = source.next().unwrap();
    }
    assert!(source
        .worker_state
        .worker
        .as_ref()
        .expect("routing-tree worker")
        .runtime
        .wait_until_paused_for_test(0, Duration::from_secs(1)));
    for _ in 0..(128 * 2) {
        let _ = source.next().unwrap();
    }
    assert_eq!(
        source.source_worker_health(),
        SourceWorkerHealth::DeadlineMiss
    );

    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 5_000,
    })
    .unwrap();
    tx.send(EngineEvent::SetPreparedGlobalFxSlot {
        slot_index: 0,
        config: realtime_engine::synth::prepare_global_fx_slot(
            "compressor".into(),
            BTreeMap::new(),
        ),
    })
    .unwrap();
    {
        let runtime = &mut source
            .worker_state
            .worker
            .as_mut()
            .expect("routing-tree worker")
            .runtime;
        runtime.set_pause_for_parity_for_test(0, false);
        runtime.set_deadline_for_test(Duration::from_secs(1));
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    let mut samples = Vec::new();
    while probe.last_coordinator_base_sample_clock() != 256 {
        for _ in 0..(128 * 2) {
            samples.push(source.next().unwrap());
        }
        assert!(std::time::Instant::now() < deadline);
    }
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    assert_eq!(probe.last_dispatch_base_sample_clock(), 384);
    for _ in 0..(128 * 2 * 3) {
        samples.push(source.next().unwrap());
    }
    assert!(samples.iter().any(|sample| sample.abs() > 0.0001));

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}
