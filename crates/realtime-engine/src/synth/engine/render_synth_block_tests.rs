use super::*;
use crate::synth::{ScalarMutation, SynthParamId};

#[test]
fn inline_source_executor_does_not_allocate_at_default_quantum() {
    let mut engine = SynthEngine::new(44_100);
    engine.note_on(0, 60, 96, 1_000);
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);
    engine.render_interleaved_block(128, &mut left, &mut right, &mut out);

    let (_, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
        });
    assert_eq!(allocations, 0);
    assert_eq!(deallocations, 0);
}

#[test]
fn synth_oscillator_scalar_edits_keep_held_voice_and_change_audio() {
    for (id, value) in [
        (SynthParamId::Osc1LevelPct, 0.0),
        (SynthParamId::Osc1DetuneCents, 50.0),
        (SynthParamId::Osc1PulseWidthPct, 5.0),
        (SynthParamId::Osc2LevelPct, 0.0),
        (SynthParamId::Osc2DetuneCents, -50.0),
        (SynthParamId::Osc2PulseWidthPct, 95.0),
    ] {
        let mut cfg = default_synth_config();
        cfg.osc1.waveform = WaveformId::Pulse;
        cfg.osc2.waveform = WaveformId::Pulse;
        cfg.amp_env.attack_ms = 0.0;
        let mut changed = synth_engine(cfg);
        let mut scalar = synth_engine(cfg);
        let mut baseline = synth_engine(cfg);
        for engine in [&mut changed, &mut scalar, &mut baseline] {
            engine.note_on(0, 81, 127, 10_000);
            for _ in 0..32 {
                engine.next_sample();
            }
        }
        let before = *changed.synth_voice_pool.lane(0).unwrap();
        let other_slot = changed.instruments[1];
        let other_revision = changed.synth_render_revisions[1];
        let (mutation, allocations, _) =
            crate::synth::test_allocator::count_allocations_and_deallocations(|| {
                changed.set_synth_param_typed(0, id, value)
            });
        assert_eq!(mutation, ScalarMutation::Changed);
        assert_eq!(allocations, 0);
        assert_eq!(
            scalar.set_synth_param_typed(0, id, value),
            ScalarMutation::Changed
        );
        let held = changed.synth_voice_pool.lane(0).unwrap();
        assert_eq!(held.canonical_lane, before.canonical_lane);
        assert_eq!(held.started_sample, before.started_sample);
        assert_eq!(
            (held.phase1.to_bits(), held.phase2.to_bits()),
            (before.phase1.to_bits(), before.phase2.to_bits())
        );
        assert_eq!(held.amp_env.stage, before.amp_env.stage);
        assert_eq!(
            changed.instruments[1].osc1.level_pct,
            other_slot.osc1.level_pct
        );
        assert_eq!(changed.synth_render_revisions[1], other_revision);

        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut rendered = Vec::new();
        changed.render_interleaved_block(256, &mut left, &mut right, &mut rendered);
        let expected: Vec<_> = (0..256)
            .flat_map(|_| {
                let (l, r) = scalar.next_stereo_sample();
                [l, r]
            })
            .collect();
        assert_eq!(rendered.len(), expected.len());
        for (actual, expected) in rendered.iter().zip(&expected) {
            assert_eq!(actual.to_bits(), expected.to_bits(), "{id:?}");
        }
        let untouched: Vec<_> = (0..256).map(|_| baseline.next_sample()).collect();
        let changed_energy = rendered
            .chunks_exact(2)
            .zip(untouched)
            .map(|(stereo, old)| ((stereo[0] + stereo[1]) * 0.5 - old).abs())
            .sum::<f32>();
        assert!(changed_energy > 0.01, "{id:?}: {changed_energy}");
        let after = changed.synth_voice_pool.lane(0).unwrap();
        assert!(after.active && after.phase1 != before.phase1);
    }
}

