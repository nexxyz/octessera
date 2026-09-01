use super::*;
use realtime_engine::synth::{
    prepare_fx_bus_slot, prepare_global_fx_slot, FxBusConfig, FxBusSlotConfig, MasterFxConfig,
    MixerConfig, SampleBuffer,
};
use serde_json::json;
use std::collections::BTreeMap;

const RETIREMENT_FILL_COUNT: usize =
    RETIREMENT_QUEUE_CAPACITY + RETIREMENT_CONTROL_BACKLOG_CAPACITY;
const RETIREMENT_EVENT_COUNT: usize = RETIREMENT_FILL_COUNT + 8;

fn warm_source(source: &mut EngineSource) {
    for _ in 0..512 {
        let _ = source.next();
    }
}

fn callback_memory_activity(source: &mut EngineSource) -> (usize, usize) {
    allocations_and_deallocations(|| {
        for _ in 0..256 {
            let _ = source.next();
        }
    })
}

fn assert_no_callback_memory_activity(source: &mut EngineSource) {
    let (allocation_count, deallocation_count) = callback_memory_activity(source);
    assert_eq!(allocation_count, 0);
    assert_eq!(deallocation_count, 0);
}

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

fn preview_buffer() -> SampleBuffer {
    SampleBuffer {
        samples: vec![0.25].into(),
        channels: 1,
        sample_rate: 44_100,
    }
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

fn retired_event_id(item: &RetiredAudioItem) -> &str {
    match item.event.as_ref() {
        Some(EngineEvent::MomentaryFxStop { id }) => id,
        _ => panic!("expected a retired momentary FX event"),
    }
}

fn drain_retired_event_ids(
    retired_rx: &crossbeam_channel::Receiver<RetiredAudioItem>,
    ids: &mut Vec<String>,
) {
    while let Ok(item) = retired_rx.try_recv() {
        ids.push(retired_event_id(&item).to_owned());
    }
}

fn fill_retirement_storage(
    tx: &EngineEventSender,
    source: &mut EngineSource,
    retired_rx: &crossbeam_channel::Receiver<RetiredAudioItem>,
) {
    for index in 0..RETIREMENT_FILL_COUNT {
        tx.send(EngineEvent::MomentaryFxStop {
            id: format!("fill-{index}"),
        })
        .unwrap();
    }
    assert_eq!(
        source.drain_control_events().control_events,
        MAX_CONTROL_EVENTS_PER_BLOCK as u64
    );
    assert_eq!(
        source.drain_control_events().control_events,
        (RETIREMENT_FILL_COUNT - MAX_CONTROL_EVENTS_PER_BLOCK) as u64
    );
    assert!(!source.retirement_disconnected);
    assert_eq!(retired_rx.len(), RETIREMENT_QUEUE_CAPACITY);
    assert_eq!(
        source.retired_backlog_len,
        RETIREMENT_CONTROL_BACKLOG_CAPACITY
    );
}

fn receive_retired_state(
    retired_rx: &crossbeam_channel::Receiver<RetiredAudioItem>,
) -> RetiredAudioState {
    let item = retired_rx.try_recv().expect("expected a retired state");
    assert!(item.event.is_none());
    item.state.expect("expected retired state payload")
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
        bus_index: usize::MAX,
        slot_index: 0,
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
        slot_index: usize::MAX,
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
fn empty_retired_state_is_skipped() {
    let (_tx, rx) = event_queue();
    let mut source = EngineSource::new(rx, 44_100);
    source.retire_state(RetiredAudioState::default());
    assert_eq!(source.retired_backlog_len, 0);
}

#[test]
fn retirement_backpressure_preserves_fifo_and_resumes_after_receiver_drain() {
    let (tx, rx) = event_queue();
    let (mut source, retired_rx) = EngineSource::with_test_retirement_receiver(rx, 44_100);
    for index in 0..RETIREMENT_EVENT_COUNT {
        tx.send(EngineEvent::MomentaryFxStop {
            id: format!("event-{index}"),
        })
        .unwrap();
    }

    let (allocation_count, deallocation_count) = allocations_and_deallocations(|| {
        assert_eq!(
            source.drain_control_events().control_events,
            MAX_CONTROL_EVENTS_PER_BLOCK as u64
        );
        assert_eq!(
            source.drain_control_events().control_events,
            (RETIREMENT_FILL_COUNT - MAX_CONTROL_EVENTS_PER_BLOCK) as u64
        );
        assert_eq!(source.drain_control_events().control_events, 0);
    });
    assert_eq!(allocation_count, 0);
    assert_eq!(deallocation_count, 0);
    assert!(!source.retirement_disconnected);
    assert_eq!(retired_rx.len(), RETIREMENT_QUEUE_CAPACITY);
    assert_eq!(
        source.retired_backlog_len,
        RETIREMENT_CONTROL_BACKLOG_CAPACITY
    );

    let mut retired_ids = Vec::new();
    drain_retired_event_ids(&retired_rx, &mut retired_ids);
    assert_eq!(retired_ids.len(), RETIREMENT_QUEUE_CAPACITY);
    assert_eq!(
        source.drain_control_events().control_events,
        (RETIREMENT_EVENT_COUNT - RETIREMENT_FILL_COUNT) as u64
    );
    while source.retired_backlog_len > 0 {
        drain_retired_event_ids(&retired_rx, &mut retired_ids);
        source.flush_retired_backlog();
    }
    drain_retired_event_ids(&retired_rx, &mut retired_ids);
    assert_eq!(source.retired_backlog_len, 0);
    let expected_ids: Vec<_> = (0..RETIREMENT_EVENT_COUNT)
        .map(|index| format!("event-{index}"))
        .collect();
    assert_eq!(retired_ids, expected_ids);
}

#[test]
fn retirement_disconnect_preserves_current_item_and_stops_control_drain() {
    let (tx, rx) = event_queue();
    let (mut source, retired_rx) = EngineSource::with_test_retirement_receiver(rx, 44_100);
    drop(retired_rx);
    tx.send(EngineEvent::MomentaryFxStop {
        id: "disconnected-current".into(),
    })
    .unwrap();
    tx.send(EngineEvent::MomentaryFxStop {
        id: "disconnected-next".into(),
    })
    .unwrap();

    let (allocation_count, deallocation_count) = allocations_and_deallocations(|| {
        assert_eq!(source.drain_control_events().control_events, 1);
    });
    assert_eq!(allocation_count, 0);
    assert_eq!(deallocation_count, 0);
    assert!(source.retirement_disconnected);
    assert_eq!(source.retired_backlog_len, 1);
    let item = source.retired_backlog[source.retired_backlog_read]
        .as_ref()
        .expect("disconnected item must remain in backlog");
    assert_eq!(retired_event_id(item), "disconnected-current");
    assert_eq!(source.drain_control_events().control_events, 0);
    let queued = source
        .control_rx
        .try_recv_ordered()
        .expect("later control event must remain queued");
    match queued {
        EngineEvent::MomentaryFxStop { id } => assert_eq!(id, "disconnected-next"),
        _ => panic!("expected queued momentary FX event"),
    }
}

#[test]
fn pending_render_retirement_stays_owned_until_capacity_returns() {
    let (tx, rx) = event_queue();
    let (mut source, retired_rx) = EngineSource::with_test_retirement_receiver(rx, 44_100);
    warm_source(&mut source);
    fill_retirement_storage(&tx, &mut source, &retired_rx);
    source.retire_event(EngineEvent::MomentaryFxStop {
        id: "pending-capacity".into(),
    });
    assert_eq!(source.retired_backlog_len, RETIREMENT_BACKLOG_CAPACITY);

    let _ = source.engine.preview_sample(0, preview_buffer(), 100);
    let (allocation_count, deallocation_count) = allocations_and_deallocations(|| {
        let _ = source.next();
    });
    assert_eq!(allocation_count, 0);
    assert_eq!(deallocation_count, 0);
    assert_eq!(
        source
            .engine
            .profile_snapshot()
            .active_preview_sample_voices,
        0
    );
    assert!(!source.engine.pending_render_retired_is_empty());
    assert_eq!(source.retired_backlog_len, RETIREMENT_BACKLOG_CAPACITY);

    let mut retired_ids = Vec::new();
    drain_retired_event_ids(&retired_rx, &mut retired_ids);
    source.refill();
    assert!(source.engine.pending_render_retired_is_empty());
    assert!(source
        .retired_backlog
        .iter()
        .flatten()
        .any(|item| { item.state.as_ref().is_some_and(|state| !state.is_empty()) }));
}
