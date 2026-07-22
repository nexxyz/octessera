use super::*;
use crate::synth::{
    default_synth_config, FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig,
    InstrumentSlotConfig, MixerConfig, SampleBankConfig,
};
use serde_json::json;

#[test]
fn prepared_audio_apply_matches_canonical_audio() {
    let config = test_config();
    let mut canonical = SynthEngine::new(44_100);
    canonical.set_instruments(config.clone());
    let mut prepared = SynthEngine::new(44_100);
    prepared.apply_prepared_audio_config(prepare_audio_config(
        config,
        Some(vec![SampleBankConfig::default()]),
        None,
        44_100,
    ));
    canonical.note_on(0, 60, 100, 1_000);
    prepared.note_on(0, 60, 100, 1_000);
    for _ in 0..128 {
        assert_eq!(
            canonical.next_stereo_sample(),
            prepared.next_stereo_sample()
        );
    }
}

#[test]
fn prepared_apply_does_not_allocate_or_grow_callback_storage() {
    let config = test_config();
    let mut engine = SynthEngine::new(44_100);
    engine.apply_prepared_audio_config(prepare_audio_config(
        config.clone(),
        Some(vec![SampleBankConfig::default()]),
        None,
        44_100,
    ));
    let initial_capacities = capacities(&engine);
    let prepared = prepare_audio_config(
        config,
        Some(vec![SampleBankConfig::default()]),
        None,
        44_100,
    );
    let (_, allocations) = crate::synth::test_allocator::count(|| {
        engine.apply_prepared_audio_config(prepared);
    });
    assert_eq!(allocations, 0);
    assert_eq!(initial_capacities, capacities(&engine));
}

#[test]
fn prepared_momentary_start_fits_fixed_control_budget() {
    let mut engine = SynthEngine::new(44_100);
    for index in 0..2 {
        let prepared = prepare_momentary_fx_start(
            format!("fx-{index}"),
            "stutter".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Global,
            44_100,
        )
        .unwrap();
        engine.apply_prepared_momentary_fx_start(prepared);
    }
    assert_eq!(engine.momentary_fx.len(), 1);
    assert_eq!(engine.momentary_fx.capacity(), 2);
}

fn capacities(engine: &SynthEngine) -> (usize, usize, usize, usize, usize, usize) {
    (
        engine.bus_pan_pos.capacity(),
        engine.bus_slot_state.capacity(),
        engine.bus_mono_scratch.capacity(),
        engine.bus_mono_snapshot.capacity(),
        engine.master_slot_state.capacity(),
        engine.bus_output_spread_state.capacity(),
    )
}

fn test_config() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "fx_bus_1".into(),
                pan_pos: 16,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Config {
                    kind: "delay".into(),
                    params: [("timeMs".into(), json!(20.0))].into_iter().collect(),
                }],
                pan_pos: 16,
                volume_pct: 100.0,
            }],
            master: None,
        }),
        pan_positions: 33,
        master_volume: 100.0,
    }
}
