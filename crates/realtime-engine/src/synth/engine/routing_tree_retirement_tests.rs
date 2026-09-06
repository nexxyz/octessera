use super::source_worker_test_fixtures::sample_engine_with_shared_buffer;
use super::*;
use crate::synth::types::{default_synth_config, DEFAULT_PAN_POSITIONS};
use crate::synth::{MomentaryFxTarget, SampleBuffer};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn routing_tree_completed_preview_and_momentary_payloads_retire_with_fifo_owners() {
    let samples: Arc<[f32]> = Arc::from(vec![0.25]);
    let mut engine = retirement_engine();
    drop(engine.preview_sample(
        0,
        SampleBuffer {
            samples: Arc::clone(&samples),
            channels: 1,
            sample_rate: 44_100,
        },
        100,
    ));
    engine.momentary_fx_start(
        "retiring".into(),
        "stutter".into(),
        BTreeMap::new(),
        MomentaryFxTarget::Instrument { index: 0 },
    );
    engine.momentary_fx[0].releasing = true;
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    let owners = runtime.take_home_owners_for_test().expect("worker owners");
    let retired_preview = owners
        .iter()
        .filter_map(|owner| owner.routing_tree.as_ref())
        .flat_map(|owner| owner.retired_preview_samples.iter())
        .flatten()
        .count();
    let retired_momentary = owners
        .iter()
        .filter_map(|owner| owner.routing_tree.as_ref())
        .flat_map(|owner| owner.retired_momentary_fx.iter())
        .flatten()
        .count();
    assert_eq!(retired_preview, 1);
    assert_eq!(retired_momentary, 1);
    runtime.return_home_owners_for_test(owners);
    let report = lifecycle.shutdown(runtime.retire());
    assert_eq!(report.joined_workers, 2);
    assert_eq!(report.destroyed_owner_count, 2);
    assert_eq!(Arc::strong_count(&samples), 1);
}

#[test]
fn routing_tree_no_control_preserves_nonempty_sample_preview_refs_and_owners() {
    let samples: Arc<[f32]> = Arc::from(vec![0.25; 16_384]);
    let mut engine = sample_engine_with_shared_buffer(Arc::clone(&samples));
    engine.note_on(0, 36, 100, 5_000);
    drop(engine.preview_sample(
        0,
        SampleBuffer {
            samples: Arc::clone(&samples),
            channels: 1,
            sample_rate: 48_000,
        },
        100,
    ));
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let initial_owners = runtime.home_owner_identities_for_test();
    let initial_refs = Arc::strong_count(&samples);
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut out = Vec::new();
    assert_eq!(
        engine.render_interleaved_block_with_source_runtime(
            &mut runtime,
            128,
            &mut left,
            &mut right,
            &mut out,
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    assert!(runtime.collect_wait_for_test(&mut engine));
    assert_eq!(runtime.home_owner_identities_for_test(), initial_owners);
    assert_eq!(Arc::strong_count(&samples), initial_refs);
    assert_eq!(engine.profile_snapshot().active_sample_voices, 1);
    assert_eq!(engine.profile_snapshot().active_preview_sample_voices, 1);
    assert!(out.iter().any(|sample| sample.abs() > 0.0));
    let report = lifecycle.shutdown(runtime.retire());
    assert_eq!(report.joined_workers, 2);
    assert!(Arc::strong_count(&samples) < initial_refs);
}

fn retirement_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: None,
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine
}
