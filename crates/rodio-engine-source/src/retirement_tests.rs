use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_fx_bus_slot, prepare_global_fx_slot,
    FxBusConfig, FxBusSlotConfig, InstrumentSlotConfig, InstrumentsConfig, MasterFxConfig,
    MixerConfig, PreparedAudioConfig, SampleBankConfig, SampleBuffer, SampleSlotConfig,
    INSTRUMENT_SLOT_COUNT, MAX_SAMPLE_VOICES_PER_SLOT, SAMPLE_VOICE_LANE_CAPACITY,
    SAMPLE_VOICE_RETIREMENT_CAPACITY,
};
use serde_json::json;
use std::collections::BTreeMap;

impl EngineSource {
    pub(super) fn retired_backlog_len(&self) -> usize {
        self.retired_backlog.as_ref().expect("retired backlog").len
    }

    pub(super) fn retired_backlog_items(&self) -> impl Iterator<Item = &RetiredAudioItem> {
        self.retired_backlog
            .as_ref()
            .expect("retired backlog")
            .items
            .iter()
            .filter_map(Option::as_ref)
    }

    pub(super) fn set_retired_drop_probe(
        &mut self,
        drop_tx: std::sync::mpsc::Sender<std::thread::ThreadId>,
    ) {
        self.retired_drop_probe = Some(drop_tx);
    }

    pub(super) fn handoff_shutdown_for_test(&mut self) {
        self.handoff_shutdown();
    }
}

pub(super) fn warm_source(source: &mut EngineSource) {
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

pub(super) fn preview_buffer() -> SampleBuffer {
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

pub(super) fn full_sample_config(value: f32) -> PreparedAudioConfig {
    prepare_audio_config(
        InstrumentsConfig {
            instruments: (0..INSTRUMENT_SLOT_COUNT)
                .map(|_| InstrumentSlotConfig {
                    kind: "sampler".into(),
                    synth: default_synth_config(),
                    mixer: None,
                })
                .collect(),
            mixer: None,
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        },
        Some(
            (0..INSTRUMENT_SLOT_COUNT)
                .map(|_| sample_bank(value))
                .collect(),
        ),
        None,
        44_100,
    )
}

fn sample_bank(value: f32) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![value; 16_384].into(),
            channels: 1,
            sample_rate: 44_100,
        }),
    };
    bank
}

fn full_sample_source() -> (
    EngineEventSender,
    EngineSource,
    crossbeam_channel::Receiver<RetiredAudioItem>,
) {
    let (tx, rx) = event_queue();
    let (mut source, retired_rx) = EngineSource::with_test_retirement_receiver(rx, 44_100);
    warm_source(&mut source);
    tx.send(EngineEvent::SetPreparedAudioConfig(full_sample_config(1.0)))
        .unwrap();
    source.refill();
    while let Ok(item) = retired_rx.try_recv() {
        drop(item);
    }
    source.idx = source.buf.len();
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for _ in 0..MAX_SAMPLE_VOICES_PER_SLOT {
            tx.send(EngineEvent::NoteOn {
                instrument_slot: slot as u8,
                note: 36,
                velocity: 100,
                duration_ms: 10_000,
            })
            .unwrap();
        }
    }
    assert_no_callback_memory_activity(&mut source);
    assert_eq!(
        source.engine.profile_snapshot().active_sample_voices,
        SAMPLE_VOICE_LANE_CAPACITY
    );
    (tx, source, retired_rx)
}

fn drop_retired_state_off_callback(state: RetiredAudioState) -> (usize, usize) {
    allocations_and_deallocations(|| drop(state))
}

#[test]
fn full_sample_lane_reuse_has_no_callback_memory_activity() {
    let (tx, mut source, retired_rx) = full_sample_source();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 36,
        velocity: 100,
        duration_ms: 10_000,
    })
    .unwrap();

    assert_no_callback_memory_activity(&mut source);
    let retired = receive_retired_state(&retired_rx);
    assert_eq!(drop_retired_state_off_callback(retired), (0, 0));
    assert_eq!(
        source.engine.profile_snapshot().active_sample_voices,
        SAMPLE_VOICE_LANE_CAPACITY
    );
}

#[test]
fn full_sample_bank_replacement_has_no_callback_memory_activity() {
    let (tx, mut source, retired_rx) = full_sample_source();
    tx.send(EngineEvent::SetPreparedAudioConfig(full_sample_config(2.0)))
        .unwrap();

    assert_no_callback_memory_activity(&mut source);
    let retired = receive_retired_state(&retired_rx);
    let (allocations, deallocations) = drop_retired_state_off_callback(retired);
    assert_eq!(allocations, 0);
    assert!(deallocations > 0);
    assert_eq!(source.engine.profile_snapshot().active_sample_voices, 0);
}

#[test]
fn full_sample_all_notes_off_has_no_callback_memory_activity() {
    let (tx, mut source, retired_rx) = full_sample_source();
    tx.send(EngineEvent::AllNotesOff).unwrap();

    assert_no_callback_memory_activity(&mut source);
    let retired = receive_retired_state(&retired_rx);
    assert_eq!(drop_retired_state_off_callback(retired), (0, 0));
    assert_eq!(source.engine.profile_snapshot().active_sample_voices, 0);
    let (_, teardown_deallocations) = allocations_and_deallocations(|| drop(source));
    assert!(teardown_deallocations > 0);
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
    assert_eq!(source.retired_backlog_len(), 0);
}

#[path = "retirement_burst_tests.rs"]
mod burst_tests;
