use super::super::*;
use crate::synth::{FmParamId, ScalarMutation, SynthParamId};

const SAMPLE_RATE: u32 = 48_000;
/// 10 ms at 48 kHz.
const GLIDE_FRAMES: usize = 480;

fn engine_with(kind: &str) -> SynthEngine {
    let mut synth = default_synth_config();
    synth.filter.cutoff_hz = 1_000.0;
    synth.osc1.level_pct = 80.0;
    engine_from(kind, synth)
}

fn engine_from(kind: &str, synth: SynthConfig) -> SynthEngine {
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: kind.into(),
            synth,
            mixer: None,
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine.note_on(0, 60, 100, 10_000);
    engine
}

fn render(engine: &mut SynthEngine, frames: usize) {
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    engine.render_interleaved_block(frames, &mut left, &mut right, &mut out);
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() <= expected.abs() * 1.0e-4 + 1.0e-4
}

#[test]
fn live_cutoff_glides_evenly_in_pitch_over_ten_milliseconds() {
    let mut engine = engine_with("synth");
    assert_eq!(
        engine.set_synth_param_typed(0, SynthParamId::FilterCutoffHz, 4_000.0),
        ScalarMutation::Changed
    );
    assert_eq!(engine.instruments[0].filter.cutoff_hz, 1_000.0);

    render(&mut engine, GLIDE_FRAMES / 4);
    let quarter = engine.instruments[0].filter.cutoff_hz;
    assert!(close(quarter, 1_000.0 * 2.0_f32.sqrt()), "{quarter}");
    render(&mut engine, GLIDE_FRAMES / 2);
    let three_quarters = engine.instruments[0].filter.cutoff_hz;
    assert!(
        close(three_quarters, 2_000.0 * 2.0_f32.sqrt()),
        "{three_quarters}"
    );
    render(&mut engine, GLIDE_FRAMES / 4);
    assert_eq!(engine.instruments[0].filter.cutoff_hz, 4_000.0);

    let revision = engine.synth_render_revisions[0];
    render(&mut engine, 128);
    assert_eq!(engine.synth_render_revisions[0], revision);
    assert!(!engine.instrument_param_glides.active);
}

#[test]
fn live_level_glides_linearly_and_a_retarget_starts_from_the_running_value() {
    let mut engine = engine_with("synth");
    engine.set_synth_param_typed(0, SynthParamId::Osc1LevelPct, 20.0);
    render(&mut engine, GLIDE_FRAMES / 2);
    assert!(close(engine.instruments[0].osc1.level_pct, 50.0));

    assert_eq!(
        engine.set_synth_param_typed(0, SynthParamId::Osc1LevelPct, 20.0),
        ScalarMutation::Unchanged
    );
    engine.set_synth_param_typed(0, SynthParamId::Osc1LevelPct, 100.0);
    render(&mut engine, GLIDE_FRAMES / 2);
    assert!(close(engine.instruments[0].osc1.level_pct, 75.0));
    render(&mut engine, GLIDE_FRAMES / 2);
    assert_eq!(engine.instruments[0].osc1.level_pct, 100.0);
}

fn level_engine(osc1_level_pct: f32, gain_pct: f32) -> SynthEngine {
    let mut synth = default_synth_config();
    synth.filter.cutoff_hz = 20_000.0;
    synth.osc1.level_pct = osc1_level_pct;
    synth.osc2.level_pct = 0.0;
    synth.amp.gain_pct = gain_pct;
    engine_from("synth", synth)
}

/// Renders `blocks` blocks of 120 frames, so four of them span the 480-frame glide exactly.
fn left_output(engine: &mut SynthEngine, blocks: usize) -> Vec<f32> {
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut rendered = Vec::new();
    for _ in 0..blocks {
        engine.render_interleaved_block(120, &mut left, &mut right, &mut out);
        rendered.extend_from_slice(&left);
    }
    rendered
}

/// The output of a gliding engine over that of a held one, at the frames loud enough to divide.
fn gain_ratios(glided: &[f32], held: &[f32]) -> Vec<(usize, f32)> {
    glided
        .iter()
        .zip(held)
        .enumerate()
        .filter(|(_, (_, held))| held.abs() > 1.0e-3)
        .map(|(frame, (glided, held))| (frame, glided / held))
        .collect()
}

#[test]
fn live_amp_gain_moves_every_sample_along_a_straight_line_then_holds_the_target() {
    let mut held = level_engine(80.0, 50.0);
    let mut glided = level_engine(80.0, 50.0);
    let mut direct = level_engine(80.0, 100.0);
    glided.set_synth_param_typed(0, SynthParamId::AmpGainPct, 100.0);

    let (held, glided, direct) = (
        left_output(&mut held, 6),
        left_output(&mut glided, 6),
        left_output(&mut direct, 6),
    );
    let ratios = gain_ratios(&glided[..GLIDE_FRAMES], &held[..GLIDE_FRAMES]);
    assert!(ratios.len() > GLIDE_FRAMES / 2);
    for (frame, ratio) in ratios {
        let expected = 1.0 + (frame + 1) as f32 / GLIDE_FRAMES as f32;
        assert!(
            close(ratio, expected),
            "frame {frame}: {ratio} vs {expected}"
        );
    }
    assert_eq!(glided[GLIDE_FRAMES..], direct[GLIDE_FRAMES..]);
}

