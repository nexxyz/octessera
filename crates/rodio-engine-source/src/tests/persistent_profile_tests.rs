use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, InstrumentSlotConfig, InstrumentsConfig,
    SampleBankConfig, SampleBuffer, SampleSlotConfig, SourceWorkerHealth, DEFAULT_PAN_POSITIONS,
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
                    fm: None,
                    pluck: None,
                    drum: None,
                    kind: "synth".into(),
                    synth,
                    mixer: None,
                },
                InstrumentSlotConfig {
                    fm: None,
                    pluck: None,
                    drum: None,
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

fn persistent_source() -> (
    EngineEventSender,
    EngineSource,
    EngineSourceWorkerShutdownOwner,
) {
    let (tx, rx) = event_queue();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, RATE, BLOCK_FRAMES, None).unwrap();
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
fn persistent_profile_cache_reports_short_sample_completion_after_refill() {
    let samples: Arc<[f32]> = Arc::from(vec![0.8, -0.4, 0.2, -0.1]);
    let (tx, mut source, shutdown) = persistent_source();
    tx.send(EngineEvent::SetPreparedAudioConfig {
        generation: 0,
        config: mixed_config(Arc::clone(&samples)),
    })
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
    tx.send(EngineEvent::SetPreparedAudioConfig {
        generation: 0,
        config: prepare_config(synth, None),
    })
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
        generation: 0,
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
