use super::*;
use crate::synth::{SampleBankConfig, SampleBuffer, SampleSlotConfig, VoiceStealingMode};

fn observed_routing_cost(active: bool) -> [u16; 2] {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(bus_config_with_reverb());
    if active {
        engine.note_on(0, 60, 100, 500);
    }
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let disposition = render_block(&mut engine, &mut runtime);
    assert_eq!(disposition, SourceWorkerRenderDisposition::Fresh);
    let observed = runtime.load_snapshot().expect("routing worker load");
    let result = observed.observed_active_cost_units;
    shutdown(lifecycle, runtime);
    result
}

#[test]
fn routing_tree_actual_threaded_active_bus_cost_is_counted_once() {
    assert_eq!(
        observed_routing_cost(true),
        [
            SOURCE_WORKER_SYNTH_COST_UNITS + BUS_CHAIN_SLOT_COST_UNITS - 1,
            0
        ]
    );
}

#[test]
fn routing_tree_actual_threaded_quiet_bus_cost_is_zero() {
    assert_eq!(observed_routing_cost(false), [0, 0]);
}

#[test]
fn routing_tree_actual_threaded_over_max_combined_completion_is_terminal() {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(bus_config_with_reverb());
    engine.note_on(0, 60, 100, 500);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    assert!(runtime.dispatch_routing_tree_for_test(&engine, 128, engine.sample_clock()));
    for parity in 0..2 {
        for _ in 0..100_000 {
            if runtime.completion_ready_for_test(parity) {
                break;
            }
            thread::yield_now();
        }
    }
    assert!(runtime.rewrite_completion_measurement_for_test(
        0,
        1_000_000,
        super::super::routing_tree_worker::ROUTING_TREE_MAX_COST_UNITS + 1,
    ));
    assert!(!runtime.collect_wait_for_test(&mut engine));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::CompletionFailed
    );
    shutdown(lifecycle, runtime);
}

fn partition_fill_config(kind: &str) -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: (0..8)
            .map(|_| InstrumentSlotConfig {
                kind: kind.into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "fx_bus_1".into(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig::default()],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

#[test]
fn routing_tree_synth_note_admission_reaches_global_capacity_without_routing_rejection() {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(partition_fill_config("synth"));
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    assert_eq!(
        engine
            .routing_tree_assignment
            .as_ref()
            .and_then(|assignment| assignment.worker_for_slot(0)),
        Some(0)
    );
    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            let per_slot = crate::synth::MAX_SYNTH_VOICES_PER_SLOT;
            for slot in 0..8 {
                for note in 0..per_slot {
                    engine.note_on(slot as u8, 36 + note as u8, 100, 5_000);
                }
            }
        })
        .is_some());
    let output = render_full_capacity(&mut runtime, &mut engine);
    assert!(output.iter().any(|sample| sample.abs() > 0.0001));
    let before = engine.profile_snapshot();
    assert_eq!(
        before.active_synth_voices,
        crate::synth::types::SYNTH_VOICE_LANE_CAPACITY
    );
    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            engine.note_on(0, 36, 100, 5_000);
        })
        .is_some());
    assert!(!engine.take_routing_tree_rejection());
    let after = recovered_profile(&mut runtime, &mut engine);
    assert_eq!(after.active_synth_voices, before.active_synth_voices);
    assert_eq!(
        after.cumulative_voice_admission_drops,
        before.cumulative_voice_admission_drops + 1
    );
    assert!(engine.routing_tree_assignment_is_valid());
    shutdown(lifecycle, runtime);
}

fn sample_partition_banks() -> Vec<SampleBankConfig> {
    let mut banks = vec![SampleBankConfig::default(); 8];
    for bank in banks.iter_mut().take(8) {
        bank.slots[0] = SampleSlotConfig {
            buffer: Some(SampleBuffer {
                samples: vec![0.25; 4096].into(),
                channels: 1,
                sample_rate: 44_100,
            }),
        };
    }
    banks
}

#[test]
fn routing_tree_sample_note_admission_reaches_global_capacity_without_routing_rejection() {
    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(partition_fill_config("sampler"));
    drop(engine.set_sample_banks(sample_partition_banks()));
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    assert_eq!(
        engine
            .routing_tree_assignment
            .as_ref()
            .and_then(|assignment| assignment.worker_for_slot(0)),
        Some(0)
    );
    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            let per_slot = crate::synth::MAX_SAMPLE_VOICES_PER_SLOT;
            for slot in 0..8 {
                for _ in 0..per_slot {
                    engine.note_on(slot as u8, 36, 100, 5_000);
                }
            }
        })
        .is_some());
    let output = render_full_capacity(&mut runtime, &mut engine);
    assert!(output.iter().any(|sample| sample.abs() > 0.0001));
    let before = engine.profile_snapshot();
    assert_eq!(
        before.active_sample_voices,
        crate::synth::types::SAMPLE_VOICE_LANE_CAPACITY
    );
    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            engine.note_on(0, 36, 100, 5_000);
        })
        .is_some());
    assert!(!engine.take_routing_tree_rejection());
    let after = recovered_profile(&mut runtime, &mut engine);
    assert_eq!(after.active_sample_voices, before.active_sample_voices);
    assert_eq!(
        after.cumulative_voice_admission_drops,
        before.cumulative_voice_admission_drops + 1
    );
    assert!(engine.routing_tree_assignment_is_valid());
    shutdown(lifecycle, runtime);
}

