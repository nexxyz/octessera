use super::routing_tree_executor_test_support::*;
use super::routing_tree_plan::RoutingTreePlan;
use super::*;
use crate::synth::SampleBuffer;
use std::collections::BTreeMap;

#[cfg(feature = "routing-tree-benchmark")]
pub(super) fn engine_source_state_signature(engine: &SynthEngine) -> String {
    format!(
        "{:?}|{:?}|{:?}",
        active_synth_voice_state(engine),
        active_sample_voice_state(engine),
        format!("{:?}", engine.preview_sample_voices),
    )
}

#[test]
fn routing_tree_rejects_stale_or_malformed_plans_without_state_commit() {
    let config = invalid_state_config();
    let mut engine = SynthEngine::new(48_000);
    let mut canonical = SynthEngine::new(48_000);
    engine.set_instruments(config.clone());
    canonical.set_instruments(config);
    engine.set_sample_banks(sample_banks());
    canonical.set_sample_banks(sample_banks());
    for target in [&mut engine, &mut canonical] {
        target.note_on(0, 36, 127, 1_000);
        target.note_on(1, 60, 100, 1_000);
        target.note_on(2, 67, 100, 1_000);
        target.preview_sample(
            0,
            SampleBuffer {
                samples: vec![0.25, 0.5, 0.25].into(),
                channels: 1,
                sample_rate: 48_000,
            },
            100,
        );
        for (id, target_kind) in [
            ("instrument", MomentaryFxTarget::Instrument { index: 1 }),
            ("bus", MomentaryFxTarget::FxBus { index: 0 }),
            ("global", MomentaryFxTarget::Global),
        ] {
            target.momentary_fx_start(
                id.into(),
                "filter_sweep".into(),
                BTreeMap::new(),
                target_kind,
            );
        }
    }
    let base_plan = RoutingTreePlan::from_render_plan(&engine.render_plan);
    let before = engine_state_signature(&engine);
    let mut left = vec![1.0; 64];
    let mut right = vec![1.0; 64];

    let mut stale = base_plan;
    stale.generation = stale.generation.wrapping_add(1);
    assert!(!engine.render_routing_tree_block_with_plan_for_test(stale, 64, &mut left, &mut right));
    assert!(left.iter().all(|sample| *sample == 0.0));
    assert!(right.iter().all(|sample| *sample == 0.0));
    assert_eq!(engine_state_signature(&engine), before);

    left.fill(1.0);
    right.fill(1.0);
    let mut malformed = base_plan;
    malformed.component_count += 1;
    assert!(
        !engine.render_routing_tree_block_with_plan_for_test(malformed, 64, &mut left, &mut right)
    );
    assert!(left.iter().all(|sample| *sample == 0.0));
    assert!(right.iter().all(|sample| *sample == 0.0));
    assert_eq!(engine_state_signature(&engine), before);

    let mut expected_left = vec![0.0; 128];
    let mut expected_right = vec![0.0; 128];
    let mut actual_left = vec![0.0; 128];
    let mut actual_right = vec![0.0; 128];
    let mut expected_interleaved = Vec::new();
    let mut actual_interleaved = Vec::new();
    canonical.render_interleaved_block(
        128,
        &mut expected_left,
        &mut expected_right,
        &mut expected_interleaved,
    );
    engine.render_interleaved_block(
        128,
        &mut actual_left,
        &mut actual_right,
        &mut actual_interleaved,
    );
    assert_eq!(actual_left, expected_left);
    assert_eq!(actual_right, expected_right);
    assert_eq!(actual_interleaved, expected_interleaved);
    assert_eq!(
        engine_state_signature(&engine),
        engine_state_signature(&canonical)
    );
}
