use super::super::*;
use super::drum_tests::drum_engine;
use crate::synth::DrumConfig;

#[test]
fn drum_inline_block_and_worker_match_without_callback_allocation() {
    let mut block = drum_engine(DrumConfig::default(), 44_100);
    let mut scalar = drum_engine(DrumConfig::default(), 44_100);
    let mut worker = drum_engine(DrumConfig::default(), 44_100);
    for engine in [&mut block, &mut scalar, &mut worker] {
        engine.drum_hit(0, 0, 0, 120);
        engine.drum_hit(0, 1, -5, 100);
    }
    let mut left = Vec::with_capacity(256);
    let mut right = Vec::with_capacity(256);
    let mut out = Vec::with_capacity(512);
    let (lifecycle, mut runtime) = SourceWorkerLifecycle::start_prewarmed(&mut worker).unwrap();
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    for frames in [64, 128, 256] {
        block.render_interleaved_block(frames, &mut left, &mut right, &mut out);
        let expected = (0..frames)
            .flat_map(|_| {
                let (l, r) = scalar.next_stereo_sample();
                [l, r]
            })
            .collect::<Vec<_>>();
        for (a, b) in out.iter().zip(&expected) {
            assert_eq!(a.to_bits(), b.to_bits());
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
        for (a, b) in out.iter().zip(&expected) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
    let ((), allocations, _) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            block.drum_hit(0, 2, 24, 127);
            block.render_interleaved_block(128, &mut left, &mut right, &mut out);
        });
    assert_eq!(allocations, 0);
}

#[cfg(feature = "routing-tree-benchmark")]
#[test]
fn drum_routing_tree_worker_matches_inline_for_distinct_hits() {
    let mut worker = drum_engine(DrumConfig::default(), 44_100);
    let mut inline = drum_engine(DrumConfig::default(), 44_100);
    for engine in [&mut worker, &mut inline] {
        engine.drum_hit(0, 0, -24, 120);
        engine.drum_hit(0, 2, 24, 100);
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
