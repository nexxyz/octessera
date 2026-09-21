use super::*;
use crate::synth::fx_params::{DuckSource, DuckSourceTap};
use crate::synth::{BusIdleThreshold, WorkerWarningThreshold};

#[test]
fn dsp_config_is_dynamic_and_does_not_change_render_plan() {
    let mut engine = SynthEngine::new(48_000);
    let generation = engine.render_plan.generation;
    let config = DspRuntimeConfig {
        worker_warning_threshold: WorkerWarningThreshold::Percent95,
        bus_idle_threshold: BusIdleThreshold::Exact,
    };

    engine.set_dsp_config(config);

    assert_eq!(engine.dsp_config(), config);
    assert_eq!(engine.render_plan.generation, generation);
}

#[test]
fn dynamic_fx_bus_slot_accepts_third_slot_and_ignores_fourth() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: Vec::new(),
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig::default()],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });

    engine.set_fx_bus_slot(0, 2, "tremolo".into(), BTreeMap::new());
    assert_eq!(engine.bus_chains[0].active_slot_count, 1);
    assert!(matches!(
        engine.bus_chains[0].slot_params[2],
        FxBusParams::Tremolo { .. }
    ));

    engine.set_fx_bus_slot(0, 3, "delay".into(), BTreeMap::new());
    assert_eq!(engine.bus_chains[0].active_slot_count, 1);
}

#[test]
fn master_fx_config_ignores_third_slot() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: Vec::new(),
        mixer: Some(MixerConfig {
            buses: Vec::new(),
            master: Some(MasterFxConfig {
                slots: vec![
                    FxBusSlotConfig::Kind("none".into()),
                    FxBusSlotConfig::Kind("none".into()),
                    FxBusSlotConfig::Kind("tremolo".into()),
                ],
            }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });

    assert_eq!(engine.master_slot_params.len(), GLOBAL_FX_SLOT_COUNT);
    assert!(engine.master_active_slot_indices.is_empty());
}

#[test]
fn dynamic_render_plan_tracks_fx_structure_not_parameters() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: Vec::new(),
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Config {
                    kind: "delay".into(),
                    params: BTreeMap::new(),
                }],
                ..FxBusConfig::default()
            }],
            master: Some(MasterFxConfig {
                slots: vec![FxBusSlotConfig::Kind("compressor".into())],
            }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let initial_generation = engine.render_plan.generation;

    engine.set_fx_bus_slot(
        0,
        0,
        "delay".into(),
        [("timeMs".into(), serde_json::json!(400.0))]
            .into_iter()
            .collect(),
    );
    engine.set_global_fx_slot(
        0,
        "compressor".into(),
        [("thresholdDb".into(), serde_json::json!(-8.0))]
            .into_iter()
            .collect(),
    );
    assert_eq!(engine.render_plan.generation, initial_generation);

    engine.set_fx_bus_slot(0, 0, "reverb".into(), BTreeMap::new());
    let changed_generation = engine.render_plan.generation;
    assert!(changed_generation > initial_generation);

    engine.set_global_fx_slot(
        0,
        "duck".into(),
        [("source".into(), serde_json::json!("I1"))]
            .into_iter()
            .collect(),
    );
    let duck_generation = engine.render_plan.generation;
    engine.set_global_fx_slot(
        0,
        "duck".into(),
        [
            ("source".into(), serde_json::json!("I1")),
            ("amountPct".into(), serde_json::json!(25.0)),
        ]
        .into_iter()
        .collect(),
    );
    assert_eq!(engine.render_plan.generation, duck_generation);
    engine.set_global_fx_slot(
        0,
        "duck".into(),
        [("source".into(), serde_json::json!("B1"))]
            .into_iter()
            .collect(),
    );
    assert!(engine.render_plan.generation > duck_generation);
}

#[test]
fn duck_source_tap_change_preserves_plan_generation_and_state() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: Vec::new(),
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Config {
                    kind: "duck".into(),
                    params: [
                        ("source".into(), serde_json::json!("I1")),
                        ("sourceTap".into(), serde_json::json!("pre")),
                    ]
                    .into_iter()
                    .collect(),
                }],
                ..FxBusConfig::default()
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let initial_plan = engine.render_plan.clone();
    engine.bus_chains[0].slot_state[0] = FxBusState::Duck { env: 0.37 };

    engine.set_fx_bus_slot(
        0,
        0,
        "duck".into(),
        [
            ("source".into(), serde_json::json!("I1")),
            ("sourceTap".into(), serde_json::json!("post")),
        ]
        .into_iter()
        .collect(),
    );

    assert_eq!(engine.render_plan, initial_plan);
    assert_eq!(engine.bus_chains[0].active_slot_count, 1);
    assert!(matches!(
        engine.bus_chains[0].slot_params[0],
        FxBusParams::Duck {
            source: DuckSource::Instrument(0),
            source_tap: DuckSourceTap::Post,
            ..
        }
    ));
    assert!(matches!(
        engine.bus_chains[0].slot_state[0],
        FxBusState::Duck { env } if env.to_bits() == 0.37_f32.to_bits()
    ));
}
