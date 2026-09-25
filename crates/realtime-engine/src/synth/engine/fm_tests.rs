use super::super::*;
use super::VoiceSource;
use crate::synth::{FmParamId, ScalarMutation, SynthParamId};

fn fm_slot(fm: FmConfig, route: &str) -> InstrumentSlotConfig {
    InstrumentSlotConfig {
        kind: "fm".into(),
        synth: default_synth_config(),
        fm: Some(fm),
        pluck: None,
        drum: None,
        mixer: Some(InstrumentMixerConfig {
            route: route.into(),
            pan_pos: DEFAULT_PAN_POSITIONS / 2,
            volume: 100.0,
        }),
    }
}

fn fm_engine(fm: FmConfig, route: &str, mixer: Option<MixerConfig>) -> SynthEngine {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![fm_slot(fm, route)],
        mixer,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine
}

#[test]
fn held_fm_index_scalar_matches_prepared_patch_depth_and_audio() {
    let mut fm = FmConfig::default();
    fm.index_env.attack_ms = 0.0;
    fm.index_env.decay_ms = 0.0;
    fm.index_env.sustain_pct = 100.0;
    fm.amp_env.attack_ms = 0.0;
    fm.amp_env.decay_ms = 0.0;
    fm.amp_env.sustain_pct = 100.0;
    let mut outputs = Vec::new();
    for value in [0_u8, 25, 50, 51, 100] {
        let mut prepared = fm_engine(fm, "direct", None);
        let mut live = fm_engine(fm, "direct", None);
        for engine in [&mut prepared, &mut live] {
            engine.note_on(0, 69, 127, 10_000);
            for _ in 0..128 {
                engine.next_sample();
            }
        }
        let before = *live.synth_voice_pool.lane(0).unwrap();
        let mut patched = fm;
        patched.index = value;
        let _ = prepared.apply_prepared_instrument_slot(
            0,
            prepare_instrument_slot_config(fm_slot(patched, "direct")),
        );
        let mutation = live.set_fm_param_typed(0, FmParamId::Index, value as f32);
        assert_eq!(
            mutation,
            if value == fm.index {
                ScalarMutation::Unchanged
            } else {
                ScalarMutation::Changed
            }
        );
        let VoiceSource::Fm {
            index: live_depth, ..
        } = live.synth_render_configs[0].source
        else {
            panic!("expected FM source");
        };
        let VoiceSource::Fm {
            index: prepared_depth,
            ..
        } = prepared.synth_render_configs[0].source
        else {
            panic!("expected prepared FM source");
        };
        assert_eq!(live_depth.to_bits(), prepared_depth.to_bits());
        assert_eq!(live_depth.to_bits(), (value as f32 * 0.04).to_bits());
        for engine in [&prepared, &live] {
            let held = engine.synth_voice_pool.lane(0).unwrap();
            assert!(held.active && held.fm);
            assert_eq!(held.canonical_lane, before.canonical_lane);
            assert_eq!(held.started_sample, before.started_sample);
            assert_eq!(
                (held.phase1.to_bits(), held.phase2.to_bits()),
                (before.phase1.to_bits(), before.phase2.to_bits())
            );
            assert_eq!(held.amp_env.stage_pos, before.amp_env.stage_pos);
            assert_eq!(held.index_env.stage_pos, before.index_env.stage_pos);
            assert_eq!(held.fm_index_limit, 4.0);
        }
        let mut rendered = Vec::with_capacity(256);
        for _ in 0..256 {
            let expected = prepared.next_sample();
            let actual = live.next_sample();
            assert_eq!(actual.to_bits(), expected.to_bits(), "FM index {value}");
            rendered.push(actual);
        }
        outputs.push(rendered);
    }
    for pair in outputs.windows(2) {
        let delta: f32 = pair[0]
            .iter()
            .zip(&pair[1])
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(
            delta > 0.01,
            "index values produced identical midrange output: {delta}"
        );
    }
}

