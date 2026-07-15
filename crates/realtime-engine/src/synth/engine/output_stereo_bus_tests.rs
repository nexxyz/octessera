use super::*;
use crate::synth::{
    FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MixerConfig, SampleBankConfig, SampleBuffer, SampleSlotConfig,
};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn delay_spread_zero_matches_mono_bus_output() {
    let mut spread_zero = spread_sample_bus_engine(0.0, 35.0, false);
    let mut reference = spread_sample_bus_engine_without_spread_param();
    spread_zero.note_on(0, 36, 127, 0);
    reference.note_on(0, 36, 127, 0);

    assert_samples_match(&mut spread_zero, &mut reference, 64);
}

#[test]
fn delay_spread_creates_lr_difference_and_preserves_mono_sum() {
    let mut spread = spread_sample_bus_engine(100.0, 50.0, false);
    let mut mono = spread_sample_bus_engine(0.0, 50.0, false);
    spread.note_on(0, 36, 127, 0);
    mono.note_on(0, 36, 127, 0);

    let mut saw_difference = false;
    for frame in 0..32 {
        let (left, right) = spread.next_stereo_sample();
        let (mono_left, mono_right) = mono.next_stereo_sample();
        if (left - right).abs() > 1.0e-6 {
            saw_difference = true;
        }
        let spread_mid = (left + right) * 0.5;
        let mono_mid = (mono_left + mono_right) * 0.5;
        assert!((spread_mid - mono_mid).abs() < 1.0e-6, "frame {frame}");
    }
    assert!(saw_difference);
}

#[test]
fn delay_spread_mix_zero_does_not_widen() {
    let mut engine = spread_sample_bus_engine(100.0, 0.0, false);
    engine.note_on(0, 36, 127, 0);

    for frame in 0..32 {
        let (left, right) = engine.next_stereo_sample();
        assert!((left - right).abs() < 1.0e-7, "frame {frame}");
    }
}

#[test]
fn delay_spread_before_downstream_mono_slot_widens_final_output() {
    let mut engine = spread_sample_bus_engine(100.0, 50.0, true);
    engine.note_on(0, 36, 127, 0);

    let mut saw_difference = false;
    for _ in 0..32 {
        let (left, right) = engine.next_stereo_sample();
        saw_difference |= (left - right).abs() > 1.0e-6;
    }
    assert!(saw_difference);
}

#[test]
fn auto_pan_produces_real_stereo_bus_output_without_changing_stored_pan() {
    let mut engine = auto_pan_sample_bus_engine();
    engine.note_on(0, 36, 127, 0);
    let stored_pan = engine.bus_pan_pos[0];

    let mut saw_difference = false;
    for _ in 0..64 {
        let (left, right) = engine.next_stereo_sample();
        saw_difference |= (left - right).abs() > 1.0e-6;
    }
    assert_eq!(engine.bus_pan_pos[0], stored_pan);
    assert!(saw_difference);
}

#[test]
fn bus_volume_scales_final_stereo_output() {
    let mut full = spread_sample_bus_engine_with_volume(100.0, 100.0, 50.0);
    let mut half = spread_sample_bus_engine_with_volume(50.0, 100.0, 50.0);
    let mut muted = spread_sample_bus_engine_with_volume(0.0, 100.0, 50.0);
    full.note_on(0, 36, 127, 0);
    half.note_on(0, 36, 127, 0);
    muted.note_on(0, 36, 127, 0);

    let mut saw_output = false;
    for frame in 0..32 {
        let (full_left, full_right) = full.next_stereo_sample();
        let (half_left, half_right) = half.next_stereo_sample();
        let (muted_left, muted_right) = muted.next_stereo_sample();
        saw_output |= full_left.abs() > 1.0e-6 || full_right.abs() > 1.0e-6;
        assert!(
            (half_left - full_left * 0.5).abs() < 1.0e-6,
            "left frame {frame}"
        );
        assert!(
            (half_right - full_right * 0.5).abs() < 1.0e-6,
            "right frame {frame}"
        );
        assert!(muted_left.abs() < 1.0e-7, "muted left frame {frame}");
        assert!(muted_right.abs() < 1.0e-7, "muted right frame {frame}");
    }
    assert!(saw_output);
}

