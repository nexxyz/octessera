use super::routing_tree_momentary_test_support::momentary_state;
use super::routing_tree_pipeline_tests::{assert_interleaved_reassociated_close, shutdown};
use super::{
    InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig, SourceWorkerLifecycle,
    SourceWorkerRenderDisposition, SynthEngine,
};
use crate::synth::types::{default_synth_config, DEFAULT_PAN_POSITIONS};
use crate::synth::{MomentaryFxTarget, SourceWorkerRuntime};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn routing_tree_pitch_release_matches_inline_through_retirement() {
    let config = pitch_release_config();
    let mut routed = SynthEngine::new(44_100);
    let mut inline = SynthEngine::new(44_100);
    for engine in [&mut routed, &mut inline] {
        engine.set_instruments(config.clone());
        engine.note_on(0, 60, 100, 10_000);
        engine.momentary_fx_start(
            "pitch".into(),
            "pitch_shift".into(),
            BTreeMap::from([
                ("mixPct".into(), json!(100.0)),
                ("semitones".into(), json!(7.0)),
                ("cents".into(), json!(0.0)),
                ("slideInMs".into(), json!(10.0)),
                ("slideOutMs".into(), json!(10.0)),
            ]),
            MomentaryFxTarget::Global,
        );
    }
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut routed, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));

    for block in 0..14 {
        let routed_out = if block == 0 {
            render_routed_ready_block(&mut routed, &mut runtime)
        } else {
            render_routed_block(&mut routed, &mut runtime)
        };
        let mut inline_left = vec![0.0; 128];
        let mut inline_right = vec![0.0; 128];
        let mut inline_out = vec![0.0; 256];
        inline.render_interleaved_block(128, &mut inline_left, &mut inline_right, &mut inline_out);
        assert_interleaved_reassociated_close(&runtime, &routed_out, &inline_out);
        assert_eq!(momentary_state(&routed), momentary_state(&inline));
    }
    assert!(runtime.collect_wait_for_test(&mut routed));

    assert!(runtime
        .with_controls_ready(&mut routed, |engine| {
            engine.momentary_fx_stop("pitch");
        })
        .is_some());
    inline.momentary_fx_stop("pitch");

    for _ in 0..5 {
        let routed_out = render_routed_block(&mut routed, &mut runtime);
        let mut inline_left = vec![0.0; 128];
        let mut inline_right = vec![0.0; 128];
        let mut inline_out = vec![0.0; 256];
        inline.render_interleaved_block(128, &mut inline_left, &mut inline_right, &mut inline_out);
        assert_eq!(
            routed_out.len(),
            inline_out.len(),
            "routing-tree release output geometry"
        );
        assert_interleaved_reassociated_close(&runtime, &routed_out, &inline_out);
        assert_eq!(momentary_state(&routed), momentary_state(&inline));
    }
    assert!(routed.momentary_fx.is_empty());
    assert!(inline.momentary_fx.is_empty());
    shutdown(lifecycle, runtime);
}

fn render_routed_block(engine: &mut SynthEngine, runtime: &mut SourceWorkerRuntime) -> Vec<f32> {
    let mut left = vec![0.0; 128];
    let mut right = vec![0.0; 128];
    let mut output = vec![0.0; 256];
    assert_eq!(
        engine.render_interleaved_block_with_source_runtime(
            runtime,
            128,
            &mut left,
            &mut right,
            &mut output,
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    output
}

fn render_routed_ready_block(
    engine: &mut SynthEngine,
    runtime: &mut SourceWorkerRuntime,
) -> Vec<f32> {
    let mut left = vec![0.0; 128];
    let mut right = vec![0.0; 128];
    let mut output = vec![0.0; 256];
    assert_eq!(
        engine.render_interleaved_block_with_source_runtime_ready_with_controls(
            runtime,
            128,
            &mut left,
            &mut right,
            &mut output,
            |_| Ok(()),
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    output
}

fn pitch_release_config() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "direct".into(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}