#[test]
fn fm_frequency_ratio_and_modulation_depth_follow_their_envelopes() {
    for ratio in [FmRatio::Half, FmRatio::One, FmRatio::Two, FmRatio::Eight] {
        let mut fm = FmConfig {
            ratio,
            index: 100,
            ..FmConfig::default()
        };
        fm.index_env.attack_ms = 0.0;
        fm.index_env.decay_ms = 0.0;
        fm.index_env.sustain_pct = 100.0;
        fm.amp_env.attack_ms = 0.0;
        fm.amp_env.decay_ms = 0.0;
        fm.amp_env.sustain_pct = 100.0;
        let mut engine = fm_engine(fm, "direct", None);
        engine.note_on(0, 69, 127, 10_000);
        let voice = engine.synth_voice_pool.lane(0).unwrap();
        assert!((voice.osc1_inc - 440.0 / 44_100.0).abs() < 1e-7);
        assert!((voice.osc2_inc - 440.0 * ratio.value() / 44_100.0).abs() < 1e-7);
        assert_eq!(voice.fm_index_limit, 4.0);
        let first = engine.next_sample();
        assert!(first.is_finite() && first.abs() > 0.0);
        let voice = engine.synth_voice_pool.lane(0).unwrap();
        assert!((voice.phase1 - 440.0 / 44_100.0).abs() < 1e-7);
        assert!((voice.phase2 - 440.0 * ratio.value() / 44_100.0).abs() < 1e-7);
    }

    let mut fm = FmConfig {
        index: 100,
        ..FmConfig::default()
    };
    fm.index_env.attack_ms = 100.0;
    fm.amp_env.attack_ms = 0.0;
    fm.amp_env.decay_ms = 0.0;
    fm.filter_env.attack_ms = 0.0;
    let mut engine = fm_engine(fm, "direct", None);
    engine.note_on(0, 69, 127, 10_000);
    for _ in 0..128 {
        engine.next_sample();
    }
    let voice = engine.synth_voice_pool.lane(0).unwrap();
    assert!(voice.index_env.level < 0.04);
    assert!((voice.amp_env.level - 0.7).abs() < 1e-5);
    assert!(voice.filt_env.level > 0.0);
    assert_eq!(
        engine.set_fm_param_typed(0, FmParamId::Index, 0.0),
        ScalarMutation::Changed
    );
    assert_eq!(
        engine.set_synth_param_typed(0, SynthParamId::AmpGainPct, 0.0),
        ScalarMutation::Rejected
    );
}

#[test]
fn fm_filter_and_routes_use_common_voice_and_bus_paths() {
    let mut low = FmConfig {
        index: 0,
        ..FmConfig::default()
    };
    low.amp_env.attack_ms = 0.0;
    low.filter.cutoff_hz = 300.0;
    low.filter.kind = FilterType::Lowpass;
    let mut high = low;
    high.filter.kind = FilterType::Highpass;
    let mut low_engine = fm_engine(low, "direct", None);
    let mut high_engine = fm_engine(high, "direct", None);
    for engine in [&mut low_engine, &mut high_engine] {
        engine.note_on(0, 81, 127, 1000);
    }
    let low_energy: f32 = (0..512).map(|_| low_engine.next_sample().abs()).sum();
    let high_energy: f32 = (0..512).map(|_| high_engine.next_sample().abs()).sum();
    assert!(
        low_energy < high_energy * 0.5,
        "{low_energy} vs {high_energy}"
    );

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
    let mut direct = fm_engine(low, "direct", Some(bus.clone()));
    let mut routed = fm_engine(low, "fx_bus_1", Some(bus));
    direct.note_on(0, 81, 127, 1000);
    routed.note_on(0, 81, 127, 1000);
    let direct_first = direct.next_sample();
    let routed_first = routed.next_sample();
    assert!(direct_first.abs() > 0.0);
    assert!(routed_first.abs() < direct_first.abs() * 0.1);
    assert!((0..256).any(|_| routed.next_sample().abs() > 1e-5));
}

#[test]
fn fm_index_is_finite_bounded_and_high_notes_limit_modulation() {
    let mut fm = FmConfig {
        index: 100,
        ratio: FmRatio::Eight,
        ..FmConfig::default()
    };
    let mut engine = fm_engine(fm, "direct", None);
    engine.note_on(0, 127, 127, 10_000);
    let voice = engine.synth_voice_pool.lane(0).unwrap();
    assert_eq!(voice.osc2_inc, 0.5);
    assert_eq!(voice.fm_index_limit, 0.0);
    assert_eq!(
        engine.set_fm_param_typed(0, FmParamId::Index, f32::NAN),
        ScalarMutation::Rejected
    );
    assert_eq!(
        engine.set_fm_param_typed(0, FmParamId::Index, 400.0),
        ScalarMutation::Unchanged
    );
    assert!((0..8192).all(|_| engine.next_sample().is_finite()));
    for kind in [
        FilterType::Lowpass,
        FilterType::Highpass,
        FilterType::Bandpass,
        FilterType::Notch,
    ] {
        fm.filter.kind = kind;
        fm.filter.cutoff_hz = 20.0;
        fm.filter.resonance = 255.0;
        fm.filter.env_amount_pct = 100.0;
        let mut engine = fm_engine(fm, "direct", None);
        engine.note_on(0, 0, 127, 10_000);
        for _ in 0..8192 {
            let sample = engine.next_sample();
            assert!(sample.is_finite() && sample.abs() < 8.0);
        }
    }
}

