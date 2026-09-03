use super::source_worker_test_fixtures::dynamic_engine;
use super::*;
use crate::synth::{
    FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MixerConfig, SampleBuffer,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn persistent_two_wave_render_matches_inline_for_full_bus_graph() {
    for reverse_completion in [false, true] {
        for frames in [32, 64, 128, 256, 2048] {
            assert_persistent_matches_inline(frames, reverse_completion);
        }
    }
}

fn assert_persistent_matches_inline(frames: usize, reverse_completion: bool) {
    let config = full_bus_config();
    let mut persistent = dynamic_engine();
    let mut inline = dynamic_engine();
    persistent.set_instruments(config.clone());
    inline.set_instruments(config);
    for engine in [&mut persistent, &mut inline] {
        engine.note_on(0, 36, 100, 5_000);
        engine.note_on(1, 60, 96, 5_000);
        engine.note_on(2, 67, 88, 5_000);
        engine.preview_sample(
            0,
            SampleBuffer {
                samples: Arc::from(vec![0.25, -0.1, 0.4, -0.2, 0.0]),
                channels: 1,
                sample_rate: 48_000,
            },
            100,
        );
        install_momentaries(engine);
    }
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut persistent).expect("persistent runtime");
    lifecycle.set_reverse_completion_for_test(reverse_completion);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let mut persistent_left = Vec::with_capacity(frames);
    let mut persistent_right = Vec::with_capacity(frames);
    let mut persistent_out = Vec::with_capacity(frames * 2);
    let mut inline_left = Vec::with_capacity(frames);
    let mut inline_right = Vec::with_capacity(frames);
    let mut inline_out = Vec::with_capacity(frames * 2);
    persistent.render_interleaved_block_with_source_runtime(
        &mut runtime,
        frames,
        &mut persistent_left,
        &mut persistent_right,
        &mut persistent_out,
    );
    inline.render_interleaved_block(frames, &mut inline_left, &mut inline_right, &mut inline_out);
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_eq!(persistent_out, inline_out);
    assert_eq!(persistent.sample_clock, inline.sample_clock);
    assert_eq!(
        persistent.active_bus_activity_count,
        inline.active_bus_activity_count
    );
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn persistent_two_wave_render_preserves_stateful_bus_tails_across_blocks() {
    let config = full_bus_config();
    let mut persistent = dynamic_engine();
    let mut inline = dynamic_engine();
    persistent.set_instruments(config.clone());
    inline.set_instruments(config);
    for engine in [&mut persistent, &mut inline] {
        engine.note_on(0, 36, 100, 5_000);
    }
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut persistent).expect("persistent runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let mut persistent_left = Vec::with_capacity(256);
    let mut persistent_right = Vec::with_capacity(256);
    let mut persistent_out = Vec::with_capacity(512);
    let mut inline_left = Vec::with_capacity(256);
    let mut inline_right = Vec::with_capacity(256);
    let mut inline_out = Vec::with_capacity(512);
    for _ in 0..3 {
        persistent.render_interleaved_block_with_source_runtime(
            &mut runtime,
            256,
            &mut persistent_left,
            &mut persistent_right,
            &mut persistent_out,
        );
        inline.render_interleaved_block(256, &mut inline_left, &mut inline_right, &mut inline_out);
        assert_eq!(persistent_out, inline_out);
    }
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_eq!(persistent.sample_clock, inline.sample_clock);
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn second_wave_dispatch_failure_faults_both_bus_owners_without_audio() {
    let mut persistent = dynamic_engine();
    persistent.set_instruments(full_bus_config());
    persistent.note_on(0, 36, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut persistent).expect("persistent runtime");
    let initial = runtime.home_owner_identities_for_test();
    runtime.set_before_bus_dispatch_hook_for_test(disconnect_second_bus_worker);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);
    persistent.render_interleaved_block_with_source_runtime(
        &mut runtime,
        128,
        &mut left,
        &mut right,
        &mut out,
    );
    assert!(out.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::DispatchFailed
    );
    for _ in 0..100_000 {
        let _ = runtime.collect_for_test(&mut persistent);
        if lifecycle.fault_owner_identities_for_test() == [Some(initial[0]), Some(initial[1])] {
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(
        lifecycle.fault_owner_identities_for_test(),
        [Some(initial[0]), Some(initial[1])]
    );
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

fn disconnect_second_bus_worker(runtime: &mut SourceWorkerRuntime, _deadline: &mut Instant) {
    runtime.disconnect_work_for_test(1);
}

#[test]
fn second_wave_panic_is_terminal_and_preserves_owner_for_shutdown() {
    let mut persistent = dynamic_engine();
    persistent.set_instruments(full_bus_config());
    persistent.note_on(0, 36, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut persistent).expect("persistent runtime");
    lifecycle.set_panic_on_bus_for_test(0);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);
    persistent.render_interleaved_block_with_source_runtime(
        &mut runtime,
        128,
        &mut left,
        &mut right,
        &mut out,
    );
    assert!(out.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::WorkerExited
    );
    for _ in 0..100_000 {
        let _ = runtime.collect_for_test(&mut persistent);
        if runtime.in_flight_mask_for_test() == 0 {
            break;
        }
        std::thread::yield_now();
    }
    let shutdown = lifecycle.shutdown(runtime.retire());
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.destroyed_owner_count, 2);
}

#[test]
fn second_wave_worker_exit_is_terminal_and_preserves_owner_for_shutdown() {
    let mut persistent = dynamic_engine();
    persistent.set_instruments(full_bus_config());
    persistent.note_on(0, 36, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut persistent).expect("persistent runtime");
    lifecycle.set_exit_on_bus_for_test(1);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);
    persistent.render_interleaved_block_with_source_runtime(
        &mut runtime,
        128,
        &mut left,
        &mut right,
        &mut out,
    );
    assert!(out.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::WorkerExited
    );
    for _ in 0..100_000 {
        let _ = runtime.collect_for_test(&mut persistent);
        if runtime.in_flight_mask_for_test() == 0 {
            break;
        }
        std::thread::yield_now();
    }
    let shutdown = lifecycle.shutdown(runtime.retire());
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.destroyed_owner_count, 2);
}

