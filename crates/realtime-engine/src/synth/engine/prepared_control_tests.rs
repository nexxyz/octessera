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
    let (retired, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            engine.apply_prepared_audio_config(prepared)
        });
    drop(retired);
    assert_eq!(allocations, 0);
    assert_eq!(deallocations, 0);
    assert_eq!(initial_capacities, capacities(&engine));
}

#[test]
fn prepared_single_slot_apply_matches_canonical_single_slot_apply() {
    const SLOT: usize = 2;
    let slot = test_slot("synth", true);
    let mut canonical = single_slot_engine();
    let mut prepared = single_slot_engine();
    canonical.synth_render_revisions[SLOT] = u32::MAX;
    prepared.synth_render_revisions[SLOT] = u32::MAX;

    canonical.set_instrument_slot(SLOT, slot.clone());
    prepared.apply_prepared_instrument_slot(SLOT, prepare_instrument_slot_config(slot));

    assert_eq!(canonical.slot_kind, prepared.slot_kind);
    assert_eq!(canonical.slot_kind[SLOT], InstrumentKind::Synth);
    assert_synth_configs_equal(&canonical.instruments[SLOT], &prepared.instruments[SLOT]);
    assert_eq!(canonical.slot_route, prepared.slot_route);
    assert_eq!(canonical.slot_pan_pos, prepared.slot_pan_pos);
    assert_eq!(canonical.slot_volume, prepared.slot_volume);
    assert_eq!(canonical.slot_pan_gains, prepared.slot_pan_gains);
    assert_eq!(canonical.slot_route[SLOT], 1);
    assert_eq!(canonical.slot_pan_pos[SLOT], 8);
    assert_eq!(canonical.slot_volume[SLOT], 0.375);
    assert_eq!(
        canonical.synth_render_revisions,
        prepared.synth_render_revisions
    );
    assert_eq!(canonical.synth_render_revisions[SLOT], 0);
    assert_eq!(
        canonical.routed_bus_slot_count,
        prepared.routed_bus_slot_count
    );
    assert_eq!(canonical.routed_bus_slot_count, 1);
}

#[test]
fn prepared_single_non_synth_slot_apply_preserves_partial_state_and_voices() {
    const SLOT: usize = 1;
    let initial = test_slot("synth", true);
    let mut next = test_slot("sampler", false);
    next.synth.filter.cutoff_hz = 320.0;
    let mut canonical = single_slot_engine();
    let mut prepared = single_slot_engine();
    canonical.set_instrument_slot(SLOT, initial.clone());
    prepared.apply_prepared_instrument_slot(SLOT, prepare_instrument_slot_config(initial));
    canonical.note_on(SLOT as u8, 60, 100, 1_000);
    prepared.note_on(SLOT as u8, 60, 100, 1_000);
    let canonical_synth = canonical.instruments[SLOT];
    let prepared_synth = prepared.instruments[SLOT];
    let canonical_revision = canonical.synth_render_revisions[SLOT];
    let prepared_revision = prepared.synth_render_revisions[SLOT];

    canonical.set_instrument_slot(SLOT, next.clone());
    prepared.apply_prepared_instrument_slot(SLOT, prepare_instrument_slot_config(next));

    assert_eq!(canonical.slot_kind, prepared.slot_kind);
    assert_eq!(canonical.slot_kind[SLOT], InstrumentKind::Sample);
    assert_synth_configs_equal(&canonical_synth, &canonical.instruments[SLOT]);
    assert_synth_configs_equal(&prepared_synth, &prepared.instruments[SLOT]);
    assert_synth_configs_equal(&canonical.instruments[SLOT], &prepared.instruments[SLOT]);
    assert_eq!(canonical.slot_route, prepared.slot_route);
    assert_eq!(canonical.slot_pan_pos, prepared.slot_pan_pos);
    assert_eq!(canonical.slot_volume, prepared.slot_volume);
    assert_eq!(canonical.slot_pan_gains, prepared.slot_pan_gains);
    assert_eq!(
        canonical.synth_render_revisions,
        prepared.synth_render_revisions
    );
    assert_eq!(canonical.synth_render_revisions[SLOT], canonical_revision);
    assert_eq!(prepared.synth_render_revisions[SLOT], prepared_revision);
    assert_eq!(canonical.active_voice_count_for_slot(SLOT), 1);
    assert_eq!(prepared.active_voice_count_for_slot(SLOT), 1);
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

fn single_slot_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(InstrumentsConfig {
        instruments: Vec::new(),
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig::default()],
            master: None,
        }),
        pan_positions: 9,
        master_volume: 100.0,
    });
    engine
}

fn test_slot(kind: &str, with_mixer: bool) -> InstrumentSlotConfig {
    let mut synth = crate::synth::default_synth_config();
    synth.osc1.waveform = WaveformId::Triangle;
    synth.amp.gain_pct = 42.0;
    InstrumentSlotConfig {
        kind: kind.into(),
        synth,
        mixer: with_mixer.then_some(InstrumentMixerConfig {
            route: "fx_bus_1".into(),
            pan_pos: 20,
            volume: 37.5,
        }),
    }
}

fn assert_synth_configs_equal(left: &crate::synth::SynthConfig, right: &crate::synth::SynthConfig) {
    assert_eq!(
        serde_json::to_value(left).unwrap(),
        serde_json::to_value(right).unwrap()
    );
}
