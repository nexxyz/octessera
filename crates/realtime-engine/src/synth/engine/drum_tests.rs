use super::super::*;
use crate::synth::{DrumConfig, DrumParamId, DrumSound, ScalarMutation};

pub(super) fn drum_slot(cfg: DrumConfig, route: &str) -> InstrumentSlotConfig {
    InstrumentSlotConfig {
        kind: "drum".into(),
        synth: default_synth_config(),
        fm: None,
        pluck: None,
        drum: Some(cfg),
        mixer: Some(InstrumentMixerConfig {
            route: route.into(),
            pan_pos: DEFAULT_PAN_POSITIONS / 2,
            volume: 100.0,
        }),
    }
}

pub(super) fn drum_engine(cfg: DrumConfig, sample_rate: u32) -> SynthEngine {
    let mut engine = SynthEngine::new(sample_rate);
    engine.set_instrument_slot(0, drum_slot(cfg, "direct"));
    engine
}

#[test]
fn drum_defaults_and_all_sounds_are_bounded_across_sample_rates_and_tunes() {
    let cfg = DrumConfig::default();
    assert!(cfg.assignments.is_empty());
    assert_eq!(cfg.voices.map(|voice| voice.sound), DrumSound::ALL);
    assert_eq!(
        cfg.voices.map(|voice| voice.decay_ms),
        [420.0, 220.0, 85.0, 650.0, 500.0, 320.0, 240.0, 95.0]
    );
    for rate in [44_100, 48_000] {
        for voice in 0..8 {
            for tune in [-24, 0, 24] {
                let mut engine = drum_engine(cfg.clone(), rate);
                engine.drum_hit(0, voice, tune, 120);
                let hit = engine.synth_voice_pool.lane(0).unwrap();
                assert!(hit.active && hit.drum_voice);
                assert!(hit.drum.inc1.is_finite() && hit.drum.inc1 > 0.0 && hit.drum.inc1 <= 0.45);
                let first = engine.next_sample();
                assert!(
                    first.is_finite() && first.abs() > 1e-9,
                    "{rate} {voice} {tune}"
                );
                for _ in 0..(rate / 2) {
                    let sample = engine.next_sample();
                    assert!(
                        sample.is_finite() && sample.abs() < 8.0,
                        "{rate} {voice} {tune}"
                    );
                }
            }
        }
    }
}

