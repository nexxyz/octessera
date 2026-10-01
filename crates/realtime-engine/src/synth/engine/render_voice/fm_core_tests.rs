use super::super::*;
use super::source_sample;
use crate::synth::{FmParamId, ScalarMutation};

fn fm_engine(fm: FmConfig) -> SynthEngine {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "fm".into(),
            synth: default_synth_config(),
            fm: Some(fm),
            pluck: None,
            drum: None,
            mixer: None,
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine
}

#[test]
fn fm_new_scalar_ids_are_exact_and_fine_ratio_edits_are_absolute() {
    let expected = [
        (FmParamId::RatioFineCents, "fm.ratioFineCents"),
        (FmParamId::Index, "fm.index"),
        (FmParamId::VelocityToIndexPct, "fm.velocityToIndexPct"),
        (FmParamId::ModShapePct, "fm.modShapePct"),
        (FmParamId::ModMixPct, "fm.modMixPct"),
        (FmParamId::IndexEnvAttackMs, "fm.indexEnv.attackMs"),
        (FmParamId::IndexEnvDecayMs, "fm.indexEnv.decayMs"),
        (FmParamId::IndexEnvSustainPct, "fm.indexEnv.sustainPct"),
        (FmParamId::IndexEnvReleaseMs, "fm.indexEnv.releaseMs"),
        (FmParamId::AmpGainPct, "fm.amp.gainPct"),
        (
            FmParamId::AmpVelocitySensitivityPct,
            "fm.amp.velocitySensitivityPct",
        ),
        (FmParamId::AmpEnvAttackMs, "fm.ampEnv.attackMs"),
        (FmParamId::AmpEnvDecayMs, "fm.ampEnv.decayMs"),
        (FmParamId::AmpEnvSustainPct, "fm.ampEnv.sustainPct"),
        (FmParamId::AmpEnvReleaseMs, "fm.ampEnv.releaseMs"),
        (FmParamId::FilterCutoffHz, "fm.filter.cutoffHz"),
        (FmParamId::FilterResonance, "fm.filter.resonance"),
        (FmParamId::FilterEnvAmountPct, "fm.filter.envAmountPct"),
        (FmParamId::FilterKeyTrackingPct, "fm.filter.keyTrackingPct"),
        (FmParamId::FilterEnvAttackMs, "fm.filterEnv.attackMs"),
        (FmParamId::FilterEnvDecayMs, "fm.filterEnv.decayMs"),
        (FmParamId::FilterEnvSustainPct, "fm.filterEnv.sustainPct"),
        (FmParamId::FilterEnvReleaseMs, "fm.filterEnv.releaseMs"),
    ];
    assert_eq!(expected.len(), FmParamId::ALL.len());
    for ((id, path), all) in expected.into_iter().zip(FmParamId::ALL) {
        assert_eq!(id, all);
        assert_eq!(FmParamId::from_path(path), Some(id));
    }
    assert!(FmParamId::from_path("fm.ratio").is_none());

    let mut fm = FmConfig {
        ratio: FmRatio::Three,
        ..FmConfig::default()
    };
    let mut live = fm_engine(fm);
    live.note_on(0, 69, 127, 10_000);
    let original_phase = live.synth_voice_pool.lane(0).unwrap().phase1;
    for cents in [100.0, -100.0, 35.0, 0.0] {
        assert_eq!(
            live.set_fm_param_typed(0, FmParamId::RatioFineCents, cents),
            ScalarMutation::Changed
        );
        let config = live.synth_render_configs[0];
        let revision = live.synth_render_revisions[0];
        let voice = live.synth_voice_pool.lane_mut(0).unwrap();
        refresh_synth_voice_render_cache(voice, &config, 44_100, revision);
        let expected_ratio = if cents == 0.0 {
            3.0
        } else {
            3.0 * 2.0_f32.powf(cents / 1200.0)
        };
        assert!((voice.osc2_inc - 440.0 * expected_ratio / 44_100.0).abs() < 1e-7);
        assert_eq!(voice.phase1, original_phase);
    }
    fm.ratio_fine_cents = 0;
    let mut fresh = fm_engine(fm);
    fresh.note_on(0, 69, 127, 10_000);
    assert_eq!(
        live.synth_voice_pool.lane(0).unwrap().osc2_inc.to_bits(),
        fresh.synth_voice_pool.lane(0).unwrap().osc2_inc.to_bits()
    );
}

