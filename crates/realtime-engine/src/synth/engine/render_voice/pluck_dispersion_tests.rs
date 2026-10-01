use super::super::midi_note_to_hz;
use super::super::*;
use crate::synth::pluck_string::{PluckRing, PluckState, RING_LEN};
use crate::synth::{PluckConfig, PluckParamId, ScalarMutation};
use std::f64::consts::TAU;

fn configured_pluck(config: PluckConfig, sample_rate: u32) -> SynthEngine {
    super::pluck_tests::pluck_engine(config, sample_rate)
}

fn legacy_next(state: &mut PluckState, ring: &mut PluckRing) -> f32 {
    let output = if (state.emitted as usize) <= state.delay {
        state.seed ^= state.seed << 13;
        state.seed ^= state.seed >> 17;
        state.seed ^= state.seed << 5;
        let noise = (state.seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        let previous_pick = if (state.emitted as usize) >= state.pick_offset {
            ring[(state.write + RING_LEN - state.pick_offset) % RING_LEN]
        } else {
            0.0
        };
        (noise - previous_pick * 0.65) * 0.5
    } else {
        let newer = ring[(state.write + RING_LEN - state.delay) % RING_LEN];
        let older = ring[(state.write + RING_LEN - state.delay - 1) % RING_LEN];
        let delayed = newer * (1.0 - state.fraction) + older * state.fraction;
        (delayed * state.brightness + state.previous * (1.0 - state.brightness)) * state.loss
    };
    ring[state.write] = output;
    state.write = (state.write + 1) % RING_LEN;
    state.emitted = state.emitted.saturating_add(1);
    state.previous = output;
    output
}

#[test]
fn pluck_neutral_settings_match_legacy_ring_samples_and_old_json_defaults() {
    let old: PluckConfig = serde_json::from_value(serde_json::json!({
        "decayMs":1500,"brightnessPct":65,"pickPositionPct":25
    }))
    .unwrap();
    let neutral = PluckConfig {
        pick_depth_pct: 65,
        dispersion_pct: 0,
        body_amount_pct: 0,
        body_frequency_hz: 500,
        ..PluckConfig::default()
    };
    let mut actual = PluckState::note_on(440.0, 44_100, 69, 100, old.into());
    let mut expected = actual;
    let mut actual_ring = [0.0; RING_LEN];
    let mut expected_ring = [0.0; RING_LEN];
    for _ in 0..(actual.delay * 4) {
        assert_eq!(
            actual.next(&mut actual_ring).to_bits(),
            legacy_next(&mut expected, &mut expected_ring).to_bits()
        );
    }
    assert_eq!(actual_ring, expected_ring);
    assert_eq!(actual.previous.to_bits(), expected.previous.to_bits());
    assert_eq!(neutral.pick_depth_pct, 65);
}

#[test]
fn pluck_dispersion_phase_closes_with_stored_coefficients_across_notes_and_rates() {
    for sample_rate in [44_100_u32, 48_000] {
        for note in 0_u8..=127 {
            let frequency = midi_note_to_hz(note) as f64;
            for (brightness_pct, decay_ms) in [(0.0, 100.0), (65.0, 1_500.0), (100.0, 5_000.0)] {
                let settings = crate::synth::pluck_string::PluckStringSettings {
                    decay_ms,
                    brightness_pct,
                    dispersion_pct: 100.0,
                    ..PluckConfig::default().into()
                };
                let state = PluckState::note_on(frequency as f32, sample_rate, note, 120, settings);
                assert!((3..RING_LEN - 1).contains(&state.delay));
                assert!(state.dispersion_coefficient.abs() < 1.0);
                assert!((0.0..1.0).contains(&state.fraction));
                if note == 127 {
                    assert_eq!(state.dispersion_coefficient, 0.0);
                }
                let w0 = TAU * frequency / f64::from(sample_rate);
                let q = f64::from(state.loss) * (1.0 - f64::from(state.brightness));
                let psi_g = (q * w0.sin()).atan2(1.0 - q * w0.cos());
                let a = f64::from(state.dispersion_coefficient);
                let psi_a = if a == 0.0 {
                    0.0
                } else {
                    ((1.0 - a * a) * w0.sin()).atan2((1.0 + a * a) * w0.cos() + 2.0 * a)
                };
                let mu = f64::from(state.fraction);
                let interpolation_phase = (mu * w0.sin()).atan2(1.0 - mu + mu * w0.cos());
                let phase = psi_g + psi_a + state.delay as f64 * w0 + interpolation_phase;
                assert!(
                    (phase - TAU).abs() < 2.0e-5,
                    "{sample_rate} Hz note {note} brightness {brightness_pct} decay {decay_ms} delay {} frac {} a {}: {phase}",
                    state.delay,
                    state.fraction,
                    state.dispersion_coefficient
                );
            }
        }
    }
}

#[test]
fn pluck_body_is_post_ring_and_next_pluck_controls_do_not_mutate_held_state() {
    let base = PluckConfig::default();
    let body_a = PluckConfig {
        body_amount_pct: 80,
        body_frequency_hz: 300,
        ..base
    };
    let body_b = PluckConfig {
        body_amount_pct: 80,
        body_frequency_hz: 900,
        ..base
    };
    let mut dry = configured_pluck(base, 44_100);
    let mut a = configured_pluck(body_a, 44_100);
    let mut b = configured_pluck(body_b, 44_100);
    for engine in [&mut dry, &mut a, &mut b] {
        engine.note_on(0, 60, 110, 10_000);
    }
    let before = *a.synth_voice_pool.lane(0).unwrap();
    let mut body_difference = 0.0;
    for _ in 0..1024 {
        let dry_sample = dry.next_sample();
        let a_sample = a.next_sample();
        let b_sample = b.next_sample();
        assert!(dry_sample.is_finite() && a_sample.is_finite() && b_sample.is_finite());
        assert!(a_sample.abs() < 4.0 && b_sample.abs() < 4.0);
        body_difference += (a_sample - b_sample).abs();
    }
    assert!(body_difference > 0.01);
    let held_a = a.synth_voice_pool.lane(0).unwrap();
    let held_b = b.synth_voice_pool.lane(0).unwrap();
    let held_dry = dry.synth_voice_pool.lane(0).unwrap();
    assert_ne!(held_a.pluck.body, held_b.pluck.body);
    assert_eq!(
        held_a.pluck.previous.to_bits(),
        held_dry.pluck.previous.to_bits()
    );
    assert_eq!(
        held_b.pluck.previous.to_bits(),
        held_dry.pluck.previous.to_bits()
    );
    assert_eq!(held_a.pluck.write, held_dry.pluck.write);
    assert_eq!(held_a.pluck.delay, held_dry.pluck.delay);
    assert_eq!(
        held_a.pluck.fraction.to_bits(),
        held_dry.pluck.fraction.to_bits()
    );
    assert_ne!(before.pluck.body, held_a.pluck.body);

    let held_before_edit = *a.synth_voice_pool.lane(0).unwrap();
    for (id, value) in [
        (PluckParamId::PickDepthPct, 20.0),
        (PluckParamId::DispersionPct, 70.0),
        (PluckParamId::BodyAmountPct, 50.0),
        (PluckParamId::BodyFrequencyHz, 1200.0),
    ] {
        assert_eq!(
            a.set_pluck_param_typed(0, id, value),
            ScalarMutation::Changed
        );
    }
    let held_after_edit = a.synth_voice_pool.lane(0).unwrap();
    assert_eq!(held_after_edit.pluck, held_before_edit.pluck);
    assert_eq!(held_after_edit.pluck.body, held_before_edit.pluck.body);
    a.note_on(0, 67, 110, 10_000);
    let future = a.synth_voice_pool.lane(1).unwrap();
    assert_eq!(future.pluck.pick_depth, 0.2);
    assert!(future.pluck.dispersion_coefficient < 0.0);
    assert_eq!(future.pluck.body_amount, 0.5);
    assert_eq!(future.pluck.body_frequency_hz, 1200.0);

    let edited = PluckConfig {
        pick_depth_pct: 20,
        dispersion_pct: 70,
        body_amount_pct: 50,
        body_frequency_hz: 1200,
        ..base
    };
    a.set_voice_stealing_mode(VoiceStealingMode::Fixed16);
    for note in 70..(70 + MAX_SYNTH_VOICES_PER_SLOT as u8) {
        a.note_on(0, note, 110, 10_000);
    }
    a.note_on(0, 90, 110, 10_000);
    let reused = a
        .synth_voice_pool
        .slot_lanes(0)
        .unwrap()
        .iter()
        .copied()
        .find(|lane| a.synth_voice_pool.lane(*lane).unwrap().midi_note == 90)
        .unwrap();
    let mut fresh = configured_pluck(edited, 44_100);
    fresh.note_on(0, 90, 110, 10_000);
    assert_eq!(
        a.synth_voice_pool.lane(reused).unwrap().pluck,
        fresh.synth_voice_pool.lane(0).unwrap().pluck
    );
}

#[test]
fn pluck_dispersion_and_body_match_scalar_block_and_worker_rendering() {
    let config = PluckConfig {
        dispersion_pct: 80,
        body_amount_pct: 55,
        body_frequency_hz: 720,
        pick_depth_pct: 35,
        ..PluckConfig::default()
    };
    let mut scalar = configured_pluck(config, 48_000);
    let mut block = configured_pluck(config, 48_000);
    let mut worker = configured_pluck(config, 48_000);
    for engine in [&mut scalar, &mut block, &mut worker] {
        engine.note_on(0, 64, 112, 10_000);
    }
    let (lifecycle, mut runtime) = SourceWorkerLifecycle::start_prewarmed(&mut worker).unwrap();
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut output = Vec::with_capacity(256);
    block.render_interleaved_block(128, &mut left, &mut right, &mut output);
    let expected = (0..128)
        .flat_map(|_| {
            let (left, right) = scalar.next_stereo_sample();
            [left, right]
        })
        .collect::<Vec<_>>();
    for (actual, expected) in output.iter().zip(&expected) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    worker.render_interleaved_block_with_source_runtime(
        &mut runtime,
        128,
        &mut left,
        &mut right,
        &mut output,
    );
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    for (actual, expected) in output.iter().zip(&expected) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[test]
fn pluck_parameter_ids_are_closed_and_new_values_are_bounded() {
    let expected = [
        (PluckParamId::DecayMs, "pluck.decayMs"),
        (PluckParamId::BrightnessPct, "pluck.brightnessPct"),
        (PluckParamId::PickPositionPct, "pluck.pickPositionPct"),
        (PluckParamId::PickDepthPct, "pluck.pickDepthPct"),
        (PluckParamId::DispersionPct, "pluck.dispersionPct"),
        (PluckParamId::BodyAmountPct, "pluck.bodyAmountPct"),
        (PluckParamId::BodyFrequencyHz, "pluck.bodyFrequencyHz"),
        (PluckParamId::AmpGainPct, "pluck.amp.gainPct"),
        (
            PluckParamId::AmpVelocitySensitivityPct,
            "pluck.amp.velocitySensitivityPct",
        ),
        (PluckParamId::AmpEnvAttackMs, "pluck.ampEnv.attackMs"),
        (PluckParamId::AmpEnvDecayMs, "pluck.ampEnv.decayMs"),
        (PluckParamId::AmpEnvSustainPct, "pluck.ampEnv.sustainPct"),
        (PluckParamId::AmpEnvReleaseMs, "pluck.ampEnv.releaseMs"),
        (PluckParamId::FilterCutoffHz, "pluck.filter.cutoffHz"),
        (PluckParamId::FilterResonance, "pluck.filter.resonance"),
        (
            PluckParamId::FilterEnvAmountPct,
            "pluck.filter.envAmountPct",
        ),
        (
            PluckParamId::FilterKeyTrackingPct,
            "pluck.filter.keyTrackingPct",
        ),
        (PluckParamId::FilterEnvAttackMs, "pluck.filterEnv.attackMs"),
        (PluckParamId::FilterEnvDecayMs, "pluck.filterEnv.decayMs"),
        (
            PluckParamId::FilterEnvSustainPct,
            "pluck.filterEnv.sustainPct",
        ),
        (
            PluckParamId::FilterEnvReleaseMs,
            "pluck.filterEnv.releaseMs",
        ),
    ];
    assert_eq!(expected.len(), PluckParamId::ALL.len());
    for ((id, path), all) in expected.into_iter().zip(PluckParamId::ALL) {
        assert_eq!(id, all);
        assert_eq!(PluckParamId::from_path(path), Some(id));
    }
    assert!(PluckParamId::from_path("pluck.bodyQ").is_none());
    let mut engine = configured_pluck(PluckConfig::default(), 44_100);
    for id in [
        PluckParamId::PickDepthPct,
        PluckParamId::DispersionPct,
        PluckParamId::BodyAmountPct,
        PluckParamId::BodyFrequencyHz,
    ] {
        assert_eq!(
            engine.set_pluck_param_typed(0, id, f32::NAN),
            ScalarMutation::Rejected
        );
    }
    assert_eq!(
        engine.set_pluck_param_typed(0, PluckParamId::BodyFrequencyHz, 50.0),
        ScalarMutation::Changed
    );
    assert_eq!(
        engine.set_pluck_param_typed(0, PluckParamId::BodyFrequencyHz, 100.0),
        ScalarMutation::Unchanged
    );
}
