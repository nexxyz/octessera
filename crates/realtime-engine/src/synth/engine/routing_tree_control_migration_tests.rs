use super::{
    prepare_instruments_config, FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig,
    InstrumentSlotConfig, InstrumentsConfig, MixerConfig, SourceWorkerLifecycle,
    SourceWorkerRenderDisposition, SynthEngine,
};
use crate::synth::types::{default_synth_config, DEFAULT_PAN_POSITIONS};
use crate::synth::{MomentaryFxTarget, SampleBankConfig, SampleBuffer, SampleSlotConfig};
use serde_json::json;
use std::collections::BTreeMap;
use std::time::Duration;

const MIGRATION_SOURCE_SLOT: usize = 1;

fn shutdown(lifecycle: SourceWorkerLifecycle, runtime: super::SourceWorkerRuntime) {
    let retirement = runtime.retire();
    let report = lifecycle.shutdown(retirement);
    assert_eq!(report.joined_workers, 2);
}

#[test]
fn routing_tree_supports_topology_mutation_after_notes_start() {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(mapping_config(100.0));
    engine.note_on(0, 60, 100, 500);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let config = InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: None,
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    };

    let mut left = vec![0.0; 128];
    let mut right = vec![0.0; 128];
    let mut out = vec![0.0; 256];
    let mut retired = None;
    assert_eq!(
        engine.render_interleaved_block_with_source_runtime_ready_with_controls(
            &mut runtime,
            128,
            &mut left,
            &mut right,
            &mut out,
            |engine| {
                retired = Some(
                    engine.apply_prepared_instruments_config(prepare_instruments_config(
                        config, 44_100,
                    )),
                );
                Ok(())
            },
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    assert_eq!(retired.expect("retired config").prepared_slots.len(), 1);
    assert!(!engine.take_routing_tree_rejection());
    assert!(engine.routing_tree_assignment_is_valid());

    shutdown(lifecycle, runtime);
}

#[test]
fn routing_tree_supports_preview_and_local_momentary_at_control_gate() {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(mapping_config(100.0));
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let mut left = vec![0.0; 128];
    let mut right = vec![0.0; 128];
    let mut out = vec![0.0; 256];
    let preview = SampleBuffer {
        samples: std::sync::Arc::from(vec![0.25; 256]),
        channels: 1,
        sample_rate: 44_100,
    };
    let disposition = engine.render_interleaved_block_with_source_runtime_ready_with_controls(
        &mut runtime,
        128,
        &mut left,
        &mut right,
        &mut out,
        |engine| {
            drop(engine.preview_sample(0, preview, 100));
            engine.momentary_fx_start(
                "instrument-filter".into(),
                "filter_sweep".into(),
                BTreeMap::new(),
                MomentaryFxTarget::Instrument { index: 0 },
            );
            Ok(())
        },
    );
    assert_eq!(disposition, SourceWorkerRenderDisposition::Fresh);
    assert!(!engine.take_routing_tree_rejection());
    let mut next_left = vec![0.0; 128];
    let mut next_right = vec![0.0; 128];
    let mut next_out = vec![0.0; 256];
    assert_eq!(
        engine.render_interleaved_block_with_source_runtime(
            &mut runtime,
            128,
            &mut next_left,
            &mut next_right,
            &mut next_out,
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    assert_eq!(engine.profile_snapshot().active_preview_sample_voices, 1);
    assert_eq!(engine.profile_snapshot().active_momentary_fx, 1);
    assert!(next_out.iter().any(|sample| *sample != 0.0));
    shutdown(lifecycle, runtime);
}

#[test]
fn routing_tree_parameter_refresh_preserves_component_workers_after_counts_change() {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(mapping_config(100.0));
    engine.note_on(0, 36, 100, 50_000);
    engine.note_on(1, 48, 100, 50_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    let initial_mapping = [0, 1, 2].map(|slot| {
        engine
            .routing_tree_assignment
            .as_ref()
            .and_then(|assignment| assignment.worker_for_slot(slot))
    });
    assert_eq!(initial_mapping, [Some(0), Some(1), Some(0)]);
    let mut left = vec![0.0; 128];
    let mut right = vec![0.0; 128];
    let mut out = vec![0.0; 256];
    let mut retired_state = None;
    let prepared = {
        let mut config = mapping_config(100.0);
        config.instruments[0].synth.amp.gain_pct = 40.0;
        prepare_instruments_config(config, 44_100)
    };
    assert_eq!(
        engine.render_interleaved_block_with_source_runtime_ready_with_controls(
            &mut runtime,
            128,
            &mut left,
            &mut right,
            &mut out,
            |engine| {
                for note in 36..46 {
                    engine.note_on(0, note, 100, 50_000);
                }
                let retired = engine.apply_prepared_instruments_config(prepared);
                assert_eq!(retired.prepared_slots.len(), 3);
                retired_state = Some(retired);
                Ok(())
            },
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    let refreshed_mapping = [0, 1, 2].map(|slot| {
        engine
            .routing_tree_assignment
            .as_ref()
            .and_then(|assignment| assignment.worker_for_slot(slot))
    });
    assert_eq!(refreshed_mapping, initial_mapping);
    assert!(engine.routing_tree_assignment_is_valid());
    drop(retired_state);

    shutdown(lifecycle, runtime);
}

#[test]
fn synth_bus_routed_omission_follows_merged_bus_worker_on_next_quantum() {
    assert_source_migration_parity("synth", merged_bus_config(Vec::new()), None, 2);
}

#[test]
fn synth_bus_routed_none_keeps_direct_fallback_after_bus_removal() {
    assert_source_migration_parity(
        "synth",
        removed_bus_config(vec![migration_slot("none", "fx_bus_2")]),
        None,
        1,
    );
}

#[test]
fn sample_bus_routed_omission_follows_merged_bus_worker_on_next_quantum() {
    assert_source_migration_parity(
        "sampler",
        merged_bus_config(Vec::new()),
        Some(migration_sample_banks()),
        2,
    );
}

#[test]
fn sample_bus_routed_none_keeps_direct_fallback_after_bus_removal() {
    assert_source_migration_parity(
        "sampler",
        removed_bus_config(vec![migration_slot("none", "fx_bus_2")]),
        Some(migration_sample_banks()),
        1,
    );
}

fn assert_source_migration_parity(
    kind: &str,
    migrated: InstrumentsConfig,
    sample_banks: Option<Vec<SampleBankConfig>>,
    migrated_bus_count: usize,
) {
    let initial = migration_initial_config(kind);
    let mut routed = SynthEngine::new(44_100);
    let mut inline = SynthEngine::new(44_100);
    routed.set_instruments(initial.clone());
    inline.set_instruments(initial);
    if let Some(banks) = sample_banks {
        drop(routed.set_sample_banks(banks.clone()));
        drop(inline.set_sample_banks(banks));
    }
    for engine in [&mut routed, &mut inline] {
        engine.note_on(0, 36, 100, 5_000);
        engine.note_on(MIGRATION_SOURCE_SLOT as u8, 60, 100, 5_000);
    }
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut routed, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let old_worker = routed
        .routing_tree_assignment
        .as_ref()
        .and_then(|assignment| assignment.worker_for_slot(MIGRATION_SOURCE_SLOT))
        .expect("initial source worker");
    render_quantum(&mut inline, None, None);
    render_quantum(&mut routed, Some(&mut runtime), Some(migrated.clone()));
    inline.set_instruments(migrated);
    let routed_output = render_quantum(&mut routed, Some(&mut runtime), None);
    let inline_output = render_quantum(&mut inline, None, None);

    let assignment = routed
        .routing_tree_assignment
        .as_ref()
        .expect("migrated routing assignment");
    if migrated_bus_count == 2 {
        let merged_worker = assignment.worker_for_bus(1).expect("merged bus worker");
        assert_ne!(old_worker, merged_worker);
        assert_eq!(
            assignment.worker_for_slot(MIGRATION_SOURCE_SLOT),
            Some(merged_worker),
            "surviving bus-routed source must follow its resulting bus worker"
        );
    } else {
        assert_eq!(
            assignment.worker_for_slot(MIGRATION_SOURCE_SLOT),
            Some(old_worker)
        );
        assert!(matches!(
            routed.render_plan.instrument_slots[MIGRATION_SOURCE_SLOT].route,
            super::render_plan::RenderPlanRoute::Direct
        ));
    }
    assert!(!routed.take_routing_tree_rejection());
    super::routing_tree_pipeline_tests::assert_interleaved_reassociated_close(
        &runtime,
        &routed_output,
        &inline_output,
    );
    shutdown(lifecycle, runtime);
}

fn render_quantum(
    engine: &mut SynthEngine,
    runtime: Option<&mut super::SourceWorkerRuntime>,
    migrated: Option<InstrumentsConfig>,
) -> Vec<f32> {
    let mut left = vec![0.0; 128];
    let mut right = vec![0.0; 128];
    let mut out = vec![0.0; 256];
    let disposition = match (runtime, migrated) {
        (Some(runtime), Some(migrated)) => engine
            .render_interleaved_block_with_source_runtime_ready_with_controls(
                runtime,
                128,
                &mut left,
                &mut right,
                &mut out,
                |engine| {
                    let prepared = prepare_instruments_config(migrated, engine.sample_rate);
                    drop(engine.apply_prepared_instruments_config(prepared));
                    Ok(())
                },
            ),
        (Some(runtime), None) => engine.render_interleaved_block_with_source_runtime(
            runtime, 128, &mut left, &mut right, &mut out,
        ),
        (None, None) => {
            engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
            SourceWorkerRenderDisposition::Fresh
        }
        (None, Some(_)) => unreachable!("inline migration is applied between quantums"),
    };
    assert_eq!(disposition, SourceWorkerRenderDisposition::Fresh);
    out
}

fn migration_initial_config(kind: &str) -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![migration_anchor_slot(), migration_slot(kind, "fx_bus_2")],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig::default(), FxBusConfig::default()],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn merged_bus_config(instruments: Vec<InstrumentSlotConfig>) -> InstrumentsConfig {
    let merged_bus = FxBusConfig {
        slots: vec![FxBusSlotConfig::Config {
            kind: "duck".into(),
            params: BTreeMap::from([("source".into(), json!("B1"))]),
        }],
        ..FxBusConfig::default()
    };
    InstrumentsConfig {
        instruments,
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig::default(), merged_bus],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn removed_bus_config(instruments: Vec<InstrumentSlotConfig>) -> InstrumentsConfig {
    InstrumentsConfig {
        instruments,
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig::default()],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn migration_slot(kind: &str, route: &str) -> InstrumentSlotConfig {
    InstrumentSlotConfig {
        kind: kind.into(),
        synth: default_synth_config(),
        mixer: Some(InstrumentMixerConfig {
            route: route.into(),
            pan_pos: DEFAULT_PAN_POSITIONS / 2,
            volume: 100.0,
        }),
    }
}

fn migration_anchor_slot() -> InstrumentSlotConfig {
    migration_slot("synth", "direct")
}

fn migration_sample_banks() -> Vec<SampleBankConfig> {
    let mut banks = vec![SampleBankConfig::default(); 8];
    banks[MIGRATION_SOURCE_SLOT].slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![0.25; 4096].into(),
            channels: 1,
            sample_rate: 44_100,
        }),
    };
    banks
}

fn mapping_config(gain_pct: f32) -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: (0..3)
            .map(|_| InstrumentSlotConfig {
                kind: "synth".into(),
                synth: {
                    let mut synth = default_synth_config();
                    synth.amp.gain_pct = gain_pct;
                    synth
                },
                mixer: None,
            })
            .collect(),
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}
