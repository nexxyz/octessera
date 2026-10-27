use super::super::*;
use super::pluck_tests::pluck_engine;
use crate::synth::PluckConfig;

#[cfg(feature = "routing-tree-executor")]
#[test]
fn pluck_routing_tree_worker_preserves_ring_and_matches_inline() {
    let cfg = PluckConfig::default();
    let mut worker = pluck_engine(cfg, 44_100);
    let mut inline = pluck_engine(cfg, 44_100);
    for engine in [&mut worker, &mut inline] {
        engine.note_on(0, 69, 120, 10_000);
        engine.note_on(0, 0, 120, 10_000);
    }
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut worker, 128).unwrap();
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    let mut left = Vec::with_capacity(256);
    let mut right = Vec::with_capacity(256);
    let mut output = Vec::with_capacity(512);
    for _ in 0..100 {
        let frames = 128;
        assert_eq!(
            worker.render_interleaved_block_with_source_runtime(
                &mut runtime,
                frames,
                &mut left,
                &mut right,
                &mut output
            ),
            SourceWorkerRenderDisposition::Fresh
        );
        let mut expected_left = Vec::with_capacity(frames);
        let mut expected_right = Vec::with_capacity(frames);
        let mut expected = Vec::with_capacity(frames * 2);
        inline.render_interleaved_block(
            frames,
            &mut expected_left,
            &mut expected_right,
            &mut expected,
        );
        assert_eq!(output.len(), expected.len());
        for (a, b) in output.iter().zip(expected) {
            assert!((a - b).abs() < 1e-5, "routing output differs: {a} vs {b}");
        }
    }
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}
