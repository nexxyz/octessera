use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_instruments_config, FxBusConfig,
    FxBusSlotConfig, InstrumentSlotConfig, InstrumentsConfig, MixerConfig, MomentaryFxTarget,
    SourceWorkerHealth, SynthEngine, DEFAULT_PAN_POSITIONS,
};
use std::collections::BTreeMap;

#[test]
fn constructor_runs_persistent_quantums() {
    let (_tx, rx) = event_queue();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, 128, None)
            .expect("routing-tree runtime");
    assert!(matches!(
        source.worker_state.mode,
        EngineSourceMode::RoutingTreePersistent
    ));
    assert_eq!(source.lookahead_frames(), 128);
    for _ in 0..(128 * 2 * 3) {
        assert_eq!(source.next(), Some(0.0));
    }
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn empty_control_steady_state_uses_no_control_gate() {
    let (_tx, rx) = event_queue();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, 128, None)
            .expect("routing-tree runtime");
    assert!(source
        .retired_backlog
        .as_mut()
        .expect("retired backlog")
        .enqueue(RetiredAudioItem {
            state: None,
            event: Some(EngineEvent::MomentaryFxStop {
                id: "empty-steady-state".into(),
            }),
            drop_probe: None,
        }));

    for _ in 0..(128 * 2 * 3) {
        assert_eq!(source.next(), Some(0.0));
    }

    assert_eq!(source.routing_tree_control_gate_calls, 0);
    assert_eq!(
        source
            .retired_backlog
            .as_ref()
            .expect("retired backlog")
            .len,
        0
    );
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn applies_controls_before_dispatching_next_quantum() {
    let (tx, rx) = event_queue();
    let instruments = InstrumentsConfig {
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
    };
    tx.send(EngineEvent::SetPreparedInstruments(
        realtime_engine::synth::prepare_instruments_config(instruments, 44_100),
    ))
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 500,
    })
    .unwrap();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, 128, None)
            .expect("routing-tree runtime");
    let mut samples = Vec::with_capacity(128 * 2 * 2);
    for _ in 0..(128 * 2 * 2) {
        samples.push(source.next().unwrap());
    }
    assert!(samples.iter().any(|sample| sample.abs() > 0.0001));
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn routing_tree_note_events_start_at_next_quantum() {
    let (tx, rx) = event_queue();
    let instruments = InstrumentsConfig {
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
    };
    tx.send(EngineEvent::SetPreparedInstruments(
        realtime_engine::synth::prepare_instruments_config(instruments, 44_100),
    ))
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 500,
    })
    .unwrap();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, 128, None)
            .expect("routing-tree runtime");

    let first: Vec<_> = (0..256).map(|_| source.next().unwrap()).collect();
    let second: Vec<_> = (0..256).map(|_| source.next().unwrap()).collect();
    assert!(first.iter().all(|sample| sample.to_bits() == 0));
    assert!(second.iter().any(|sample| sample.abs() > 0.0001));

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn routing_tree_preview_runs_through_source_control_gate() {
    let (tx, rx) = event_queue();
    tx.send(EngineEvent::SetPreparedAudioConfig(prepare_audio_config(
        direct_synth_instruments(),
        None,
        None,
        44_100,
    )))
    .unwrap();
    tx.send(EngineEvent::PreviewSample {
        instrument_slot: 0,
        buffer: realtime_engine::synth::SampleBuffer {
            samples: vec![0.5; 4096].into(),
            channels: 1,
            sample_rate: 44_100,
        },
        velocity: 100,
    })
    .unwrap();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, 128, None)
            .expect("routing-tree runtime");

    for _ in 0..(128 * 2 * 2) {
        let _ = source.next();
    }
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    assert_eq!(source.profile_snapshot().active_preview_sample_voices, 1);

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn routing_tree_local_momentary_fx_runs_through_source_control_gate() {
    let (tx, rx) = event_queue();
    tx.send(EngineEvent::SetPreparedAudioConfig(prepare_audio_config(
        direct_synth_instruments(),
        None,
        None,
        44_100,
    )))
    .unwrap();
    tx.send(EngineEvent::PreparedMomentaryFxStart(
        realtime_engine::synth::prepare_momentary_fx_start(
            "local".into(),
            "filter_sweep".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Instrument { index: 0 },
            44_100,
        )
        .expect("momentary FX"),
    ))
    .unwrap();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, 128, None)
            .expect("routing-tree runtime");

    for _ in 0..(128 * 2 * 2) {
        let _ = source.next();
    }
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    assert_eq!(source.profile_snapshot().active_momentary_fx, 1);

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn routing_tree_global_momentary_uses_ready_quantum_before_next_source_note() {
    let prepared =
        realtime_engine::synth::prepare_instruments_config(direct_synth_instruments(), 44_100);
    let (tx, rx) = event_queue();
    tx.send(EngineEvent::SetPreparedInstruments(prepared))
        .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 48,
        velocity: 100,
        duration_ms: 500,
    })
    .unwrap();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, 128, None)
            .expect("routing-tree runtime");

    let first: Vec<_> = (0..256).map(|_| source.next().unwrap()).collect();
    assert!(first.iter().all(|sample| sample.to_bits() == 0));

    tx.send(EngineEvent::PreparedMomentaryFxStart(
        realtime_engine::synth::prepare_momentary_fx_start(
            "global-freeze".into(),
            "freeze".into(),
            BTreeMap::from([("releaseMs".into(), serde_json::json!(1.0))]),
            MomentaryFxTarget::Global,
            44_100,
        )
        .expect("global momentary FX"),
    ))
    .unwrap();
    tx.send(EngineEvent::NoteOff {
        instrument_slot: 0,
        note: 48,
    })
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 72,
        velocity: 100,
        duration_ms: 500,
    })
    .unwrap();

    let ready_quantum: Vec<_> = (0..256).map(|_| source.next().unwrap()).collect();
    assert!(ready_quantum.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(source.profile_snapshot().active_synth_voices, 1);
    assert_eq!(source.profile_snapshot().active_momentary_fx, 1);

    let next_source_quantum: Vec<_> = (0..256).map(|_| source.next().unwrap()).collect();
    assert!(next_source_quantum
        .iter()
        .all(|sample| sample.to_bits() == 0));
    assert_eq!(source.profile_snapshot().active_synth_voices, 2);
    assert_eq!(source.profile_snapshot().active_momentary_fx, 1);

    tx.send(EngineEvent::MomentaryFxStop {
        id: "global-freeze".into(),
    })
    .unwrap();
    let _ = (0..256).map(|_| source.next().unwrap()).collect::<Vec<_>>();
    let after_stop: Vec<_> = (0..256).map(|_| source.next().unwrap()).collect();
    assert!(after_stop.iter().any(|sample| sample.abs() > 0.0001));
    assert_eq!(source.profile_snapshot().active_momentary_fx, 0);
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn routing_tree_profile_matches_inline_after_a_completed_quantum() {
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
    let prepared = prepare_instruments_config(instruments, 44_100);
    let (tx, rx) = event_queue();
    tx.send(EngineEvent::SetPreparedInstruments(prepared.clone()))
        .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 500,
    })
    .unwrap();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, 128, None)
            .expect("routing-tree runtime");
    let _ = (0..512).map(|_| source.next().unwrap()).collect::<Vec<_>>();

    let mut inline = SynthEngine::new(44_100);
    drop(inline.apply_prepared_instruments_config(prepared));
    inline.note_on(0, 60, 100, 500);
    assert_eq!(source.profile_snapshot(), inline.profile_snapshot());

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

fn direct_synth_instruments() -> InstrumentsConfig {
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

#[test]
fn processes_bus_owned_by_worker() {
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
        mixer: Some(realtime_engine::synth::MixerConfig {
            buses: vec![realtime_engine::synth::FxBusConfig::default()],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    };
    tx.send(EngineEvent::SetPreparedInstruments(
        realtime_engine::synth::prepare_instruments_config(instruments, 44_100),
    ))
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 500,
    })
    .unwrap();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, 128, None)
            .expect("routing-tree runtime");
    let mut samples = Vec::with_capacity(128 * 2 * 2);
    for _ in 0..(128 * 2 * 2) {
        samples.push(source.next().unwrap());
    }
    assert!(samples.iter().any(|sample| sample.abs() > 0.0001));
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}