fn assert_samples_match(actual: &mut SynthEngine, expected: &mut SynthEngine, frames: usize) {
    for frame in 0..frames {
        let (actual_left, actual_right) = actual.next_stereo_sample();
        let (expected_left, expected_right) = expected.next_stereo_sample();
        assert_eq!(
            actual_left.to_bits(),
            expected_left.to_bits(),
            "left frame {frame}"
        );
        assert_eq!(
            actual_right.to_bits(),
            expected_right.to_bits(),
            "right frame {frame}"
        );
    }
}

fn spread_sample_bus_engine(spread_pct: f32, mix_pct: f32, downstream_mono: bool) -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(spread_bus_config(
        Some(spread_pct),
        mix_pct,
        downstream_mono,
    ));
    engine.set_sample_banks(vec![sample_bank(vec![1.0, 0.25, -0.5, 0.75, 0.0])]);
    engine
}

fn spread_sample_bus_engine_without_spread_param() -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(spread_bus_config(None, 35.0, false));
    engine.set_sample_banks(vec![sample_bank(vec![1.0, 0.25, -0.5, 0.75, 0.0])]);
    engine
}

fn spread_sample_bus_engine_with_volume(
    volume_pct: f32,
    spread_pct: f32,
    mix_pct: f32,
) -> SynthEngine {
    let mut config = spread_bus_config(Some(spread_pct), mix_pct, false);
    if let Some(mixer) = config.mixer.as_mut() {
        mixer.buses[0].volume_pct = volume_pct;
    }
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(config);
    engine.set_sample_banks(vec![sample_bank(vec![1.0, 0.25, -0.5, 0.75, 0.0])]);
    engine
}

fn auto_pan_sample_bus_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![routed_sampler_slot()],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Config {
                    kind: "auto_pan".to_string(),
                    params: [
                        ("rateHz".to_string(), json!(20.0)),
                        ("depthPct".to_string(), json!(100.0)),
                    ]
                    .into_iter()
                    .collect::<BTreeMap<_, _>>(),
                }],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume_pct: 100.0,
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.set_sample_banks(vec![sample_bank(vec![1.0; 128])]);
    engine
}

fn spread_bus_config(
    spread_pct: Option<f32>,
    mix_pct: f32,
    downstream_mono: bool,
) -> InstrumentsConfig {
    let mut delay_params = BTreeMap::from([
        ("timeMs".to_string(), json!(1.0)),
        ("feedback".to_string(), json!(0.0)),
        ("mixPct".to_string(), json!(mix_pct)),
    ]);
    if let Some(spread_pct) = spread_pct {
        delay_params.insert("spreadPct".to_string(), json!(spread_pct));
    }
    let mut slots = vec![FxBusSlotConfig::Config {
        kind: "delay".to_string(),
        params: delay_params,
    }];
    if downstream_mono {
        slots.push(FxBusSlotConfig::Config {
            kind: "saturator".to_string(),
            params: BTreeMap::from([
                ("drive".to_string(), json!(1.0)),
                ("mixPct".to_string(), json!(100.0)),
            ]),
        });
    }
    InstrumentsConfig {
        instruments: vec![routed_sampler_slot()],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots,
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume_pct: 100.0,
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn routed_sampler_slot() -> InstrumentSlotConfig {
    InstrumentSlotConfig {
        kind: "sampler".to_string(),
        synth: default_synth_config(),
        mixer: Some(InstrumentMixerConfig {
            route: "fx_bus_1".to_string(),
            pan_pos: DEFAULT_PAN_POSITIONS / 2,
            volume: 100.0,
        }),
    }
}

fn sample_bank(samples: Vec<f32>) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(sample_buffer(samples)),
    };
    bank
}

fn sample_buffer(samples: Vec<f32>) -> SampleBuffer {
    SampleBuffer {
        samples: samples.into(),
        channels: 1,
        sample_rate: 48_000,
    }
}
