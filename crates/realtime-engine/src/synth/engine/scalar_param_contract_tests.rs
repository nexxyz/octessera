use super::super::scalar_param::{SampleBankParamId, ScalarMutation, SynthParamId};

#[test]
fn synth_scalar_ids_match_exact_dotted_paths() {
    let expected = [
        (SynthParamId::AmpGainPct, "synth.amp.gainPct"),
        (
            SynthParamId::AmpVelocitySensitivityPct,
            "synth.amp.velocitySensitivityPct",
        ),
        (SynthParamId::AmpEnvAttackMs, "synth.ampEnv.attackMs"),
        (SynthParamId::AmpEnvDecayMs, "synth.ampEnv.decayMs"),
        (SynthParamId::AmpEnvSustainPct, "synth.ampEnv.sustainPct"),
        (SynthParamId::AmpEnvReleaseMs, "synth.ampEnv.releaseMs"),
        (SynthParamId::FilterCutoffHz, "synth.filter.cutoffHz"),
        (SynthParamId::FilterResonance, "synth.filter.resonance"),
        (
            SynthParamId::FilterEnvAmountPct,
            "synth.filter.envAmountPct",
        ),
        (
            SynthParamId::FilterKeyTrackingPct,
            "synth.filter.keyTrackingPct",
        ),
        (
            SynthParamId::FilterEnvAttackMs,
            "synth.filterEnv.attackMs",
        ),
        (
            SynthParamId::FilterEnvDecayMs,
            "synth.filterEnv.decayMs",
        ),
        (
            SynthParamId::FilterEnvSustainPct,
            "synth.filterEnv.sustainPct",
        ),
        (
            SynthParamId::FilterEnvReleaseMs,
            "synth.filterEnv.releaseMs",
        ),
    ];
    assert_eq!(expected.len(), SynthParamId::ALL.len());
    for ((id, path), expected_id) in expected.into_iter().zip(SynthParamId::ALL) {
        assert_eq!(id, expected_id);
        assert_eq!(serde_json::to_string(&id).unwrap(), format!("\"{path}\""));
        assert_eq!(SynthParamId::from_path(path), Some(id));
    }
}

#[test]
fn sample_scalar_ids_match_exact_dotted_paths() {
    let expected = [
        (SampleBankParamId::TuneSemis, "sample.tuneSemis"),
        (SampleBankParamId::AmpGainPct, "sample.amp.gainPct"),
        (
            SampleBankParamId::AmpVelocitySensitivityPct,
            "sample.amp.velocitySensitivityPct",
        ),
        (SampleBankParamId::FilterCutoffHz, "sample.filter.cutoffHz"),
        (SampleBankParamId::FilterResonance, "sample.filter.resonance"),
    ];
    assert_eq!(expected.len(), SampleBankParamId::ALL.len());
    for ((id, path), expected_id) in expected.into_iter().zip(SampleBankParamId::ALL) {
        assert_eq!(id, expected_id);
        assert_eq!(serde_json::to_string(&id).unwrap(), format!("\"{path}\""));
        assert_eq!(SampleBankParamId::from_path(path), Some(id));
    }
}