#[test]
fn live_osc_level_moves_every_sample_along_a_straight_line() {
    let mut held = level_engine(40.0, 80.0);
    let mut glided = level_engine(40.0, 80.0);
    glided.set_synth_param_typed(0, SynthParamId::Osc1LevelPct, 80.0);

    let (held, glided) = (left_output(&mut held, 4), left_output(&mut glided, 4));
    let ratios = gain_ratios(&glided, &held);
    assert!(ratios.len() > GLIDE_FRAMES / 2);
    for (frame, ratio) in ratios {
        let expected = 1.0 + (frame + 1) as f32 / GLIDE_FRAMES as f32;
        assert!(
            (ratio - expected).abs() < 0.01,
            "frame {frame}: {ratio} vs {expected}"
        );
    }
}

#[test]
fn fm_index_glides_in_the_voice_source_the_voices_read() {
    let mut engine = engine_with("fm");
    let index = |engine: &SynthEngine| match engine.synth_render_configs[0].source {
        render_voice::VoiceSource::Fm { index, .. } => index,
        _ => panic!("fm source"),
    };
    let start = index(&engine);
    engine.set_fm_param_typed(0, FmParamId::Index, 50.0);
    render(&mut engine, GLIDE_FRAMES / 2);
    assert!(close(index(&engine), (start + 2.0) / 2.0));
    render(&mut engine, GLIDE_FRAMES / 2);
    assert_eq!(index(&engine), 2.0);
}

#[test]
fn slot_config_lands_a_running_glide_and_then_snaps_to_the_config() {
    let mut engine = engine_with("synth");
    engine.set_synth_param_typed(0, SynthParamId::FilterCutoffHz, 4_000.0);
    render(&mut engine, GLIDE_FRAMES / 4);

    let mut synth = default_synth_config();
    synth.filter.cutoff_hz = 300.0;
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "synth".into(),
            synth,
            mixer: None,
        },
    );
    assert_eq!(engine.instruments[0].filter.cutoff_hz, 300.0);
    render(&mut engine, GLIDE_FRAMES);
    assert_eq!(engine.instruments[0].filter.cutoff_hz, 300.0);
}

#[cfg(feature = "routing-tree-executor")]
#[test]
fn routing_tree_blocks_carry_the_glide_to_the_workers_block_by_block() {
    let mut engine = engine_with("synth");
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    let (mut left, mut right, mut out) = (vec![0.0; 128], vec![0.0; 128], vec![0.0; 256]);
    let mut cutoffs = Vec::new();
    for block in 0..5 {
        let disposition = engine.render_interleaved_block_with_source_runtime_ready_with_controls(
            &mut runtime,
            128,
            &mut left,
            &mut right,
            &mut out,
            |engine| {
                if block == 0 {
                    engine.set_synth_param_typed(0, SynthParamId::FilterCutoffHz, 4_000.0);
                }
                Ok(())
            },
        );
        assert_eq!(disposition, SourceWorkerRenderDisposition::Fresh);
        cutoffs.push(engine.instruments[0].filter.cutoff_hz);
    }
    let after_one_block = 1_000.0 * 4.0_f32.powf(128.0 / GLIDE_FRAMES as f32);
    assert!(close(cutoffs[0], after_one_block), "{cutoffs:?}");
    assert_eq!(cutoffs[3], 4_000.0);
    assert!(runtime.collect_wait_for_test(&mut engine));
    let shutdown = lifecycle.shutdown(runtime.retire());
    assert_eq!(shutdown.joined_workers, 2);
}

fn block(engine: &mut SynthEngine) -> Vec<f32> {
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
    out
}

#[test]
fn a_gliding_value_voices_read_live_reaches_the_sound_without_refreshing_voice_caches() {
    let targets = |engine: &mut SynthEngine| {
        engine.set_synth_param_typed(0, SynthParamId::Osc2PulseWidthPct, 20.0);
        engine.set_synth_param_typed(0, SynthParamId::Osc1LevelPct, 20.0);
        engine.set_synth_param_typed(0, SynthParamId::FilterCutoffHz, 3_000.0);
    };
    let mut gliding = engine_with("synth");
    targets(&mut gliding);
    let revision = gliding.synth_render_revisions[0];
    render(&mut gliding, GLIDE_FRAMES * 2);
    assert_eq!(gliding.synth_render_revisions[0], revision);

    let mut snapped = engine_with("synth");
    targets(&mut snapped);
    snapped.land_live_param_glides();
    render(&mut snapped, GLIDE_FRAMES * 2);
    // A level glide's per-sample ramp ends one block after its block value does.
    let _ = (block(&mut gliding), block(&mut snapped));
    let (gliding, snapped) = (block(&mut gliding), block(&mut snapped));
    assert!(gliding
        .iter()
        .zip(&snapped)
        .all(|(a, b)| (a - b).abs() < 1.0e-6));
}

#[test]
fn a_gliding_value_voices_cache_at_note_on_refreshes_them_while_it_moves() {
    let mut engine = engine_with("synth");
    engine.set_synth_param_typed(0, SynthParamId::Osc1DetuneCents, 50.0);
    let revision = engine.synth_render_revisions[0];
    render(&mut engine, GLIDE_FRAMES / 2);
    assert_ne!(engine.synth_render_revisions[0], revision);
}
