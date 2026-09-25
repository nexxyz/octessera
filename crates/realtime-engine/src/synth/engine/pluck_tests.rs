use super::super::*;
use super::VoiceSource;
use crate::synth::{PluckConfig, PluckParamId, ScalarMutation};

fn pluck_slot(cfg: PluckConfig, route: &str) -> InstrumentSlotConfig {
    InstrumentSlotConfig {
        kind: "pluck".into(),
        synth: default_synth_config(),
        fm: None,
        pluck: Some(cfg),
        drum: None,
        mixer: Some(InstrumentMixerConfig {
            route: route.into(),
            pan_pos: DEFAULT_PAN_POSITIONS / 2,
            volume: 100.0,
        }),
    }
}

pub(super) fn pluck_engine(cfg: PluckConfig, sample_rate: u32) -> SynthEngine {
    let mut engine = SynthEngine::new(sample_rate);
    engine.set_instrument_slot(0, pluck_slot(cfg, "direct"));
    engine
}

#[test]
fn pluck_starts_immediately_across_midi_range_with_safe_full_delay_ring() {
    for sample_rate in [44_100, 48_000] {
        for note in 0_u8..=127 {
            let mut engine = pluck_engine(PluckConfig::default(), sample_rate);
            engine.note_on(0, note, 127, 10_000);
            let voice = engine.synth_voice_pool.lane(0).unwrap();
            assert!(voice.pluck_voice);
            assert!((3..8192).contains(&voice.pluck.delay));
            let delay = voice.pluck.delay;
            let first = engine.next_sample();
            assert!(first.is_finite() && first.abs() > 1e-9, "note {note}");
            for _ in 0..(delay * 3 + 100) {
                let sample = engine.next_sample();
                assert!(sample.is_finite() && sample.abs() < 8.0, "note {note}");
            }
        }
    }
}

#[test]
fn pluck_delay_tracks_tuned_sample_frames_at_both_rates() {
    for sample_rate in [44_100_u32, 48_000] {
        for note in 0_u8..=127 {
            let mut engine = pluck_engine(PluckConfig::default(), sample_rate);
            engine.note_on(0, note, 120, 10_000);
            let voice = engine.synth_voice_pool.lane(0).unwrap();
            let period = sample_rate as f32 / voice.freq_hz;
            let delay = voice.pluck.delay as f32 + voice.pluck.fraction;
            assert!((delay - (period - 0.5)).abs() < 0.001, "note {note}");
            let length = voice.pluck.delay * 3 + 512;
            let samples = (0..length)
                .map(|_| engine.next_sample())
                .collect::<Vec<_>>();
            let lag = period.round() as usize;
            let start = lag + 8;
            let count = 256;
            let correlation = (start..start + count)
                .map(|index| samples[index] * samples[index + lag])
                .sum::<f32>();
            assert!(
                correlation.is_finite() && correlation > 0.0,
                "note {note}: {correlation}"
            );
        }
    }
}

#[test]
fn pluck_held_live_coefficients_and_next_pick_do_not_restart_ring() {
    let mut engine = pluck_engine(PluckConfig::default(), 44_100);
    engine.note_on(0, 69, 100, 10_000);
    for _ in 0..300 {
        engine.next_sample();
    }
    let held = *engine.synth_voice_pool.lane(0).unwrap();
    let VoiceSource::Pluck {
        decay_ms,
        brightness_pct,
        pick_position_pct,
    } = engine.synth_render_configs[0].source
    else {
        panic!("expected Plucked source");
    };
    assert_eq!(
        (decay_ms, brightness_pct, pick_position_pct),
        (1500.0, 65.0, 25.0)
    );
    for (id, value) in [
        (PluckParamId::DecayMs, 200.0),
        (PluckParamId::BrightnessPct, 10.0),
        (PluckParamId::PickPositionPct, 40.0),
    ] {
        assert_eq!(
            engine.set_pluck_param_typed(0, id, value),
            ScalarMutation::Changed
        );
    }
    let before = engine.synth_voice_pool.lane(0).unwrap();
    assert_eq!(before.pluck.write, held.pluck.write);
    assert_eq!(before.pluck.emitted, held.pluck.emitted);
    assert_eq!(before.pluck.pick_offset, held.pluck.pick_offset);
    assert_eq!(before.canonical_lane, held.canonical_lane);
    engine.next_sample();
    let after = engine.synth_voice_pool.lane(0).unwrap();
    assert_eq!(
        after.pluck.write,
        (held.pluck.write + 1) % crate::synth::pluck_string::RING_LEN
    );
    assert!(after.pluck.loss < held.pluck.loss);
    assert!(after.pluck.brightness < held.pluck.brightness);
    assert_eq!(after.pluck.pick_offset, held.pluck.pick_offset);
    engine.note_on(0, 69, 100, 10_000);
    let new_voice = engine.synth_voice_pool.lane(1).unwrap();
    assert_ne!(new_voice.pluck.pick_offset, held.pluck.pick_offset);
}