#[test]
fn typed_scalar_updates_preserve_all_existing_clamps() {
    let mut engine = SynthEngine::new(48_000);
    let synth_updates = [
        (SynthParamId::AmpGainPct, 120.0),
        (SynthParamId::AmpVelocitySensitivityPct, -1.0),
        (SynthParamId::AmpEnvAttackMs, -1.0),
        (SynthParamId::AmpEnvDecayMs, 6_000.0),
        (SynthParamId::AmpEnvSustainPct, 120.0),
        (SynthParamId::AmpEnvReleaseMs, 20_000.0),
        (SynthParamId::FilterCutoffHz, 0.0),
        (SynthParamId::FilterResonance, 300.0),
        (SynthParamId::FilterEnvAmountPct, -200.0),
        (SynthParamId::FilterKeyTrackingPct, 120.0),
        (SynthParamId::FilterEnvAttackMs, -1.0),
        (SynthParamId::FilterEnvDecayMs, 6_000.0),
        (SynthParamId::FilterEnvSustainPct, 120.0),
        (SynthParamId::FilterEnvReleaseMs, 20_000.0),
    ];
    for (id, value) in synth_updates {
        assert_eq!(
            engine.set_synth_param_typed(0, id, value),
            ScalarMutation::Changed
        );
    }
    assert_eq!(
        synth_scalar_values(&engine, 0),
        [
            100.0, 0.0, 0.0, 5_000.0, 100.0, 10_000.0, 20.0, 255.0, -100.0, 100.0,
            0.0, 5_000.0, 100.0, 10_000.0,
        ]
    );

    let sample_updates = [
        (SampleBankParamId::TuneSemis, -100.0),
        (SampleBankParamId::AmpGainPct, -1.0),
        (SampleBankParamId::AmpVelocitySensitivityPct, -1.0),
        (SampleBankParamId::FilterCutoffHz, 0.0),
        (SampleBankParamId::FilterResonance, 300.0),
    ];
    for (id, value) in sample_updates {
        assert_eq!(
            engine.set_sample_bank_param_typed(0, id, value),
            ScalarMutation::Changed
        );
    }
    assert_eq!(
        sample_scalar_values(&engine, 0),
        [-24.0, 0.0, 0.0, 20.0, 255.0]
    );
}

#[test]
fn typed_scalar_nonfinite_values_reject_without_mutation_or_revision_churn() {
    let mut engine = SynthEngine::new(48_000);
    let before_values = synth_scalar_values(&engine, 0);
    let before_revision = engine.synth_render_revisions[0];
    for id in SynthParamId::ALL {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                engine.set_synth_param_typed(0, id, value),
                ScalarMutation::Rejected
            );
            assert_f32_arrays_bitwise_equal(before_values, synth_scalar_values(&engine, 0));
            assert_eq!(engine.synth_render_revisions[0], before_revision);
        }
    }

    let before_sample_values = sample_scalar_values(&engine, 0);
    for id in SampleBankParamId::ALL {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                engine.set_sample_bank_param_typed(0, id, value),
                ScalarMutation::Rejected
            );
            assert_f32_arrays_bitwise_equal(
                before_sample_values,
                sample_scalar_values(&engine, 0),
            );
        }
    }
}

#[test]
fn legacy_scalar_wrappers_parse_only_canonical_paths() {
    let mut engine = SynthEngine::new(48_000);
    assert_eq!(
        engine.set_synth_param(0, "synth.filter.cutoffHz", 320.0),
        ScalarMutation::Changed
    );
    assert_eq!(
        engine.set_sample_bank_param(0, "sample.filter.cutoffHz", 1_234.0),
        ScalarMutation::Changed
    );
    assert_eq!(
        engine.set_synth_param(0, "synth.filter.unknown", 1.0),
        ScalarMutation::Rejected
    );
    assert_eq!(
        engine.set_sample_bank_param(0, "sample.unknown", 1.0),
        ScalarMutation::Rejected
    );
}

fn synth_scalar_values(engine: &SynthEngine, slot: usize) -> [f32; 14] {
    let synth = engine.instruments[slot];
    [
        synth.amp.gain_pct,
        synth.amp.velocity_sensitivity_pct,
        synth.amp_env.attack_ms,
        synth.amp_env.decay_ms,
        synth.amp_env.sustain_pct,
        synth.amp_env.release_ms,
        synth.filter.cutoff_hz,
        synth.filter.resonance,
        synth.filter.env_amount_pct,
        synth.filter.key_tracking_pct,
        synth.filter_env.attack_ms,
        synth.filter_env.decay_ms,
        synth.filter_env.sustain_pct,
        synth.filter_env.release_ms,
    ]
}

fn sample_scalar_values(engine: &SynthEngine, slot: usize) -> [f32; 5] {
    let bank = &engine.sample_banks[slot];
    [
        bank.tune_semis,
        bank.gain_pct,
        bank.velocity_sensitivity_pct,
        bank.filter_cutoff_hz,
        bank.filter_resonance,
    ]
}

fn assert_f32_arrays_bitwise_equal<const N: usize>(expected: [f32; N], actual: [f32; N]) {
    for (expected, actual) in expected.into_iter().zip(actual) {
        assert_eq!(expected.to_bits(), actual.to_bits());
    }
}