#[test]
fn synth_block_matches_scalar_for_waveforms_velocities_and_quanta() {
    for waveform in [
        WaveformId::Sine,
        WaveformId::Triangle,
        WaveformId::Saw,
        WaveformId::Square,
        WaveformId::Pulse,
    ] {
        for velocity in [0, 1, 64, 127] {
            for frames in [32, 64, 128, 256] {
                let mut cfg = default_synth_config();
                cfg.osc1.waveform = waveform;
                cfg.osc2.waveform = waveform;
                cfg.osc1.detune_cents = 11.0;
                cfg.osc2.detune_cents = -17.0;
                cfg.amp.velocity_sensitivity_pct = 63.0;
                cfg.amp.gain_pct = 71.0;
                let mut block = synth_engine(cfg);
                let mut scalar = synth_engine(cfg);
                block.note_on(0, 60, velocity, 10_000);
                scalar.note_on(0, 60, velocity, 10_000);
                render_pair(&mut block, &mut scalar, frames);
            }
        }
    }
}

#[test]
fn synth_block_matches_scalar_for_static_cutoff_paths() {
    for (cutoff_cc, resonance_cc) in [(None, None), (Some(83), Some(101))] {
        let mut cfg = default_synth_config();
        cfg.filter.env_amount_pct = if cutoff_cc.is_some() { 47.0 } else { 0.0 };
        cfg.filter.kind = FilterType::Notch;
        cfg.filter.resonance = 73.0;
        let mut block = synth_engine(cfg);
        let mut scalar = synth_engine(cfg);
        block.note_on(0, 48, 96, 10_000);
        scalar.note_on(0, 48, 96, 10_000);
        if let Some(value) = cutoff_cc {
            block.cc(0, 74, value);
            scalar.cc(0, 74, value);
        }
        if let Some(value) = resonance_cc {
            block.cc(0, 71, value);
            scalar.cc(0, 71, value);
        }
        for frames in [32, 64, 128, 256] {
            render_pair(&mut block, &mut scalar, frames);
        }
    }
}

#[test]
fn synth_block_matches_scalar_for_dynamic_filter_envelopes() {
    for (env_amount, resonance_cc) in [(42.0, None), (-42.0, Some(47))] {
        let mut cfg = default_synth_config();
        cfg.filter.env_amount_pct = env_amount;
        cfg.filter.cutoff_hz = 1_700.0;
        cfg.filter.resonance = 68.0;
        cfg.filter.kind = FilterType::Highpass;
        let mut block = synth_engine(cfg);
        let mut scalar = synth_engine(cfg);
        block.note_on(0, 67, 83, 10_000);
        scalar.note_on(0, 67, 83, 10_000);
        if let Some(value) = resonance_cc {
            block.cc(0, 71, value);
            scalar.cc(0, 71, value);
        }
        for frames in [32, 64, 128, 256] {
            render_pair(&mut block, &mut scalar, frames);
        }
    }
}

#[test]
fn synth_block_matches_scalar_across_config_and_mod_changes_and_release() {
    let mut cfg = default_synth_config();
    cfg.filter.env_amount_pct = 0.0;
    cfg.amp_env.attack_ms = 1.0;
    cfg.amp_env.release_ms = 3.0;
    cfg.filter_env.attack_ms = 1.0;
    cfg.filter_env.release_ms = 3.0;
    let mut block = synth_engine(cfg);
    let mut scalar = synth_engine(cfg);
    block.note_on(0, 60, 97, 10_000);
    scalar.note_on(0, 60, 97, 10_000);
    render_pair(&mut block, &mut scalar, 32);

    for engine in [&mut block, &mut scalar] {
        engine.set_synth_param(0, "synth.filter.cutoffHz", 2_300.0);
        engine.set_synth_param(0, "synth.filter.resonance", 61.0);
        engine.set_synth_param(0, "synth.filter.envAmountPct", -35.0);
        engine.cc(0, 71, 83);
    }
    render_pair(&mut block, &mut scalar, 64);

    block.note_off(0, 60);
    scalar.note_off(0, 60);
    render_pair(&mut block, &mut scalar, 128);
    render_pair(&mut block, &mut scalar, 256);
}

