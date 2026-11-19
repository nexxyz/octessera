use super::super::*;
use crate::synth::{FmParamId, ScalarMutation, SynthParamId};

const SAMPLE_RATE: u32 = 48_000;
/// 10 ms at 48 kHz.
const GLIDE_FRAMES: usize = 480;

fn engine_with(kind: &str) -> SynthEngine {
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    let mut synth = default_synth_config();
    synth.filter.cutoff_hz = 1_000.0;
    synth.osc1.level_pct = 80.0;
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