#[test]
fn second_wave_deadline_miss_is_terminal_and_preserves_owner_for_shutdown() {
    let mut persistent = dynamic_engine();
    persistent.set_instruments(full_bus_config());
    persistent.note_on(0, 36, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut persistent).expect("persistent runtime");
    runtime.set_before_bus_dispatch_hook_for_test(expire_second_wave_deadline);
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);
    persistent.render_interleaved_block_with_source_runtime(
        &mut runtime,
        128,
        &mut left,
        &mut right,
        &mut out,
    );
    assert!(out.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::DeadlineMiss
    );
    runtime.set_pause_for_parity_for_test(0, false);
    let shutdown = lifecycle.shutdown(runtime.retire());
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.destroyed_owner_count, 2);
}

pub(super) fn full_bus_config() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![
            instrument("sampler", "fx_bus_1"),
            instrument("synth", "fx_bus_2"),
            instrument("synth", "none"),
            instrument("sampler", "fx_bus_4"),
        ],
        mixer: Some(MixerConfig {
            buses: vec![
                bus(vec!["delay", "duck", "reverb"]),
                bus(vec!["glitch", "vinyl", "auto_pan"]),
                bus(vec!["chorus", "filter_lfo", "compressor"]),
                bus(vec!["distortion", "eq", "saturator"]),
            ],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn instrument(kind: &str, route: &str) -> InstrumentSlotConfig {
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

fn bus(kinds: Vec<&str>) -> FxBusConfig {
    FxBusConfig {
        slots: kinds
            .into_iter()
            .map(|kind| {
                if kind == "duck" {
                    FxBusSlotConfig::Config {
                        kind: kind.into(),
                        params: BTreeMap::from([("source".into(), json!("I2"))]),
                    }
                } else {
                    FxBusSlotConfig::Kind(kind.into())
                }
            })
            .collect(),
        ..FxBusConfig::default()
    }
}

fn install_momentaries(engine: &mut SynthEngine) {
    for (id, kind, target) in [
        (
            "instrument",
            "stutter",
            MomentaryFxTarget::Instrument { index: 0 },
        ),
        ("bus", "freeze", MomentaryFxTarget::FxBus { index: 0 }),
        ("global", "stutter", MomentaryFxTarget::Global),
    ] {
        let prepared = prepare_momentary_fx_start(
            id.into(),
            kind.into(),
            BTreeMap::new(),
            target,
            engine.sample_rate,
        )
        .expect("momentary FX");
        drop(engine.apply_prepared_momentary_fx_start(prepared));
    }
}

fn expire_second_wave_deadline(runtime: &mut SourceWorkerRuntime, deadline: &mut Instant) {
    runtime.set_pause_for_parity_for_test(0, true);
    *deadline = Instant::now();
}
