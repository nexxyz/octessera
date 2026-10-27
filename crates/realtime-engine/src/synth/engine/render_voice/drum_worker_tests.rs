use super::super::*;
use super::drum_tests::drum_engine;
use crate::synth::DrumConfig;

#[cfg(feature = "routing-tree-executor")]
#[test]
fn drum_routing_tree_worker_matches_inline_for_distinct_hits() {
    let mut worker = drum_engine(DrumConfig::default(), 44_100);
    let mut inline = drum_engine(DrumConfig::default(), 44_100);
    for engine in [&mut worker, &mut inline] {
        engine.cell_hit(0, 0, 0, -2400, 120);
        engine.cell_hit(0, 2, 0, 2400, 100);
    }
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut worker, 128).unwrap();
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);
    for _ in 0..100 {
        assert_eq!(
            worker.render_interleaved_block_with_source_runtime(
                &mut runtime,
                128,
                &mut left,
                &mut right,
                &mut out
            ),
            SourceWorkerRenderDisposition::Fresh
        );
        let mut expected_left = Vec::with_capacity(128);
        let mut expected_right = Vec::with_capacity(128);
        let mut expected = Vec::with_capacity(256);
        inline.render_interleaved_block(
            128,
            &mut expected_left,
            &mut expected_right,
            &mut expected,
        );
        for (a, b) in out.iter().zip(expected) {
            assert!((a - b).abs() < 1e-5, "routing Drum differs: {a} vs {b}");
        }
    }
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}
