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
                fm: None,
                pluck: None,
                drum: None,
                kind: "sampler".into(),
                synth,
                mixer: None,
            },
            InstrumentSlotConfig {
                fm: None,
                pluck: None,
                drum: None,
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