#[test]
fn synth_block_static_filter_prepares_once_per_active_voice() {
    for cutoff_cc in [None, Some(83)] {
        let mut cfg = default_synth_config();
        cfg.filter.env_amount_pct = if cutoff_cc.is_some() { 42.0 } else { 0.0 };
        let mut engine = synth_engine(cfg);
        engine.note_on(0, 60, 96, 10_000);
        if let Some(value) = cutoff_cc {
            engine.cc(0, 74, value);
        }
        reset_prepare_count_for_test();
        render_engine(&mut engine, 256);
        assert_eq!(prepare_count_for_test(), 1);
    }
}

#[test]
fn synth_block_dynamic_filter_prepares_each_active_frame() {
    let mut cfg = default_synth_config();
    cfg.filter.env_amount_pct = 42.0;
    let mut engine = synth_engine(cfg);
    engine.note_on(0, 60, 96, 10_000);
    reset_prepare_count_for_test();
    render_engine(&mut engine, 256);
    assert_eq!(prepare_count_for_test(), 256);
}

fn synth_engine(cfg: SynthConfig) -> SynthEngine {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "synth".to_string(),
            synth: cfg,
            mixer: None,
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine
}

fn render_pair(block: &mut SynthEngine, scalar: &mut SynthEngine, frames: usize) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut out = Vec::new();
    block.render_interleaved_block(frames, &mut left, &mut right, &mut out);
    let mut expected = Vec::with_capacity(frames * 2);
    for _ in 0..frames {
        let (left, right) = scalar.next_stereo_sample();
        expected.push(left);
        expected.push(right);
    }
    assert_eq!(out.len(), expected.len());
    for (index, (actual, expected)) in out.iter().zip(expected).enumerate() {
        assert_eq!(actual.to_bits(), expected.to_bits(), "sample {index}");
    }
    assert_voice_state_matches(block, scalar);
}

fn render_engine(engine: &mut SynthEngine, frames: usize) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut out = Vec::new();
    engine.render_interleaved_block(frames, &mut left, &mut right, &mut out);
}

fn assert_voice_state_matches(actual: &SynthEngine, expected: &SynthEngine) {
    assert_eq!(actual.sample_clock, expected.sample_clock);
    assert_eq!(
        actual.synth_render_revisions,
        expected.synth_render_revisions
    );
    let actual = actual.synth_voice_pool.lane(0).expect("synth lane");
    let expected = expected.synth_voice_pool.lane(0).expect("synth lane");
    assert_eq!(actual.active, expected.active);
    assert_eq!(actual.instrument_slot, expected.instrument_slot);
    assert_eq!(actual.midi_note, expected.midi_note);
    assert_eq!(actual.velocity, expected.velocity);
    assert_eq!(
        actual.velocity_norm.to_bits(),
        expected.velocity_norm.to_bits()
    );
    assert_eq!(actual.note_off_sample, expected.note_off_sample);
    assert_eq!(actual.started_sample, expected.started_sample);
    assert_eq!(actual.freq_hz.to_bits(), expected.freq_hz.to_bits());
    assert_eq!(actual.osc1_inc.to_bits(), expected.osc1_inc.to_bits());
    assert_eq!(actual.osc2_inc.to_bits(), expected.osc2_inc.to_bits());
    assert_eq!(actual.render_revision, expected.render_revision);
    assert_eq!(actual.phase1.to_bits(), expected.phase1.to_bits());
    assert_eq!(actual.phase2.to_bits(), expected.phase2.to_bits());
    assert_env_state_matches(actual.amp_env, expected.amp_env);
    assert_env_state_matches(actual.filt_env, expected.filt_env);
    assert_eq!(actual.filt, expected.filt);
}

fn assert_env_state_matches(actual: EnvState, expected: EnvState) {
    assert_eq!(actual.stage, expected.stage);
    assert_eq!(actual.level.to_bits(), expected.level.to_bits());
    assert_eq!(actual.stage_pos, expected.stage_pos);
    assert_eq!(actual.stage_len, expected.stage_len);
    assert_eq!(actual.sustain.to_bits(), expected.sustain.to_bits());
    assert_eq!(
        actual.release_start.to_bits(),
        expected.release_start.to_bits()
    );
}
