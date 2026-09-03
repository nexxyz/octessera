use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, FxBusConfig, FxBusSlotConfig,
    InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig, MasterFxConfig, MixerConfig,
    SampleBankConfig, SampleBuffer, SampleSlotConfig, SourceWorkerHealth, SynthEngine,
    VoiceStealingMode, DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT, SYNTH_VOICE_LANE_CAPACITY,
};
use std::sync::Arc;
use std::time::Duration;

const RATE: u32 = 44_100;
const BLOCK_FRAMES: usize = 128;

fn block_bits(source: &mut EngineSource) -> Vec<u32> {
    (0..BLOCK_FRAMES * 2)
        .map(|_| source.next().unwrap().to_bits())
        .collect()
}

pub(super) fn sample_bank(samples: Arc<[f32]>) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples,
            channels: 1,
            sample_rate: RATE,
        }),
    };
    bank
}

pub(super) fn mixed_config(samples: Arc<[f32]>) -> realtime_engine::synth::PreparedAudioConfig {
    prepare_config(default_synth_config(), Some(sample_bank(samples)))
}

fn prepare_config(
    synth: realtime_engine::synth::SynthConfig,
    sample_bank: Option<SampleBankConfig>,
) -> realtime_engine::synth::PreparedAudioConfig {
    prepare_audio_config(
        InstrumentsConfig {
            instruments: vec![
                InstrumentSlotConfig {
                    kind: "synth".into(),
                    synth,
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
        sample_bank.map(|bank| vec![SampleBankConfig::default(), bank]),
        None,
        RATE,
    )
}

fn bus_heavy_config() -> realtime_engine::synth::PreparedAudioConfig {
    prepare_audio_config(
        InstrumentsConfig {
            instruments: (0..INSTRUMENT_SLOT_COUNT)
                .map(|slot| InstrumentSlotConfig {
                    kind: "synth".into(),
                    synth: default_synth_config(),
                    mixer: Some(InstrumentMixerConfig {
                        route: format!("fx_bus_{}", (slot % 4) + 1),
                        pan_pos: slot.min(DEFAULT_PAN_POSITIONS - 1),
                        volume: 100.0,
                    }),
                })
                .collect(),
            mixer: Some(MixerConfig {
                buses: vec![
                    bus(vec!["delay", "reverb"], 1),
                    bus(vec!["filter_lfo", "chorus"], 2),
                    bus(vec!["compressor"], 3),
                    bus(vec!["eq"], 4),
                ],
                master: Some(MasterFxConfig {
                    slots: vec![
                        FxBusSlotConfig::Kind("compressor".into()),
                        FxBusSlotConfig::Kind("eq".into()),
                    ],
                }),
            }),
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        },
        None,
        Some(VoiceStealingMode::None),
        RATE,
    )
}

fn bus(slots: Vec<&str>, pan_pos: usize) -> FxBusConfig {
    FxBusConfig {
        slots: slots
            .into_iter()
            .map(|kind| FxBusSlotConfig::Kind(kind.into()))
            .collect(),
        pan_pos,
        volume_pct: 100.0,
    }
}

fn persistent_source() -> (
    EngineEventSender,
    EngineSource,
    EngineSourceWorkerShutdownOwner,
) {
    let (tx, rx) = event_queue();
    let (mut source, shutdown) =
        EngineSource::with_persistent_workers(rx, RATE, BLOCK_FRAMES, None).unwrap();
    source
        .worker_state
        .worker
        .as_mut()
        .expect("persistent worker")
        .runtime
        .set_deadline_for_test(Duration::from_secs(1));
    (tx, source, shutdown)
}

#[test]
fn persistent_profile_cache_tracks_controls_without_pool_reads() {
    let samples: Arc<[f32]> = Arc::from(vec![0.8; 4_096]);
    let (tx, mut source, shutdown) = persistent_source();
    tx.send(EngineEvent::SetPreparedAudioConfig(mixed_config(
        Arc::clone(&samples),
    )))
    .unwrap();
    tx.send(EngineEvent::SetVoiceStealingMode(VoiceStealingMode::None))
        .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 1,
        note: 36,
        velocity: 100,
        duration_ms: 50_000,
    })
    .unwrap();
    for note in 0..=SYNTH_VOICE_LANE_CAPACITY {
        tx.send(EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 36 + note as u8,
            velocity: 100,
            duration_ms: 50_000,
        })
        .unwrap();
    }
    let _ = block_bits(&mut source);

    let active = source.profile_snapshot();
    assert_eq!(active.active_synth_voices, SYNTH_VOICE_LANE_CAPACITY);
    assert_eq!(active.active_sample_voices, 1);
    assert_eq!(active.cumulative_voice_admission_drops, 1);

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
    let _ = block_bits(&mut source);
    assert_eq!(source.profile_snapshot(), active);

    tx.send(EngineEvent::SetPreparedSampleBank {
        instrument_slot: 1,
        bank: sample_bank(Arc::clone(&samples)),
    })
    .unwrap();
    let _ = block_bits(&mut source);
    let replaced = source.profile_snapshot();
    assert_eq!(replaced.active_synth_voices, SYNTH_VOICE_LANE_CAPACITY);
    assert_eq!(replaced.active_sample_voices, 0);
    assert_eq!(replaced.cumulative_voice_admission_drops, 1);

    tx.send(EngineEvent::NoteOn {
        instrument_slot: 1,
        note: 36,
        velocity: 100,
        duration_ms: 50_000,
    })
    .unwrap();
    let _ = block_bits(&mut source);
    assert_eq!(source.profile_snapshot().active_sample_voices, 1);
    assert_eq!(
        source.profile_snapshot().cumulative_voice_admission_drops,
        1
    );

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn persistent_profile_cache_reports_bus_fx_after_completed_block() {
    let config = bus_heavy_config();
    let (tx, mut source, shutdown) = persistent_source();
    tx.send(EngineEvent::SetPreparedAudioConfig(config.clone()))
        .unwrap();
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for note in [60, 67] {
            tx.send(EngineEvent::NoteOn {
                instrument_slot: slot as u8,
                note,
                velocity: 100,
                duration_ms: 60_000,
            })
            .unwrap();
        }
    }
    let _ = block_bits(&mut source);
    let cached = source.profile_snapshot();

    let mut inline = SynthEngine::new(RATE);
    let retired = inline.apply_prepared_audio_config(config);
    drop(retired);
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for note in [60, 67] {
            inline.note_on(slot as u8, note, 100, 60_000);
        }
    }
    let inline_snapshot = inline.profile_snapshot();

    assert_eq!(cached.active_synth_voices, 16);
    assert_eq!(cached.active_sample_voices, 0);
    assert_eq!(cached.active_preview_sample_voices, 0);
    assert_eq!(cached.active_momentary_fx, 0);
    assert_eq!(cached.active_bus_fx_slots, 6);
    assert_eq!(cached.active_global_fx_slots, 2);
    assert_eq!(cached, inline_snapshot);

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn persistent_profile_cache_reports_short_sample_completion_after_refill() {
    let samples: Arc<[f32]> = Arc::from(vec![0.8, -0.4, 0.2, -0.1]);
    let (tx, mut source, shutdown) = persistent_source();
    tx.send(EngineEvent::SetPreparedAudioConfig(mixed_config(
        Arc::clone(&samples),
    )))
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 1,
        note: 36,
        velocity: 100,
        duration_ms: 50_000,
    })
    .unwrap();
    let _ = block_bits(&mut source);
    assert_eq!(source.profile_snapshot().active_sample_voices, 0);
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn persistent_profile_cache_reports_synth_release_completion_after_refill() {
    let mut synth = default_synth_config();
    synth.amp_env.release_ms = 0.0;
    synth.filter_env.release_ms = 0.0;
    let (tx, mut source, shutdown) = persistent_source();
    tx.send(EngineEvent::SetPreparedAudioConfig(prepare_config(
        synth, None,
    )))
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 0,
    })
    .unwrap();
    let _ = block_bits(&mut source);
    assert_eq!(source.profile_snapshot().active_synth_voices, 0);
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn persistent_profile_cache_reports_preview_completion_and_drains_pending_retirement() {
    let samples: Arc<[f32]> = Arc::from(vec![0.8, -0.4, 0.2, -0.1]);
    let (tx, mut source, shutdown) = persistent_source();
    tx.send(EngineEvent::PreviewSample {
        instrument_slot: 1,
        buffer: SampleBuffer {
            samples: Arc::clone(&samples),
            channels: 1,
            sample_rate: RATE,
        },
        velocity: 100,
    })
    .unwrap();
    let _ = block_bits(&mut source);
    assert_eq!(source.profile_snapshot().active_preview_sample_voices, 0);
    assert!(source.engine.pending_render_retired_is_empty());
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}
