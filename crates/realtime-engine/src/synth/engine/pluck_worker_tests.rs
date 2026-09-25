use super::super::*;
use super::pluck_tests::pluck_engine;
use crate::synth::{PluckConfig, PluckParamId, ScalarMutation};

#[test]
fn pluck_block_worker_scalar_parity_and_allocation_free_callback() {
    let mut cfg = PluckConfig::default();
    cfg.filter.key_tracking_pct = 100.0;
    cfg.filter.env_amount_pct = 30.0;
    let mut block = pluck_engine(cfg, 44_100);
    let mut scalar = pluck_engine(cfg, 44_100);
    let mut worker = pluck_engine(cfg, 44_100);
    for engine in [&mut block, &mut scalar, &mut worker] {
        engine.note_on(0, 48, 120, 10_000);
    }
    let mut left = Vec::with_capacity(256);
    let mut right = Vec::with_capacity(256);
    let mut out = Vec::with_capacity(512);
    let (lifecycle, mut runtime) = SourceWorkerLifecycle::start_prewarmed(&mut worker).unwrap();
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    for (frames, brightness) in [(64, 20.0), (128, 85.0), (256, 65.0)] {
        for engine in [&mut block, &mut scalar] {
            assert_eq!(
                engine.set_pluck_param_typed(0, PluckParamId::BrightnessPct, brightness),
                ScalarMutation::Changed
            );
        }
        assert_eq!(
            runtime.with_controls_ready(&mut worker, |engine| engine.set_pluck_param_typed(
                0,
                PluckParamId::BrightnessPct,
                brightness
            )),
            Some(ScalarMutation::Changed)
        );
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
    let ((), allocs, _) = crate::synth::test_allocator::count_allocations_and_deallocations(|| {
        block.render_interleaved_block(256, &mut left, &mut right, &mut out);
    });
    assert_eq!(allocs, 0);
}

#[cfg(feature = "routing-tree-benchmark")]
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