#[test]
fn routing_tree_hot_swap_renders_full_surviving_synth_and_sample_capacity() {
    let mut engine = SynthEngine::new(44_100);
    let mut reference = SynthEngine::new(44_100);
    engine.set_instruments(partition_fill_config("synth"));
    reference.set_instruments(partition_fill_config("synth"));
    drop(engine.set_sample_banks(sample_partition_banks()));
    drop(reference.set_sample_banks(sample_partition_banks()));
    engine.set_voice_stealing_mode(VoiceStealingMode::None);
    reference.set_voice_stealing_mode(VoiceStealingMode::None);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    assert!(runtime
        .with_controls_ready(&mut engine, |engine| {
            let per_slot = crate::synth::MAX_SYNTH_VOICES_PER_SLOT;
            for slot in 0..8 {
                for note in 0..per_slot {
                    engine.note_on(slot as u8, 36 + note as u8, 100, 5_000);
                }
            }
        })
        .is_some());
    for slot in 0..8 {
        for note in 0..crate::synth::MAX_SYNTH_VOICES_PER_SLOT {
            reference.note_on(slot as u8, 36 + note as u8, 100, 5_000);
        }
    }
    let synth_output = render_full_capacity(&mut runtime, &mut engine);
    let _ = render_inline_full_capacity(&mut reference);
    assert!(synth_output.iter().any(|sample| sample.abs() > 0.0001));
    assert_eq!(
        engine.profile_snapshot().active_synth_voices,
        crate::synth::types::SYNTH_VOICE_LANE_CAPACITY
    );

    let sample_config = partition_fill_config("sampler");
    let reference_sample_config = sample_config.clone();
    let disposition = {
        let mut left = vec![0.0; 128];
        let mut right = vec![0.0; 128];
        let mut out = vec![0.0; 256];
        engine.render_interleaved_block_with_source_runtime_ready_with_controls(
            &mut runtime,
            128,
            &mut left,
            &mut right,
            &mut out,
            |engine| {
                engine.set_instruments(sample_config);
                for slot in 0..8 {
                    for _ in 0..crate::synth::MAX_SAMPLE_VOICES_PER_SLOT {
                        engine.note_on(slot as u8, 36, 100, 5_000);
                    }
                }
                Ok(())
            },
        )
    };
    assert_eq!(disposition, SourceWorkerRenderDisposition::Fresh);
    assert!(!engine.take_routing_tree_rejection());
    let _ = render_inline_block(&mut reference);
    reference.set_instruments(reference_sample_config);
    for slot in 0..8 {
        for _ in 0..crate::synth::MAX_SAMPLE_VOICES_PER_SLOT {
            reference.note_on(slot as u8, 36, 100, 5_000);
        }
    }
    let mixed_output = render_full_capacity(&mut runtime, &mut engine);
    let expected_output = render_inline_full_capacity(&mut reference);
    assert_eq!(mixed_output, expected_output);
    let snapshot = engine.profile_snapshot();
    assert_eq!(
        snapshot.active_synth_voices,
        crate::synth::types::SYNTH_VOICE_LANE_CAPACITY
    );
    assert_eq!(
        snapshot.active_sample_voices,
        crate::synth::types::SAMPLE_VOICE_LANE_CAPACITY
    );
    assert!(engine.routing_tree_assignment_is_valid());
    shutdown(lifecycle, runtime);
}

fn recovered_profile(
    runtime: &mut SourceWorkerRuntime,
    engine: &mut SynthEngine,
) -> crate::synth::SynthProfileSnapshot {
    runtime
        .with_recovered_owners(engine, |engine| engine.profile_snapshot())
        .expect("recovered source owners")
}

fn render_full_capacity(runtime: &mut SourceWorkerRuntime, engine: &mut SynthEngine) -> Vec<f32> {
    let mut left = vec![0.0; 128];
    let mut right = vec![0.0; 128];
    let mut out = vec![0.0; 256];
    assert_eq!(
        engine.render_interleaved_block_with_source_runtime(
            runtime, 128, &mut left, &mut right, &mut out,
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    assert_eq!(
        engine.render_interleaved_block_with_source_runtime(
            runtime, 128, &mut left, &mut right, &mut out,
        ),
        SourceWorkerRenderDisposition::Fresh
    );
    assert!(runtime.collect_wait_for_test(engine));
    out
}

fn render_inline_block(engine: &mut SynthEngine) -> Vec<f32> {
    let mut left = vec![0.0; 128];
    let mut right = vec![0.0; 128];
    let mut out = vec![0.0; 256];
    engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
    out
}

fn render_inline_full_capacity(engine: &mut SynthEngine) -> Vec<f32> {
    let _ = render_inline_block(engine);
    render_inline_block(engine)
}
