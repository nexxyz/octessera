use super::*;

fn delay_params() -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("timeMs".into(), json!(25.0)),
        ("feedback".into(), json!(0.2)),
        ("mixPct".into(), json!(50.0)),
    ])
}

fn eq_params() -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("lowGainDb".into(), json!(6.0)),
        ("midGainDb".into(), json!(-6.0)),
        ("midFreqHz".into(), json!(1000.0)),
        ("midQ".into(), json!(2.0)),
        ("highGainDb".into(), json!(6.0)),
        ("mixPct".into(), json!(100.0)),
    ])
}

fn bus_fx_instruments() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: Vec::new(),
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Config {
                    kind: "delay".into(),
                    params: delay_params(),
                }],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume_pct: 100.0,
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn master_fx_instruments() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: Vec::new(),
        mixer: Some(MixerConfig {
            buses: Vec::new(),
            master: Some(MasterFxConfig {
                slots: vec![FxBusSlotConfig::Config {
                    kind: "eq".into(),
                    params: eq_params(),
                }],
            }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

#[test]
fn same_kind_bus_fx_state_is_retired_without_callback_deallocation() {
    let (tx, rx) = event_queue();
    let mut source = EngineSource::new(rx, 44_100);
    source.engine.set_instruments(bus_fx_instruments());
    warm_source(&mut source);
    tx.send(EngineEvent::SetPreparedFxBusSlot {
        bus_index: 0,
        slot_index: 0,
        generation: 0,
        config: prepare_fx_bus_slot("delay".into(), delay_params(), 44_100),
    })
    .unwrap();

    assert_no_callback_memory_activity(&mut source);
}

#[test]
fn same_kind_global_fx_state_is_retired_without_callback_deallocation() {
    let (tx, rx) = event_queue();
    let (mut source, retired_rx) = EngineSource::with_test_retirement_receiver(rx, 44_100);
    source.engine.set_instruments(master_fx_instruments());
    warm_source(&mut source);
    tx.send(EngineEvent::SetPreparedGlobalFxSlot {
        slot_index: 0,
        generation: 0,
        config: prepare_global_fx_slot("eq".into(), eq_params()),
    })
    .unwrap();

    let (allocation_count, deallocation_count) = callback_memory_activity(&mut source);
    let retired = receive_retired_state(&retired_rx);
    assert!(!retired.is_empty());
    assert_eq!(allocation_count, 0);
    assert_eq!(deallocation_count, 0);
}

#[test]
fn invalid_bus_fx_state_is_retired_without_callback_deallocation() {
    let (tx, rx) = event_queue();
    let mut source = EngineSource::new(rx, 44_100);
    warm_source(&mut source);
    tx.send(EngineEvent::SetPreparedFxBusSlot {
        bus_index: u8::MAX,
        slot_index: 0,
        generation: 0,
        config: prepare_fx_bus_slot("delay".into(), delay_params(), 44_100),
    })
    .unwrap();

    assert_no_callback_memory_activity(&mut source);
}

#[test]
fn invalid_global_fx_state_is_retired_without_callback_deallocation() {
    let (tx, rx) = event_queue();
    let (mut source, retired_rx) = EngineSource::with_test_retirement_receiver(rx, 44_100);
    warm_source(&mut source);
    tx.send(EngineEvent::SetPreparedGlobalFxSlot {
        slot_index: u8::MAX,
        generation: 0,
        config: prepare_global_fx_slot("eq".into(), eq_params()),
    })
    .unwrap();

    let (allocation_count, deallocation_count) = callback_memory_activity(&mut source);
    let retired = retired_rx
        .try_recv()
        .expect("expected the ignored event back");
    assert!(matches!(
        retired.event,
        Some(EngineEvent::SetPreparedGlobalFxSlot {
            slot_index: u8::MAX,
            ..
        })
    ));
    assert_eq!(allocation_count, 0);
    assert_eq!(deallocation_count, 0);
}
