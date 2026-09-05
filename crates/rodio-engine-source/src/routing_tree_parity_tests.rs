use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_instruments_config, FxBusConfig, FxBusSlotConfig,
    InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig, MixerConfig,
    SourceWorkerHealth, DEFAULT_PAN_POSITIONS,
};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn threaded_routing_tree_matches_inline_for_duck_spread_tail_and_profile() {
    let prepared = prepare_instruments_config(threaded_parity_config(), 44_100);
    let (inline_tx, inline_rx) = event_queue();
    let (routing_tx, routing_rx) = event_queue();
    for tx in [&inline_tx, &routing_tx] {
        tx.send(EngineEvent::SetPreparedInstruments(prepared.clone()))
            .unwrap();
        for (slot, note) in [(0, 36), (1, 60), (2, 67)] {
            tx.send(EngineEvent::NoteOn {
                instrument_slot: slot,
                note,
                velocity: 100,
                duration_ms: 5_000,
            })
            .unwrap();
        }
    }
    let mut inline = EngineSource::new(inline_rx, 44_100);
    let (mut routing, routing_shutdown) =
        EngineSource::with_routing_tree_persistent_workers_for_benchmark(
            routing_rx, 44_100, 128, None,
        )
        .expect("routing-tree runtime");

    let first_inline = next_block(&mut inline, 128);
    let first_routing = next_block(&mut routing, 128);
    assert!(first_inline.iter().any(|sample| sample.abs() > 0.0001));
    assert!(first_routing.iter().all(|sample| sample.to_bits() == 0));
    let mut previous_inline = first_inline;
    for _ in 0..5 {
        let current_routing = next_block(&mut routing, 128);
        assert_eq!(current_routing, previous_inline);
        assert_eq!(routing.profile_snapshot(), inline.profile_snapshot());
        previous_inline = next_block(&mut inline, 128);
    }

    for tx in [&inline_tx, &routing_tx] {
        tx.send(EngineEvent::NoteOff {
            instrument_slot: 0,
            note: 36,
        })
        .unwrap();
    }
    for _ in 0..8 {
        let current_routing = next_block(&mut routing, 128);
        assert_eq!(current_routing, previous_inline);
        assert_eq!(routing.profile_snapshot(), inline.profile_snapshot());
        previous_inline = next_block(&mut inline, 128);
    }
    assert_eq!(routing.source_worker_health(), SourceWorkerHealth::Healthy);

    drop(routing);
    drop(inline);
    assert_eq!(routing_shutdown.shutdown().joined_workers, 2);
}

fn next_block(source: &mut EngineSource, frames: usize) -> Vec<f32> {
    (0..frames * 2).map(|_| source.next().unwrap()).collect()
}

fn threaded_parity_config() -> InstrumentsConfig {
    let instrument_duck = FxBusSlotConfig::Config {
        kind: "duck".into(),
        params: [
            ("source".into(), json!("I2")),
            ("amountPct".into(), json!(80.0)),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>(),
    };
    InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "synth".into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_1".into(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            },
            InstrumentSlotConfig {
                kind: "synth".into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".into(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 25.0,
                }),
            },
            InstrumentSlotConfig {
                kind: "synth".into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_1".into(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 40.0,
                }),
            },
            InstrumentSlotConfig {
                kind: "synth".into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_2".into(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 35.0,
                }),
            },
        ],
        mixer: Some(MixerConfig {
            buses: vec![
                FxBusConfig {
                    slots: vec![instrument_duck, FxBusSlotConfig::Kind("reverb".into())],
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume_pct: 100.0,
                },
                FxBusConfig {
                    slots: vec![
                        FxBusSlotConfig::Config {
                            kind: "delay".into(),
                            params: [
                                ("mixPct".into(), json!(50.0)),
                                ("spreadPct".into(), json!(100.0)),
                            ]
                            .into_iter()
                            .collect(),
                        },
                        FxBusSlotConfig::Kind("reverb".into()),
                    ],
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume_pct: 100.0,
                },
                FxBusConfig {
                    slots: vec![FxBusSlotConfig::Kind("reverb".into())],
                    ..FxBusConfig::default()
                },
                FxBusConfig::default(),
            ],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}
