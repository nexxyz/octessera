use super::*;
use crate::synth::fx_param::{FxParamId, FxParamMutation};

/// 10 ms at 48 kHz.
const GLIDE_FRAMES: usize = 480;

fn fx_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: MixerRoute::FxBus(0),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Kind("delay".into())],
                ..FxBusConfig::default()
            }],
            master: Some(MasterFxConfig {
                slots: vec![FxBusSlotConfig::Kind("compressor".into())],
            }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.note_on(0, 60, 100, 10_000);
    engine
}

fn render(engine: &mut SynthEngine, frames: usize) {
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    engine.render_interleaved_block(frames, &mut left, &mut right, &mut out);
}

fn bus_mix(engine: &SynthEngine) -> f32 {
    match engine.bus_chains[0].slot_params[0] {
        FxBusParams::Delay { mix, .. } => mix,
        _ => panic!("delay"),
    }
}

fn makeup_db(engine: &SynthEngine) -> f32 {
    match engine.master_slot_params[0] {
        FxBusParams::Compressor { makeup_db, .. } => makeup_db,
        _ => panic!("compressor"),
    }
}

#[test]
fn a_live_bus_mix_glides_over_the_frames_the_bus_processes() {
    let mut engine = fx_engine();
    render(&mut engine, 64);
    let start = bus_mix(&engine);
    assert_eq!(
        engine.set_fx_bus_param(0, 0, FxParamId::MixPct, 95.0),
        FxParamMutation::Changed
    );
    assert_eq!(bus_mix(&engine), start);
    render(&mut engine, GLIDE_FRAMES / 2);
    assert!((bus_mix(&engine) - (start + 0.95) / 2.0).abs() < 1.0e-5);
    render(&mut engine, GLIDE_FRAMES / 2);
    assert_eq!(bus_mix(&engine), 0.95);
}

#[test]
fn a_bus_slot_config_drops_a_running_glide_and_keeps_its_own_values() {
    let mut engine = fx_engine();
    engine.set_fx_bus_param(0, 0, FxParamId::MixPct, 95.0);
    render(&mut engine, GLIDE_FRAMES / 4);
    engine.set_fx_bus_slot(
        0,
        0,
        "delay".into(),
        [("mixPct".to_string(), serde_json::json!(10))]
            .into_iter()
            .collect(),
    );
    assert_eq!(bus_mix(&engine), 0.1);
    render(&mut engine, GLIDE_FRAMES);
    assert_eq!(bus_mix(&engine), 0.1);
}

#[test]
fn a_live_master_makeup_glides_and_a_master_slot_config_drops_it() {
    let mut engine = fx_engine();
    render(&mut engine, 64);
    let start = makeup_db(&engine);
    let target = start + 12.0;
    engine.set_global_fx_param(0, FxParamId::MakeupDb, target);
    render(&mut engine, GLIDE_FRAMES / 4);
    assert!((makeup_db(&engine) - (start + 3.0)).abs() < 1.0e-4);
    render(&mut engine, GLIDE_FRAMES);
    assert_eq!(makeup_db(&engine), target);

    engine.set_global_fx_param(0, FxParamId::MakeupDb, start);
    render(&mut engine, GLIDE_FRAMES / 4);
    engine.set_global_fx_slot(
        0,
        "compressor".into(),
        [("makeupDb".to_string(), serde_json::json!(2))]
            .into_iter()
            .collect(),
    );
    assert_eq!(makeup_db(&engine), 2.0);
    render(&mut engine, GLIDE_FRAMES);
    assert_eq!(makeup_db(&engine), 2.0);
}