#[test]
fn drum_hits_coexist_note_off_does_not_choke_and_source_resets_on_steal() {
    let mut engine = drum_engine(DrumConfig::default(), 44_100);
    engine.drum_hit(0, 0, 0, 120);
    for _ in 0..32 {
        engine.next_sample();
    }
    engine.drum_hit(0, 1, 0, 120);
    assert_eq!(engine.active_voice_count_for_slot(0), 2);
    engine.note_off(0, 36);
    assert_eq!(engine.active_voice_count_for_slot(0), 2);
    assert!((0..512).any(|_| engine.next_sample().abs() > 1e-5));
    for _ in 0..(44_100 * 3) {
        engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
    engine.set_voice_stealing_mode(VoiceStealingMode::Fixed16);
    for _ in 0..MAX_SYNTH_VOICES_PER_SLOT {
        engine.drum_hit(0, 3, 0, 120);
    }
    for _ in 0..64 {
        engine.next_sample();
    }
    let before = engine.profile_snapshot().cumulative_voice_steals;
    engine.drum_hit(0, 7, 0, 120);
    assert_eq!(
        engine.profile_snapshot().cumulative_voice_steals,
        before + 1
    );
    let replaced = engine
        .synth_voice_pool
        .slot_lanes(0)
        .unwrap()
        .iter()
        .find_map(|lane| {
            engine
                .synth_voice_pool
                .lane(*lane)
                .filter(|voice| voice.drum.sound == DrumSound::Rim)
        })
        .unwrap();
    assert_eq!(replaced.drum.elapsed, 0);
}

#[test]
fn drum_next_hit_scalars_preserve_active_hit_and_other_kit_voices() {
    let mut engine = drum_engine(DrumConfig::default(), 44_100);
    engine.drum_hit(0, 2, 0, 120);
    for _ in 0..128 {
        engine.next_sample();
    }
    let old = *engine.synth_voice_pool.lane(0).unwrap();
    let other = engine.drum_voices[0][3];
    for (param, value) in [
        (DrumParamId::TuneSemis, 12.0),
        (DrumParamId::DecayMs, 900.0),
        (DrumParamId::TonePct, 15.0),
        (DrumParamId::AttackMs, 20.0),
    ] {
        assert_eq!(
            engine.set_drum_param_typed(0, 2, param, value),
            ScalarMutation::Changed
        );
    }
    let held = engine.synth_voice_pool.lane(0).unwrap();
    assert_eq!(held.canonical_lane, old.canonical_lane);
    assert_eq!(held.drum.elapsed, old.drum.elapsed);
    assert_eq!(
        held.drum.decay_step.to_bits(),
        old.drum.decay_step.to_bits()
    );
    assert_eq!(engine.drum_voices[0][3].decay_ms, other.decay_ms);
    engine.drum_hit(0, 2, 0, 120);
    let new_hit = engine.synth_voice_pool.lane(1).unwrap();
    assert_eq!(new_hit.drum.attack_frames, 882);
    assert!(new_hit.drum.decay_step > old.drum.decay_step);
    assert_eq!(engine.active_voice_count_for_slot(0), 2);
}

#[test]
fn drum_prepared_and_scalar_next_hit_controls_match_without_morphing_first_hit() {
    let cfg = DrumConfig::default();
    let mut prepared = drum_engine(cfg.clone(), 44_100);
    let mut live = drum_engine(cfg.clone(), 44_100);
    for engine in [&mut prepared, &mut live] {
        engine.drum_hit(0, 2, -12, 120);
        for _ in 0..128 {
            engine.next_sample();
        }
    }
    let previous = *live.synth_voice_pool.lane(0).unwrap();
    let mut patch = cfg;
    patch.voices[2].tune_semis = 12;
    patch.voices[2].decay_ms = 700.0;
    patch.voices[2].tone_pct = 20.0;
    patch.voices[2].attack_ms = 8.0;
    prepared.set_instrument_slot(0, drum_slot(patch, "direct"));
    for (param, value) in [
        (DrumParamId::TuneSemis, 12.0),
        (DrumParamId::DecayMs, 700.0),
        (DrumParamId::TonePct, 20.0),
        (DrumParamId::AttackMs, 8.0),
    ] {
        assert_eq!(
            live.set_drum_param_typed(0, 2, param, value),
            ScalarMutation::Changed
        );
    }
    for engine in [&prepared, &live] {
        let first = engine.synth_voice_pool.lane(0).unwrap();
        assert_eq!(first.drum.elapsed, previous.drum.elapsed);
        assert_eq!(first.drum.inc1.to_bits(), previous.drum.inc1.to_bits());
        assert_eq!(first.drum.phase1.to_bits(), previous.drum.phase1.to_bits());
        assert_eq!(first.canonical_lane, previous.canonical_lane);
    }
    for engine in [&mut prepared, &mut live] {
        engine.drum_hit(0, 2, 5, 100);
    }
    assert_eq!(
        prepared
            .synth_voice_pool
            .lane(1)
            .unwrap()
            .drum
            .inc1
            .to_bits(),
        live.synth_voice_pool.lane(1).unwrap().drum.inc1.to_bits()
    );
    for _ in 0..512 {
        assert_eq!(
            prepared.next_sample().to_bits(),
            live.next_sample().to_bits()
        );
    }
}

#[test]
fn drum_uses_common_filter_and_bus_and_live_slot_gain() {
    let mut low = DrumConfig::default();
    low.filter.cutoff_hz = 300.0;
    low.filter.kind = FilterType::Lowpass;
    let mut high = low.clone();
    high.filter.kind = FilterType::Highpass;
    let mut low_engine = drum_engine(low.clone(), 44_100);
    let mut high_engine = drum_engine(high, 44_100);
    for engine in [&mut low_engine, &mut high_engine] {
        engine.drum_hit(0, 1, 0, 120);
    }
    let difference: f32 = (0..512)
        .map(|_| (low_engine.next_sample() - high_engine.next_sample()).abs())
        .sum();
    assert!(difference > 0.01);
    let bus = MixerConfig {
        buses: vec![FxBusConfig {
            slots: vec![FxBusSlotConfig::Config {
                kind: "delay".into(),
                params: [
                    ("timeMs".into(), serde_json::json!(2.0)),
                    ("mixPct".into(), serde_json::json!(100.0)),
                ]
                .into_iter()
                .collect(),
            }],
            pan_pos: DEFAULT_PAN_POSITIONS / 2,
            volume_pct: 100.0,
        }],
        master: None,
    };
    let mut direct = SynthEngine::new(44_100);
    let mut routed = SynthEngine::new(44_100);
    for (engine, route) in [(&mut direct, "direct"), (&mut routed, "fx_bus_1")] {
        engine.set_instruments(InstrumentsConfig {
            instruments: vec![drum_slot(low.clone(), route)],
            mixer: Some(bus.clone()),
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        });
        engine.drum_hit(0, 1, 0, 127);
    }
    let first = direct.next_sample();
    assert!(first.abs() > 1e-9);
    assert!(routed.next_sample().abs() < first.abs() * 0.1);
    assert!((0..256).any(|_| routed.next_sample().abs() > 1e-7));
    assert_eq!(
        direct.set_drum_param_typed(0, 0, DrumParamId::AmpGainPct, 0.0),
        ScalarMutation::Changed
    );
    assert!(direct.synth_voice_pool.lane(0).unwrap().active);
    assert_eq!(direct.next_sample(), 0.0);
}
