use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_fx_bus_slot,
    prepare_instrument_slot_config, prepare_momentary_fx_start, FxBusConfig, FxBusSlotConfig,
    InstrumentSlotConfig, InstrumentsConfig, MixerConfig, MomentaryFxTarget, SampleBankConfig,
    SampleBuffer, SampleSlotConfig, SourceWorkerHealth, DEFAULT_PAN_POSITIONS,
    INSTRUMENT_SLOT_COUNT,
};
use serde_json::json;
use std::collections::BTreeMap;

const RATE: u32 = 44_100;
const BLOCK_FRAMES: usize = 128;

#[test]
fn routing_tree_full_bank_controls_previews_and_momentary_fx_run_through_rodio() {
    let (tx, rx) = event_queue();
    tx.send(EngineEvent::SetPreparedAudioConfig(full_bank_config(
        4, 0.8,
    )))
    .unwrap();
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        tx.send(EngineEvent::NoteOn {
            instrument_slot: slot as u8,
            note: if slot.is_multiple_of(2) { 60 } else { 36 },
            velocity: 100,
            duration_ms: 10_000,
        })
        .unwrap();
    }
    tx.send(EngineEvent::PreviewSample {
        instrument_slot: 1,
        buffer: sample_buffer(0.6),
        velocity: 100,
    })
    .unwrap();
    tx.send(EngineEvent::PreparedMomentaryFxStart(
        prepare_momentary_fx_start(
            "local-filter".into(),
            "filter_sweep".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Instrument { index: 0 },
            RATE,
        )
        .unwrap(),
    ))
    .unwrap();
    tx.send(EngineEvent::SetSynthParam {
        instrument_slot: 0,
        path: "synth.amp.gainPct".into(),
        value: 72.0,
    })
    .unwrap();
    tx.send(EngineEvent::SetSampleBankParam {
        instrument_slot: 1,
        path: "sample.amp.gainPct".into(),
        value: 65.0,
    })
    .unwrap();
    tx.send(EngineEvent::SetMasterVolume { volume_pct: 90.0 })
        .unwrap();
    tx.send(EngineEvent::SetInstrumentMixer {
        instrument_slot: 0,
        volume_pct: Some(80.0),
        pan_pos: Some(DEFAULT_PAN_POSITIONS / 2),
    })
    .unwrap();
    tx.send(EngineEvent::SetFxBusMixer {
        bus_index: 0,
        pan_pos: Some(DEFAULT_PAN_POSITIONS / 2),
        volume_pct: Some(90.0),
    })
    .unwrap();
    tx.send(EngineEvent::SetPreparedSampleBank {
        instrument_slot: 1,
        bank: sample_bank(0.5),
    })
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 1,
        note: 36,
        velocity: 100,
        duration_ms: 10_000,
    })
    .unwrap();
    tx.send(EngineEvent::SetPreparedInstrumentSlot {
        instrument_slot: 2,
        config: prepare_instrument_slot_config(instrument("sampler", "fx_bus_1")),
    })
    .unwrap();
    tx.send(EngineEvent::SetPreparedFxBusSlot {
        bus_index: 0,
        slot_index: 0,
        config: prepare_fx_bus_slot("compressor".into(), BTreeMap::new(), RATE),
    })
    .unwrap();
    tx.send(EngineEvent::MomentaryFxUpdate {
        id: "local-filter".into(),
        params: BTreeMap::from([("sweepOutMs".into(), json!(2.0))]),
    })
    .unwrap();

    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, RATE, BLOCK_FRAMES, None).unwrap();
    let (allocations, deallocations) = allocations_and_deallocations(|| {
        for _ in 0..BLOCK_FRAMES * 2 * 3 {
            let _ = source.next();
        }
    });
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    let profile = source.profile_snapshot();
    assert!(profile.active_synth_voices > 0);
    assert!(profile.active_sample_voices > 0);
    assert!(profile.active_preview_sample_voices > 0);
    assert_eq!(profile.active_momentary_fx, 1);
    assert!(profile.active_bus_fx_slots > 0);
    assert!(source.routing_tree_control_gate_calls > 0);

    tx.send(EngineEvent::SetPreparedAudioConfig(full_bank_config(
        1, 0.4,
    )))
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 72,
        velocity: 110,
        duration_ms: 10_000,
    })
    .unwrap();
    let mut migrated_output = Vec::with_capacity(BLOCK_FRAMES * 2 * 3);
    for _ in 0..BLOCK_FRAMES * 2 * 3 {
        migrated_output.push(source.next().unwrap());
    }
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    assert!(migrated_output.iter().any(|sample| sample.abs() > 0.0001));

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

fn full_bank_config(
    bus_count: usize,
    sample_value: f32,
) -> realtime_engine::synth::PreparedAudioConfig {
    let instruments = InstrumentsConfig {
        instruments: (0..INSTRUMENT_SLOT_COUNT)
            .map(|slot| {
                let kind = if slot.is_multiple_of(2) {
                    "synth"
                } else {
                    "sampler"
                };
                instrument(kind, &format!("fx_bus_{}", (slot % bus_count) + 1))
            })
            .collect(),
        mixer: Some(MixerConfig {
            buses: (0..bus_count)
                .map(|_| FxBusConfig {
                    slots: vec![FxBusSlotConfig::Kind("reverb".into())],
                    ..FxBusConfig::default()
                })
                .collect(),
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    };
    prepare_audio_config(
        instruments,
        Some(
            (0..INSTRUMENT_SLOT_COUNT)
                .map(|_| sample_bank(sample_value))
                .collect(),
        ),
        None,
        RATE,
    )
}

fn instrument(kind: &str, route: &str) -> InstrumentSlotConfig {
    InstrumentSlotConfig {
        kind: kind.into(),
        synth: default_synth_config(),
        mixer: Some(realtime_engine::synth::InstrumentMixerConfig {
            route: route.into(),
            pan_pos: DEFAULT_PAN_POSITIONS / 2,
            volume: 100.0,
        }),
    }
}

fn sample_bank(value: f32) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(sample_buffer(value)),
    };
    bank
}

fn sample_buffer(value: f32) -> SampleBuffer {
    SampleBuffer {
        samples: vec![value; 4096].into(),
        channels: 1,
        sample_rate: RATE,
    }
}
