use super::source_worker_test_fixtures::dynamic_engine;
use super::source_worker_two_wave_tests::full_bus_config;
use super::*;
use std::time::Duration;

#[test]
fn persistent_two_wave_reactivation_uses_lower_projected_worker_load() {
    let config = full_bus_config();
    let mut persistent = dynamic_engine();
    let mut inline = dynamic_engine();
    persistent.set_instruments(config.clone());
    inline.set_instruments(config);
    persistent.note_on(0, 36, 100, 5_000);
    inline.note_on(0, 36, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut persistent).expect("persistent runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let mut persistent_left = Vec::with_capacity(128);
    let mut persistent_right = Vec::with_capacity(128);
    let mut persistent_out = Vec::with_capacity(256);
    let mut inline_left = Vec::with_capacity(128);
    let mut inline_right = Vec::with_capacity(128);
    let mut inline_out = Vec::with_capacity(256);
    persistent.render_interleaved_block_with_source_runtime(
        &mut runtime,
        128,
        &mut persistent_left,
        &mut persistent_right,
        &mut persistent_out,
    );
    inline.render_interleaved_block(128, &mut inline_left, &mut inline_right, &mut inline_out);
    assert_eq!(persistent_out, inline_out);
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_eq!(runtime.home_bus_carrier_assignments_for_test()[0][0], None);
    assert_eq!(
        runtime.home_bus_carrier_assignments_for_test()[1][0],
        Some(Some(1))
    );
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

#[test]
fn persistent_two_wave_quiet_boundary_returns_non_custodian_carrier_home() {
    let config = full_bus_config();
    let mut persistent = dynamic_engine();
    let mut inline = dynamic_engine();
    persistent.set_instruments(config.clone());
    inline.set_instruments(config);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut persistent).expect("persistent runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let initial_owners = runtime.home_owner_identities_for_test();
    let initial_carrier_scratch = runtime.home_bus_carrier_scratch_addresses_for_test();
    move_home_bus_carrier_to_worker(&runtime, 0, 1);
    assert_eq!(
        runtime.home_bus_carrier_assignments_for_test()[1][0],
        Some(Some(1))
    );
    let mut persistent_left = Vec::with_capacity(128);
    let mut persistent_right = Vec::with_capacity(128);
    let mut persistent_out = Vec::with_capacity(256);
    let mut inline_left = Vec::with_capacity(128);
    let mut inline_right = Vec::with_capacity(128);
    let mut inline_out = Vec::with_capacity(256);
    for _ in 0..(48_000usize * 250 / 1000 / 128) {
        persistent.render_interleaved_block_with_source_runtime(
            &mut runtime,
            128,
            &mut persistent_left,
            &mut persistent_right,
            &mut persistent_out,
        );
        inline.render_interleaved_block(128, &mut inline_left, &mut inline_right, &mut inline_out);
        assert_eq!(persistent_out, inline_out);
    }
    assert_eq!(
        runtime.home_bus_carrier_assignments_for_test()[1][0],
        Some(Some(1))
    );
    persistent.render_interleaved_block_with_source_runtime(
        &mut runtime,
        96,
        &mut persistent_left,
        &mut persistent_right,
        &mut persistent_out,
    );
    inline.render_interleaved_block(96, &mut inline_left, &mut inline_right, &mut inline_out);
    assert_eq!(persistent_out, inline_out);
    assert_eq!(
        runtime.health_snapshot().status,
        SourceWorkerHealth::Healthy
    );
    assert_eq!(
        runtime.home_bus_carrier_assignments_for_test()[0][0],
        Some(None)
    );
    assert_eq!(runtime.home_bus_carrier_assignments_for_test()[1][0], None);
    assert_eq!(runtime.home_owner_identities_for_test(), initial_owners);
    assert_eq!(
        runtime.home_bus_carrier_scratch_addresses_for_test(),
        initial_carrier_scratch
    );
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

fn move_home_bus_carrier_to_worker(
    runtime: &SourceWorkerRuntime,
    logical_bus_id: usize,
    worker: usize,
) {
    let mut owners = runtime.take_home_owners_for_test().expect("owner pair");
    let source = owners
        .iter()
        .position(|owner| owner.bus_carriers[logical_bus_id].is_some())
        .expect("bus carrier");
    let mut carrier = owners[source].bus_carriers[logical_bus_id]
        .take()
        .expect("bus carrier");
    carrier
        .owner
        .as_mut()
        .expect("configured bus owner")
        .assigned_worker = Some(worker);
    assert!(owners[worker].bus_carriers[logical_bus_id].is_none());
    owners[worker].bus_carriers[logical_bus_id] = Some(carrier);
    runtime.return_home_owners_for_test(owners);
}

#[test]
fn forged_wrong_bus_residency_fails_before_audio_commit() {
    let mut persistent = dynamic_engine();
    persistent.set_instruments(full_bus_config());
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut persistent).expect("persistent runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    move_home_bus_carrier_to_worker(&runtime, 0, 1);
    runtime.set_after_bus_dispatch_hook_for_test(forge_wrong_bus_residency);
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
        SourceWorkerHealth::CompletionFailed
    );
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

fn forge_wrong_bus_residency(runtime: &mut SourceWorkerRuntime) {
    for _ in 0..100_000 {
        if runtime.completion_ready_for_test(0) && runtime.completion_ready_for_test(1) {
            assert!(runtime.swap_completion_carrier_for_test(0));
            return;
        }
        std::thread::yield_now();
    }
    panic!("bus completions did not arrive");
}
