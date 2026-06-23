use super::engine::FREEZE_INJECT_MS;
use super::{
    default_synth_config, FilterType, FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig,
    InstrumentSlotConfig, InstrumentsConfig, MasterFxConfig, MixerConfig, MomentaryFxTarget,
    SampleBankConfig, SampleBuffer, SampleSlotConfig, SynthEngine, DEFAULT_PAN_POSITIONS,
    INSTRUMENT_SLOT_COUNT,
};
use serde_json::json;
use std::collections::BTreeMap;

mod basics;
mod coverage_compile;
mod coverage_process;
mod dynamics;
mod momentary_buffers;
mod momentary_pitch_filter;
mod routing;
mod routing_gating;
mod voices;

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

fn duck_test_engine(with_duck: bool) -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    let slot1 = if with_duck {
        FxBusSlotConfig::Config {
            kind: "duck".to_string(),
            params: BTreeMap::from([
                ("source".to_string(), serde_json::json!("I1")),
                ("threshold".to_string(), serde_json::json!(0.01)),
                ("amountPct".to_string(), serde_json::json!(100.0)),
                ("attackMs".to_string(), serde_json::json!(1.0)),
                ("releaseMs".to_string(), serde_json::json!(20.0)),
            ]),
        }
    } else {
        FxBusSlotConfig::Kind("none".to_string())
    };
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "sampler".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_2".to_string(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            },
            InstrumentSlotConfig {
                kind: "sampler".to_string(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_1".to_string(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            },
        ],
        mixer: Some(MixerConfig {
            buses: vec![
                FxBusConfig {
                    slots: vec![slot1],
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                },
                FxBusConfig {
                    slots: vec![FxBusSlotConfig::Kind("none".to_string())],
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                },
            ],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.set_sample_banks(vec![
        sample_bank(vec![1.0; 512]),
        sample_bank(vec![0.5; 512]),
    ]);
    engine
}