#[test]
fn fm_uses_synth_admission_and_retires_voices_on_type_switch() {
    let fm = FmConfig::default();
    let mut engine = fm_engine(fm, "direct", None);
    engine.set_voice_stealing_mode(VoiceStealingMode::Fixed16);
    for note in 60..(60 + MAX_SYNTH_VOICES_PER_SLOT as u8 + 1) {
        engine.note_on(0, note, 127, 10_000);
    }
    assert_eq!(
        engine.profile_snapshot().active_synth_voices,
        MAX_SYNTH_VOICES_PER_SLOT
    );
    assert!(engine.profile_snapshot().cumulative_voice_steals >= 1);
    engine.note_off(0, 68);
    assert!(engine.synth_voice_pool.lane(0).is_some());
    let mut synth = fm_slot(fm, "direct");
    synth.kind = "synth".into();
    engine.set_instrument_slot(0, synth);
    assert_eq!(engine.profile_snapshot().active_synth_voices, 0);
    engine.note_on(0, 69, 127, 1000);
    assert!(!engine.synth_voice_pool.lane(0).unwrap().fm);
    engine.set_instrument_slot(0, fm_slot(fm, "direct"));
    assert_eq!(engine.profile_snapshot().active_synth_voices, 0);
}

#[test]
fn fm_index_release_finishes_before_its_independent_amp_release() {
    let mut fm = FmConfig::default();
    fm.index_env.decay_ms = 0.0;
    fm.index_env.sustain_pct = 100.0;
    fm.amp_env.attack_ms = 0.0;
    fm.amp_env.decay_ms = 0.0;
    let mut engine = fm_engine(fm, "direct", None);
    engine.note_on(0, 69, 127, 2_000);
    for _ in 0..128 {
        engine.next_sample();
    }
    engine.note_off(0, 69);
    for _ in 0..(44_100 * 150 / 1000) {
        engine.next_sample();
    }
    let voice = engine.synth_voice_pool.lane(0).unwrap();
    assert!(voice.active);
    assert!(voice.index_env.is_off());
    assert!(voice.amp_env.level > 0.1);
    assert!(!voice.filt_env.is_off());
}

#[test]
fn fm_static_filter_is_prepared_once_and_scalar_edits_do_not_allocate() {
    let mut engine = fm_engine(FmConfig::default(), "direct", None);
    engine.note_on(0, 69, 100, 5_000);
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut output = Vec::with_capacity(256);
    engine.render_interleaved_block(128, &mut left, &mut right, &mut output);
    reset_prepare_count_for_test();
    let ((), allocations, _) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            for index in 0..128 {
                assert_eq!(
                    engine.set_fm_param_typed(0, FmParamId::Index, index as f32),
                    if index > 100 {
                        ScalarMutation::Unchanged
                    } else {
                        ScalarMutation::Changed
                    }
                );
            }
            engine.render_interleaved_block(128, &mut left, &mut right, &mut output);
        });
    assert_eq!(allocations, 0);
    assert_eq!(prepare_count_for_test(), 1);
}

#[test]
fn fm_block_and_inline_worker_match_scalar_without_callback_allocation() {
    let mut fm = FmConfig::default();
    fm.filter.env_amount_pct = 40.0;
    fm.filter.key_tracking_pct = 100.0;
    fm.index_env.attack_ms = 3.0;
    let mut block = fm_engine(fm, "direct", None);
    let mut scalar = fm_engine(fm, "direct", None);
    let mut worker = fm_engine(fm, "direct", None);
    for engine in [&mut block, &mut scalar, &mut worker] {
        engine.note_on(0, 72, 112, 250);
    }
    let mut left = Vec::with_capacity(2048);
    let mut right = Vec::with_capacity(2048);
    let mut out = Vec::with_capacity(4096);
    let (lifecycle, mut runtime) = SourceWorkerLifecycle::start_prewarmed(&mut worker).unwrap();
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    for (frames, index) in [(32, 25.0), (128, 50.0), (256, 100.0)] {
        for engine in [&mut block, &mut scalar] {
            assert_eq!(
                engine.set_fm_param_typed(0, FmParamId::Index, index),
                ScalarMutation::Changed
            );
        }
        assert_eq!(
            runtime.with_controls_ready(&mut worker, |engine| {
                engine.set_fm_param_typed(0, FmParamId::Index, index)
            }),
            Some(ScalarMutation::Changed)
        );
        block.render_interleaved_block(frames, &mut left, &mut right, &mut out);
        let reference: Vec<_> = (0..frames)
            .flat_map(|_| {
                let (l, r) = scalar.next_stereo_sample();
                [l, r]
            })
            .collect();
        for (actual, expected) in out.iter().zip(&reference) {
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
        worker.render_interleaved_block_with_source_runtime(
            &mut runtime,
            frames,
            &mut left,
            &mut right,
            &mut out,
        );
        assert_eq!(
            runtime.health_snapshot().status,
            SourceWorkerHealth::Healthy
        );
        for (actual, expected) in out.iter().zip(&reference) {
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
    }
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);

    let mut engine = fm_engine(fm, "direct", None);
    engine.note_on(0, 72, 112, 1_000);
    engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
    let ((), allocations, _) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
        });
    assert_eq!(allocations, 0);
}
