use super::*;
use crate::queue::{COALESCED_QUEUE_CAPACITY, ORDERED_QUEUE_CAPACITY};
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_momentary_fx_start, FxBusConfig,
    FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MasterFxConfig, MixerConfig, MomentaryFxTarget, SampleBankConfig, SampleBuffer,
    SampleSlotConfig, DEFAULT_PAN_POSITIONS,
};
use std::collections::BTreeMap;

const RATE: u32 = 44_100;
const FRAMES: usize = 64;

fn source() -> (EngineEventSender, EngineSource) {
    let (tx, rx) = event_queue();
    (tx, EngineSource::with_block_frames(rx, RATE, FRAMES))
}

fn block(source: &mut EngineSource) -> Vec<f32> {
    (0..FRAMES * 2).map(|_| source.next().unwrap()).collect()
}

fn energy(samples: &[f32]) -> f32 {
    samples.iter().map(|sample| sample.abs()).sum()
}

fn synth_config() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: None,
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn sample_config() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "fx_bus_1".into(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Kind("none".into())],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume_pct: 100.0,
            }],
            master: Some(MasterFxConfig { slots: Vec::new() }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn sample_bank() -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![0.8, -0.4, 0.2, -0.1, 0.05].into(),
            channels: 1,
            sample_rate: RATE,
        }),
    };
    bank
}

fn prepared(config: InstrumentsConfig) -> realtime_engine::synth::PreparedAudioConfig {
    prepare_audio_config(config, Some(vec![sample_bank()]), None, RATE)
}

#[test]
fn prepared_config_note_and_dynamic_control_cross_source_blocks() {
    let (tx, mut source) = source();
    tx.send(EngineEvent::SetPreparedAudioConfig(
        prepared(synth_config()),
    ))
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 120,
        duration_ms: 1_000,
    })
    .unwrap();
    let before = block(&mut source);
    assert!(energy(&before) > 0.0);

    tx.send(EngineEvent::SetSynthParam {
        instrument_slot: 0,
        path: "synth.amp.gainPct".into(),
        value: 0.0,
    })
    .unwrap();
    let after = block(&mut source);
    assert!(energy(&after) < energy(&before) * 0.1);
}

#[test]
fn bus_and_master_routing_controls_bound_output_energy() {
    let config = prepared(sample_config());
    let (tx, mut source) = source();
    tx.send(EngineEvent::SetPreparedAudioConfig(config.clone()))
        .unwrap();
    tx.send(EngineEvent::PreviewSample {
        instrument_slot: 0,
        buffer: sample_bank().slots[0].buffer.clone().unwrap(),
        velocity: 127,
    })
    .unwrap();
    let audible = block(&mut source);
    assert!(energy(&audible) > 0.0);

    tx.send(EngineEvent::SetFxBusMixer {
        bus_index: 0,
        pan_pos: None,
        volume_pct: Some(0.0),
    })
    .unwrap();
    tx.send(EngineEvent::PreviewSample {
        instrument_slot: 0,
        buffer: sample_bank().slots[0].buffer.clone().unwrap(),
        velocity: 127,
    })
    .unwrap();
    assert!(energy(&block(&mut source)) < energy(&audible) * 0.01);

    tx.send(EngineEvent::SetFxBusMixer {
        bus_index: 0,
        pan_pos: None,
        volume_pct: Some(100.0),
    })
    .unwrap();
    tx.send(EngineEvent::PreviewSample {
        instrument_slot: 0,
        buffer: sample_bank().slots[0].buffer.clone().unwrap(),
        velocity: 127,
    })
    .unwrap();
    let audible_before_master_mute = block(&mut source);
    assert!(energy(&audible_before_master_mute) > 0.0);

    tx.send(EngineEvent::SetMasterVolume { volume_pct: 0.0 })
        .unwrap();
    tx.send(EngineEvent::PreviewSample {
        instrument_slot: 0,
        buffer: sample_bank().slots[0].buffer.clone().unwrap(),
        velocity: 127,
    })
    .unwrap();
    assert!(energy(&block(&mut source)) < f32::EPSILON);
}

#[test]
fn momentary_fx_start_and_stop_follow_block_lifecycle() {
    let (tx, mut source) = source();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 1_000,
    })
    .unwrap();
    let _ = block(&mut source);
    tx.send(EngineEvent::PreparedMomentaryFxStart(
        prepare_momentary_fx_start(
            "test".into(),
            "stutter".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Global,
            RATE,
        )
        .unwrap(),
    ))
    .unwrap();
    let _ = block(&mut source);
    assert_eq!(source.engine.profile_snapshot().active_momentary_fx, 1);
    tx.send(EngineEvent::MomentaryFxStop { id: "test".into() })
        .unwrap();
    let _ = block(&mut source);
    assert_eq!(source.engine.profile_snapshot().active_momentary_fx, 0);
}

#[test]
fn sample_preview_is_deterministic_for_the_same_fixture() {
    let run = || {
        let (tx, mut source) = source();
        tx.send(EngineEvent::SetPreparedAudioConfig(prepared(
            sample_config(),
        )))
        .unwrap();
        tx.send(EngineEvent::PreviewSample {
            instrument_slot: 0,
            buffer: sample_bank().slots[0].buffer.clone().unwrap(),
            velocity: 127,
        })
        .unwrap();
        energy(&block(&mut source))
    };
    assert_eq!(run().to_bits(), run().to_bits());
}

#[test]
fn superseding_coalesced_controls_are_applied_before_rendering() {
    let (tx, mut source) = source();
    tx.send(EngineEvent::SetPreparedAudioConfig(
        prepared(synth_config()),
    ))
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 1_000,
    })
    .unwrap();
    for value in 0..=COALESCED_QUEUE_CAPACITY + 8 {
        tx.send(EngineEvent::SetMasterVolume {
            volume_pct: if value == COALESCED_QUEUE_CAPACITY + 8 {
                100.0
            } else {
                0.0
            },
        })
        .unwrap();
    }
    assert!(energy(&block(&mut source)) > 0.0);
}

#[test]
fn emergency_all_notes_off_clears_voices_after_a_populated_queue() {
    let (tx, mut source) = source();
    for _ in 0..ORDERED_QUEUE_CAPACITY {
        tx.send(EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 100,
            duration_ms: 1_000,
        })
        .unwrap();
    }
    tx.send(EngineEvent::AllNotesOff).unwrap();
    for _ in 0..400 {
        let _ = block(&mut source);
    }
    let snapshot = source.engine.profile_snapshot();
    assert_eq!(snapshot.active_synth_voices, 0);
}

#[test]
fn control_budget_spills_into_the_next_source_block() {
    let (tx, mut source) = source();
    let (report_tx, report_rx) = std::sync::mpsc::channel();
    for _ in 0..MAX_CONTROL_EVENTS_PER_BLOCK {
        tx.send(EngineEvent::ProbeMark {
            sent_at: std::time::Instant::now(),
            report_tx: report_tx.clone(),
        })
        .unwrap();
    }
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 72,
        velocity: 100,
        duration_ms: 1_000,
    })
    .unwrap();
    let first = block(&mut source);
    assert_eq!(energy(&first), 0.0);
    assert_eq!(report_rx.try_iter().count(), MAX_CONTROL_EVENTS_PER_BLOCK);
    assert!(energy(&block(&mut source)) > 0.0);
}
