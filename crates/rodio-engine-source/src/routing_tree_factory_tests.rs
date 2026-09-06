use super::*;
use crossbeam_channel::bounded;
use realtime_engine::synth::{
    default_synth_config, install_source_worker_shutdown_probe_for_test, InstrumentSlotConfig,
    InstrumentsConfig, SourceWorkerSetupError, DEFAULT_PAN_POSITIONS,
};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn successful_start_hook(_: usize) -> Result<(), ()> {
    Ok(())
}

fn fail_parity_one_start_hook(parity: usize) -> Result<(), ()> {
    (parity != 1).then_some(()).ok_or(())
}

#[test]
fn product_routing_constructor_uses_one_quantum_of_lookahead() {
    let (_tx, rx) = event_queue();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, 128, None)
            .expect("routing-tree runtime");

    assert!(matches!(
        source.worker_state.mode,
        EngineSourceMode::RoutingTreePersistent
    ));
    assert_eq!(source.lookahead_frames(), 128);
    for _ in 0..256 {
        assert_eq!(source.next(), Some(0.0));
    }

    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn product_routing_constructor_accepts_worker_start_hook() {
    let (_tx, rx) = event_queue();
    let (source, shutdown) = EngineSource::with_routing_tree_persistent_workers_with_hook(
        rx,
        44_100,
        128,
        None,
        successful_start_hook,
    )
    .expect("routing-tree runtime");

    assert_eq!(source.lookahead_frames(), 128);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn product_routing_constructor_rejects_invalid_frames_before_setup() {
    for block_frames in [31, 2049] {
        let reaper_spawn_failure = crate::source_worker_reaper::fail_next_reaper_spawn_for_test();
        let (_tx, rx) = event_queue();
        let result =
            EngineSource::with_routing_tree_persistent_workers(rx, 44_100, block_frames, None);
        assert!(matches!(
            result,
            Err(SourceWorkerSetupError::InvalidBlockFrames {
                requested,
                min: MIN_BLOCK_FRAMES,
                max: MAX_BLOCK_FRAMES,
            }) if requested == block_frames
        ));
        assert_eq!(reaper_spawn_failure.attempts_for_test(), 0);

        let reaper_spawn_failure = crate::source_worker_reaper::fail_next_reaper_spawn_for_test();
        let (_tx, rx) = event_queue();
        let result = EngineSource::with_routing_tree_persistent_workers_with_hook(
            rx,
            44_100,
            block_frames,
            None,
            successful_start_hook,
        );
        assert!(matches!(
            result,
            Err(SourceWorkerSetupError::InvalidBlockFrames { requested, .. })
                if requested == block_frames
        ));
        assert_eq!(reaper_spawn_failure.attempts_for_test(), 0);
    }
}

#[test]
fn product_routing_constructor_owns_worker_cleanup_when_reaper_setup_fails() {
    let (probe_tx, probe_rx) = bounded(1);
    let _probe_guard = install_source_worker_shutdown_probe_for_test(probe_tx);
    let _reaper_spawn_failure = crate::source_worker_reaper::fail_next_reaper_spawn_for_test();
    let (_tx, rx) = event_queue();

    let result = EngineSource::with_routing_tree_persistent_workers(rx, 44_100, 128, None);
    assert!(matches!(
        result,
        Err(SourceWorkerSetupError::RetirementReaperUnavailable)
    ));
    let (shutdown, _) = probe_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("routing worker cleanup report");
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.retirement_error, None);
}

#[test]
fn product_routing_hook_failure_joins_workers_without_returning_a_source() {
    let (probe_tx, probe_rx) = bounded(1);
    let _probe_guard = install_source_worker_shutdown_probe_for_test(probe_tx);
    let (_tx, rx) = event_queue();

    let result = EngineSource::with_routing_tree_persistent_workers_with_hook(
        rx,
        44_100,
        128,
        None,
        fail_parity_one_start_hook,
    );
    assert!(matches!(
        result,
        Err(SourceWorkerSetupError::WorkerSchedulingUnavailable { parity: 1 })
    ));
    let (shutdown, shutdown_thread) = probe_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("routing worker cleanup report");
    assert_eq!(shutdown_thread, thread::current().id());
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.destroyed_owner_count, 0);
}

#[test]
fn benchmark_routing_constructors_remain_compatible() {
    let (_tx, rx) = event_queue();
    let (source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers_for_benchmark(rx, 44_100, 128, None)
            .unwrap();
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);

    let (_tx, rx) = event_queue();
    let (source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers_for_benchmark_with_hook(
            rx,
            44_100,
            128,
            None,
            successful_start_hook,
        )
        .unwrap();
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}

#[test]
fn product_routing_source_retirement_waits_off_the_source_callback() {
    let (_tx, rx) = event_queue();
    let (source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 44_100, 128, None).unwrap();
    let shutdown_thread = thread::spawn(move || shutdown.shutdown().joined_workers);
    thread::sleep(Duration::from_millis(5));
    assert!(!shutdown_thread.is_finished());
    drop(source);
    assert_eq!(shutdown_thread.join().unwrap(), 2);
}

#[test]
fn routing_rejected_topology_state_is_retired_after_source_callback() {
    let (tx, rx) = event_queue();
    let (mut source, retired_rx) =
        EngineSource::with_routing_tree_test_retirement_receiver(rx, 44_100, 128);
    let (drop_tx, drop_rx) = mpsc::channel();
    source.set_retired_drop_probe(drop_tx);
    tx.send(EngineEvent::SetPreparedAudioConfig(
        realtime_engine::synth::prepare_audio_config(
            direct_synth_instruments(),
            None,
            None,
            44_100,
        ),
    ))
    .unwrap();
    for _ in 0..256 {
        let _ = source.next();
    }
    while retired_rx.try_recv().is_ok() {}
    while drop_rx.try_recv().is_ok() {}

    tx.send(EngineEvent::SetPreparedFxBusSlot {
        bus_index: 0,
        slot_index: 0,
        config: realtime_engine::synth::prepare_fx_bus_slot(
            "reverb".into(),
            std::collections::BTreeMap::new(),
            44_100,
        ),
    })
    .unwrap();
    let (allocations, deallocations) = super::allocations_and_deallocations(|| {
        for _ in 0..256 {
            let _ = source.next();
        }
    });
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(
        source.source_worker_health(),
        realtime_engine::synth::SourceWorkerHealth::CompletionFailed
    );
    drop(source);
    assert_ne!(
        drop_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("retired topology state"),
        thread::current().id()
    );
}

fn direct_synth_instruments() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(realtime_engine::synth::InstrumentMixerConfig {
                route: "direct".into(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}
