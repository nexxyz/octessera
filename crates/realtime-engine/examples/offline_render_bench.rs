use realtime_engine::synth::{
    default_synth_config, FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig,
    InstrumentSlotConfig, InstrumentsConfig, MixerConfig, SynthEngine, DEFAULT_PAN_POSITIONS,
    INSTRUMENT_SLOT_COUNT,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::time::Instant;

const SAMPLE_RATE: u32 = 48_000;
const SECONDS: usize = 20;

fn main() {
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    engine.set_instruments(bench_config());

    let frames = SAMPLE_RATE as usize * SECONDS;
    let start = Instant::now();
    let mut checksum = 0.0_f32;

    for frame in 0..frames {
        if frame % 6_000 == 0 {
            let slot = ((frame / 6_000) % INSTRUMENT_SLOT_COUNT) as u8;
            let note = 48 + ((frame / 6_000) % 24) as u8;
            engine.note_on(slot, note, 96, 450);
        }
        let (left, right) = engine.next_stereo_sample();
        checksum += (left * 0.5 + right * 0.5).abs();
    }

    let elapsed = start.elapsed().as_secs_f64();
    let realtime = SECONDS as f64 / elapsed;
    let samples_per_second = frames as f64 / elapsed;
    println!("rendered_seconds={SECONDS}");
    println!("elapsed_seconds={elapsed:.4}");
    println!("realtime_ratio={realtime:.2}");
    println!("samples_per_second={samples_per_second:.0}");
    println!("checksum={checksum:.6}");
}

fn bench_config() -> InstrumentsConfig {
    let synth = default_synth_config();
    let instruments = (0..INSTRUMENT_SLOT_COUNT)
        .map(|idx| InstrumentSlotConfig {
            kind: "synth".to_string(),
            synth,
            mixer: Some(InstrumentMixerConfig {
                route: format!("fx_bus_{}", (idx % 4) + 1),
                pan_pos: idx % DEFAULT_PAN_POSITIONS,
                volume: 100.0,
            }),
        })
        .collect();

    InstrumentsConfig {
        instruments,
        mixer: Some(MixerConfig {
            buses: vec![
                bus("delay", [("timeMs", json!(180.0)), ("mixPct", json!(25.0))]),
                bus("chorus", [("rateHz", json!(0.7)), ("mixPct", json!(35.0))]),
                bus(
                    "filter_lfo",
                    [("rateHz", json!(0.4)), ("depthPct", json!(55.0))],
                ),
                bus(
                    "saturator",
                    [("drive", json!(1.8)), ("mixPct", json!(45.0))],
                ),
            ],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn bus<const N: usize>(kind: &str, params: [(&str, serde_json::Value); N]) -> FxBusConfig {
    FxBusConfig {
        slots: vec![FxBusSlotConfig::Config {
            kind: kind.to_string(),
            params: params
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect::<BTreeMap<_, _>>(),
        }],
        pan_pos: DEFAULT_PAN_POSITIONS / 2,
    }
}
