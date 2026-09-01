use super::*;
use std::sync::Arc;

pub(super) fn dynamic_engine() -> SynthEngine {
    let mut synth = default_synth_config();
    synth.filter.cutoff_hz = 1_100.0;
    synth.filter.resonance = 48.0;
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "sampler".into(),
                synth,
                mixer: None,
            },
            InstrumentSlotConfig {
                kind: "synth".into(),
                synth,
                mixer: None,
            },
        ],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let mut bank = sample_bank(
        (0..4_096)
            .map(|index| (index as f32 * 0.013).sin())
            .collect(),
    );
    bank.tune_semis = 7.0;
    bank.gain_pct = 83.0;
    bank.velocity_sensitivity_pct = 37.0;
    bank.filter_cutoff_hz = 1_200.0;
    bank.filter_resonance = 54.0;
    let _ = engine.set_sample_banks(vec![bank, SampleBankConfig::default()]);
    engine
}

pub(super) fn full_mixed_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: (0..8)
            .map(|_| InstrumentSlotConfig {
                kind: "synth".into(),
                synth: default_synth_config(),
                mixer: None,
            })
            .collect(),
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let mut banks = Vec::new();
    for _ in 0..8 {
        banks.push(sample_bank(vec![0.25; 16_384]));
    }
    let _ = engine.set_sample_banks(banks);
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    engine
}

pub(super) fn sample_engine_with_shared_buffer(samples: Arc<[f32]>) -> SynthEngine {
    let mut engine = dynamic_engine();
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples,
            channels: 1,
            sample_rate: 48_000,
        }),
    };
    let _ = engine.set_sample_banks(vec![bank, SampleBankConfig::default()]);
    engine
}

pub(super) fn assert_worker_matches_inline(
    runtime: &mut SourceWorkerRuntime,
    worker: &mut SynthEngine,
    inline: &mut SynthEngine,
    frames: usize,
) {
    let mut worker_left = Vec::with_capacity(frames);
    let mut worker_right = Vec::with_capacity(frames);
    let mut worker_out = Vec::with_capacity(frames * 2);
    let mut inline_left = Vec::with_capacity(frames);
    let mut inline_right = Vec::with_capacity(frames);
    let mut inline_out = Vec::with_capacity(frames * 2);
    worker.render_interleaved_block_with_source_runtime(
        runtime,
        frames,
        &mut worker_left,
        &mut worker_right,
        &mut worker_out,
    );
    inline.render_interleaved_block(frames, &mut inline_left, &mut inline_right, &mut inline_out);
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_eq!(worker_out.len(), inline_out.len());
    for (index, (actual, expected)) in worker_out.iter().zip(inline_out).enumerate() {
        assert_eq!(actual.to_bits(), expected.to_bits(), "sample {index}");
    }
}

fn sample_bank(samples: Vec<f32>) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: samples.into(),
            channels: 1,
            sample_rate: 48_000,
        }),
    };
    bank
}