#[test]
fn pluck_retrigger_does_not_read_old_ring_and_release_keeps_a_tail() {
    let cfg = PluckConfig::default();
    let mut used = pluck_engine(cfg, 44_100);
    used.note_on(0, 48, 127, 10_000);
    for _ in 0..3_000 {
        used.next_sample();
    }
    used.all_notes_off();
    for _ in 0..44_100 {
        used.next_sample();
    }
    assert_eq!(used.active_voice_count_for_slot(0), 0);
    let mut fresh = pluck_engine(cfg, 44_100);
    for engine in [&mut used, &mut fresh] {
        engine.note_on(0, 69, 100, 10_000);
    }
    for frame in 0..512 {
        assert_eq!(
            used.next_sample().to_bits(),
            fresh.next_sample().to_bits(),
            "frame {frame}"
        );
    }
    used.note_off(0, 69);
    let first_release = used.next_sample();
    assert!(first_release.is_finite());
    assert!(used.synth_voice_pool.lane(0).unwrap().active);
    assert!((0..(44_100 / 10)).any(|_| used.next_sample().abs() > 1e-6));
    for _ in 0..44_100 {
        used.next_sample();
    }
    assert_eq!(used.active_voice_count_for_slot(0), 0);
}

#[test]
fn pluck_short_one_shot_keeps_a_bounded_amp_release() {
    let mut engine = pluck_engine(PluckConfig::default(), 44_100);
    engine.note_on(0, 69, 100, 10);
    for _ in 0..(44_100 / 50) {
        engine.next_sample();
    }
    assert!(engine.synth_voice_pool.lane(0).unwrap().active);
    assert!((0..(44_100 / 10)).any(|_| engine.next_sample().abs() > 1e-6));
    for _ in 0..44_100 {
        engine.next_sample();
    }
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
}

#[test]
fn pluck_type_switch_retires_the_old_string_voice() {
    let cfg = PluckConfig::default();
    let mut engine = pluck_engine(cfg, 44_100);
    engine.note_on(0, 69, 127, 10_000);
    for _ in 0..128 {
        engine.next_sample();
    }
    let mut synth = pluck_slot(cfg, "direct");
    synth.kind = "synth".into();
    synth.pluck = None;
    engine.set_instrument_slot(0, synth);
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
    engine.note_on(0, 69, 127, 10_000);
    assert!(!engine.synth_voice_pool.lane(0).unwrap().pluck_voice);
    engine.set_instrument_slot(0, pluck_slot(cfg, "direct"));
    assert_eq!(engine.active_voice_count_for_slot(0), 0);
}

#[test]
fn pluck_string_controls_affect_audio_without_unbounded_output() {
    let defaults = PluckConfig::default();
    for (field, changed) in [
        (
            "decay",
            PluckConfig {
                decay_ms: 150.0,
                ..defaults
            },
        ),
        (
            "brightness",
            PluckConfig {
                brightness_pct: 0.0,
                ..defaults
            },
        ),
        (
            "pick",
            PluckConfig {
                pick_position_pct: 50.0,
                ..defaults
            },
        ),
    ] {
        let mut original = pluck_engine(defaults, 44_100);
        let mut edited = pluck_engine(changed, 44_100);
        for engine in [&mut original, &mut edited] {
            engine.note_on(0, 69, 127, 10_000);
        }
        let mut difference = 0.0;
        for _ in 0..2_048 {
            let a = original.next_sample();
            let b = edited.next_sample();
            assert!(a.is_finite() && b.is_finite() && a.abs() < 4.0 && b.abs() < 4.0);
            difference += (a - b).abs();
        }
        assert!(difference > 0.05, "{field}: {difference}");
    }
}

