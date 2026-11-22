use super::super::fx::FxBusState;
use super::super::fx_param::FxParamId;
use super::super::types::FilterType;
use super::*;
use crate::synth::{
    default_synth_config, FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig,
    InstrumentSlotConfig, InstrumentsConfig, MixerConfig, MixerRoute, DEFAULT_PAN_POSITIONS,
};
use serde_json::json;

const SAMPLE_RATE: u32 = 48_000;

fn config(time_ms: f32, cutoff_hz: f32) -> InstrumentsConfig {
    let mut synth = default_synth_config();
    synth.filter.cutoff_hz = cutoff_hz;
    synth.filter.kind = FilterType::Lowpass;
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "synth".into(),
            synth,
            mixer: Some(InstrumentMixerConfig {
                route: MixerRoute::FxBus(0),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Config {
                    kind: "delay".into(),
                    params: [("timeMs".to_string(), json!(time_ms))].into(),
                }],
                ..FxBusConfig::default()
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn read_head(engine: &SynthEngine) -> f32 {
    match &engine.bus_mix.chains[0].slot_state[0] {
        FxBusState::Delay { cache, .. } => cache.current_samples(),
        _ => panic!("delay state"),
    }
}

fn render(engine: &mut SynthEngine, frames: usize) {
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    engine.render_interleaved_block(frames, &mut left, &mut right, &mut out);
}

#[test]
fn a_synth_edit_during_a_delay_time_glide_leaves_the_read_head_gliding() {
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    engine.set_instruments(config(100.0, 2_000.0));
    engine.note_on(0, 60, 100, 1_000);
    render(&mut engine, 256);
    assert_eq!(read_head(&engine), 4_800.0);
    engine.set_fx_bus_param(0, 0, FxParamId::TimeMs, 300.0);
    render(&mut engine, 256);
    let gliding = read_head(&engine);
    assert!(gliding > 4_800.0 && gliding < 14_000.0, "{gliding}");

    engine.set_instruments(config(300.0, 1_200.0));
    assert_eq!(
        read_head(&engine),
        gliding,
        "the edit leaves the glide alone"
    );
    render(&mut engine, 256);
    assert!(read_head(&engine) > gliding && read_head(&engine) < 14_400.0);
}

#[test]
fn a_load_with_another_delay_time_lands_the_read_head_on_it() {
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    engine.set_instruments(config(100.0, 2_000.0));
    render(&mut engine, 256);
    engine.set_instruments(config(300.0, 2_000.0));
    assert!((read_head(&engine) - 14_400.0).abs() < 0.01);
}