#[test]
fn fm_neutral_source_is_sample_identical_to_legacy_equation() {
    let mut engine = fm_engine(FmConfig {
        index: 67,
        ..FmConfig::default()
    });
    engine.note_on(0, 71, 111, 10_000);
    let config = engine.synth_render_configs[0];
    let initial_voice = *engine.synth_voice_pool.lane(0).unwrap();
    let mut expected_voice = initial_voice;
    let mut actual_voice = initial_voice;
    let mut ring = [0.0; crate::synth::pluck_string::RING_LEN];
    let mut phase_wraps = 0;
    let mut saw_env_stage_change = false;
    for _ in 0..12_000 {
        let previous_phase1 = expected_voice.phase1;
        let previous_phase2 = expected_voice.phase2;
        let previous_stage = expected_voice.index_env.stage;
        expected_voice.phase1 = (expected_voice.phase1 + expected_voice.osc1_inc).fract();
        expected_voice.phase2 = (expected_voice.phase2 + expected_voice.osc2_inc).fract();
        phase_wraps += usize::from(expected_voice.phase1 < previous_phase1);
        phase_wraps += usize::from(expected_voice.phase2 < previous_phase2);
        let expected = (std::f32::consts::TAU * expected_voice.phase1
            + (67.0_f32 * 0.04).min(expected_voice.fm_index_limit)
                * expected_voice.index_env.next()
                * (std::f32::consts::TAU * expected_voice.phase2).sin())
        .sin();
        let actual = source_sample(&config, &mut actual_voice, &mut ring);
        assert_eq!(actual.to_bits(), expected.to_bits());
        assert_eq!(
            actual_voice.phase1.to_bits(),
            expected_voice.phase1.to_bits()
        );
        assert_eq!(
            actual_voice.phase2.to_bits(),
            expected_voice.phase2.to_bits()
        );
        assert_eq!(
            actual_voice.index_env.stage_pos,
            expected_voice.index_env.stage_pos
        );
        assert_eq!(
            actual_voice.index_env.level.to_bits(),
            expected_voice.index_env.level.to_bits()
        );
        saw_env_stage_change |= expected_voice.index_env.stage != previous_stage;
    }
    assert!(phase_wraps > 0);
    assert!(saw_env_stage_change);
    assert_eq!(expected_voice.index_env.stage, EnvStage::Sustain);
    assert!(expected_voice.index_env.stage_pos > 0);
}

#[test]
fn fm_scalar_controls_are_absolute_and_revision_refresh_keeps_held_phase() {
    let mut engine = fm_engine(FmConfig::default());
    engine.note_on(0, 75, 96, 10_000);
    for _ in 0..17 {
        engine.next_sample();
    }
    let before = *engine.synth_voice_pool.lane(0).unwrap();
    for (id, value) in [
        (FmParamId::VelocityToIndexPct, 100.0),
        (FmParamId::ModShapePct, 75.0),
        (FmParamId::ModMixPct, 60.0),
    ] {
        assert_eq!(
            engine.set_fm_param_typed(0, id, value),
            ScalarMutation::Changed
        );
        let config = engine.synth_render_configs[0];
        let revision = engine.synth_render_revisions[0];
        let voice = engine.synth_voice_pool.lane_mut(0).unwrap();
        refresh_synth_voice_render_cache(voice, &config, 44_100, revision);
        assert_eq!(voice.phase1.to_bits(), before.phase1.to_bits());
        assert_eq!(voice.phase2.to_bits(), before.phase2.to_bits());
        assert_eq!(voice.index_env.stage_pos, before.index_env.stage_pos);
    }
    assert_eq!(
        engine.set_fm_param_typed(0, FmParamId::ModShapePct, 75.0),
        ScalarMutation::Unchanged
    );
    assert_eq!(
        engine.set_fm_param_typed(0, FmParamId::ModMixPct, 101.0),
        ScalarMutation::Changed
    );
    assert_eq!(
        engine.set_fm_param_typed(0, FmParamId::ModMixPct, f32::INFINITY),
        ScalarMutation::Rejected
    );
}