#[test]
fn pluck_prepared_and_live_controls_match_with_held_ring_continuity() {
    let cfg = PluckConfig::default();
    for (param, value) in [
        (PluckParamId::DecayMs, 220.0),
        (PluckParamId::BrightnessPct, 10.0),
        (PluckParamId::PickPositionPct, 40.0),
    ] {
        let mut prepared = pluck_engine(cfg, 44_100);
        let mut live = pluck_engine(cfg, 44_100);
        for engine in [&mut prepared, &mut live] {
            engine.note_on(0, 69, 120, 10_000);
            for _ in 0..500 {
                engine.next_sample();
            }
        }
        let before = *live.synth_voice_pool.lane(0).unwrap();
        let mut patch = cfg;
        match param {
            PluckParamId::DecayMs => patch.decay_ms = value,
            PluckParamId::BrightnessPct => patch.brightness_pct = value,
            PluckParamId::PickPositionPct => patch.pick_position_pct = value,
            _ => unreachable!(),
        }
        let _ = prepared.apply_prepared_instrument_slot(
            0,
            prepare_instrument_slot_config(pluck_slot(patch, "direct")),
        );
        assert_eq!(
            live.set_pluck_param_typed(0, param, value),
            ScalarMutation::Changed
        );
        for engine in [&prepared, &live] {
            let held = engine.synth_voice_pool.lane(0).unwrap();
            assert_eq!(held.pluck.write, before.pluck.write);
            assert_eq!(held.pluck.emitted, before.pluck.emitted);
            assert_eq!(held.pluck.pick_offset, before.pluck.pick_offset);
            assert_eq!(held.canonical_lane, before.canonical_lane);
        }
        for _ in 0..512 {
            assert_eq!(
                prepared.next_sample().to_bits(),
                live.next_sample().to_bits()
            );
        }
    }
}

#[test]
fn pluck_stolen_ring_does_not_leak_prior_voice_samples() {
    let cfg = PluckConfig::default();
    let mut reused = pluck_engine(cfg, 44_100);
    reused.set_voice_stealing_mode(VoiceStealingMode::Fixed16);
    for note in 60..(60 + MAX_SYNTH_VOICES_PER_SLOT as u8) {
        reused.note_on(0, note, 120, 10_000);
    }
    for _ in 0..1_000 {
        reused.next_sample();
    }
    let ((), allocations, _) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            reused.note_on(0, 80, 120, 10_000);
        });
    assert_eq!(allocations, 0);
    assert!(reused.profile_snapshot().cumulative_voice_steals >= 1);
    let victim_lane = reused
        .synth_voice_pool
        .slot_lanes(0)
        .unwrap()
        .iter()
        .copied()
        .find(|lane| reused.synth_voice_pool.lane(*lane).unwrap().midi_note == 80)
        .unwrap();
    let mut fresh = pluck_engine(cfg, 44_100);
    fresh.note_on(0, 80, 120, 10_000);
    for frame in 0..256 {
        let a = reused
            .synth_voice_pool
            .lane_and_ring_mut(victim_lane)
            .unwrap();
        let b = fresh.synth_voice_pool.lane_and_ring_mut(0).unwrap();
        assert_eq!(
            a.0.pluck.next(a.1).to_bits(),
            b.0.pluck.next(b.1).to_bits(),
            "frame {frame}"
        );
    }
}

#[test]
fn pluck_uses_common_filter_key_tracking_and_bus_fx_route() {
    let mut low = PluckConfig::default();
    low.filter.cutoff_hz = 300.0;
    low.filter.kind = FilterType::Lowpass;
    low.filter.key_tracking_pct = 100.0;
    let mut high = low;
    high.filter.kind = FilterType::Highpass;
    let mut low_engine = pluck_engine(low, 44_100);
    let mut high_engine = pluck_engine(high, 44_100);
    for engine in [&mut low_engine, &mut high_engine] {
        engine.note_on(0, 72, 127, 1_000);
        assert_eq!(
            engine.synth_voice_pool.lane(0).unwrap().filter_key_scale,
            2.0
        );
    }
    let difference: f32 = (0..512)
        .map(|_| (low_engine.next_sample() - high_engine.next_sample()).abs())
        .sum();
    assert!(difference > 0.1);
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
            instruments: vec![pluck_slot(low, route)],
            mixer: Some(bus.clone()),
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        });
        engine.note_on(0, 72, 127, 1_000);
    }
    let first = direct.next_sample();
    assert!(first.abs() > 1e-9);
    assert!(routed.next_sample().abs() < first.abs() * 0.1);
    assert!((0..256).any(|_| routed.next_sample().abs() > 1e-6));

    let mut static_voice = pluck_engine(PluckConfig::default(), 44_100);
    static_voice.note_on(0, 69, 100, 1_000);
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut output = Vec::new();
    reset_prepare_count_for_test();
    static_voice.render_interleaved_block(128, &mut left, &mut right, &mut output);
    assert_eq!(prepare_count_for_test(), 1);
}
