use super::probe::ProfileProbe;
use super::*;
use crate::audio::AudioStreamHealth;
use realtime_engine::synth::{
    default_synth_config, prepare_instruments_config, InstrumentSlotConfig, InstrumentsConfig,
    DEFAULT_PAN_POSITIONS,
};
use rodio_engine_source::{
    event_queue, EngineEvent, EngineSource, EngineSourceWorkerShutdownOwner,
};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

const BLOCK_FRAMES: usize = 128;
const SAMPLE_RATE: u32 = 44_100;

#[test]
fn fixture_profile_barriers_match_executor_lookahead_and_publish_next_profile() {
    let modes = [
        BenchmarkExecutorMode::Inline,
        BenchmarkExecutorMode::PersistentTwoWorkers,
    ];
    for executor_mode in modes {
        assert_fixture_profile_barriers(executor_mode, 1);
    }
    #[cfg(feature = "routing-tree-benchmark")]
    assert_fixture_profile_barriers(BenchmarkExecutorMode::RoutingTreePersistent, 2);
}

fn assert_fixture_profile_barriers(executor_mode: BenchmarkExecutorMode, expected_barriers: usize) {
    let (sender, receiver) = event_queue();
    sender
        .send(EngineEvent::SetPreparedInstruments(
            prepare_instruments_config(
                InstrumentsConfig {
                    instruments: vec![InstrumentSlotConfig {
                        kind: "synth".into(),
                        synth: default_synth_config(),
                        mixer: None,
                    }],
                    mixer: None,
                    pan_positions: DEFAULT_PAN_POSITIONS,
                    master_volume: 100.0,
                },
                SAMPLE_RATE,
            ),
        ))
        .unwrap();
    let (mut source, shutdown) = source_for_executor(executor_mode, receiver);
    let health = AudioStreamHealth::new("fixture barrier test".into());
    let profile_probe = Arc::new(ProfileProbe::new());
    let (published_tx, published_rx) = mpsc::sync_channel(1);
    let (run_block_tx, run_block_rx) = mpsc::sync_channel(0);
    let (block_done_tx, block_done_rx) = mpsc::sync_channel(0);
    let driver_probe = Arc::clone(&profile_probe);
    let driver = thread::spawn(move || {
        let mut refill_generation = 0;
        while run_block_rx.recv().is_ok() {
            refill_generation += 1;
            for _ in 0..(BLOCK_FRAMES * 2) {
                source.next();
            }
            if driver_probe.request_pending() {
                driver_probe.publish(source.profile_snapshot());
                let _ = published_tx.send(());
            }
            let _ = block_done_tx.send(refill_generation);
        }
        drop(source);
    });

    let mut initial_generation = 0;
    for _ in 0..expected_barriers {
        run_block_tx.send(()).unwrap();
        initial_generation = block_done_rx.recv().unwrap();
    }
    sender
        .send(EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 100,
            duration_ms: 10_000,
        })
        .unwrap();
    let mut barrier_generations = Vec::new();
    wait_for_fixture_profile_barriers(executor_mode, || {
        let report_rx = send_probe(&sender)?;
        let deadline = Instant::now() + PROFILE_TIMEOUT;
        loop {
            if health.runtime_status() == crate::audio::AudioStreamStatus::Terminal {
                health.log_worker_terminal_once();
                return Err("benchmark DSP worker entered a terminal health state".into());
            }
            if Instant::now() >= deadline {
                return Err("fixture probe barrier failed: timed out".into());
            }
            run_block_tx
                .send(())
                .map_err(|error| format!("fixture block driver stopped: {error}"))?;
            let generation = block_done_rx
                .recv()
                .map_err(|error| format!("fixture block driver stopped: {error}"))?;
            if report_rx.try_recv().is_ok() {
                barrier_generations.push(generation);
                break;
            }
        }
        Ok(())
    })
    .unwrap_or_else(|error| panic!("{executor_mode:?}: {error}"));
    assert_eq!(barrier_generations.len(), expected_barriers);
    assert!(barrier_generations[0] > initial_generation);
    assert!(barrier_generations
        .windows(2)
        .all(|generations| generations[1] > generations[0]));

    let generation = profile_probe.request();
    run_block_tx.send(()).unwrap();
    published_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("fixture profile was not published");
    block_done_rx.recv().expect("fixture block driver stopped");
    let profile = profile_probe.poll(generation).expect("fixture profile");
    assert_eq!(profile.active_synth_voices, 1, "{executor_mode:?}");
    assert_eq!(profile.active_sample_voices, 0);
    assert_eq!(profile.active_momentary_fx, 0);
    assert_eq!(profile.active_bus_fx_slots, 0);
    assert_eq!(profile.active_global_fx_slots, 0);

    drop(run_block_tx);
    driver.join().unwrap();
    if let Some(shutdown) = shutdown {
        assert_eq!(shutdown.shutdown().joined_workers, 2);
    }
}

fn source_for_executor(
    executor_mode: BenchmarkExecutorMode,
    receiver: rodio_engine_source::EngineEventReceiver,
) -> (EngineSource, Option<EngineSourceWorkerShutdownOwner>) {
    match executor_mode {
        BenchmarkExecutorMode::Inline => (
            EngineSource::with_block_frames(receiver, SAMPLE_RATE, BLOCK_FRAMES),
            None,
        ),
        BenchmarkExecutorMode::PersistentTwoWorkers => {
            let (source, shutdown) = EngineSource::with_persistent_workers_for_benchmark(
                receiver,
                SAMPLE_RATE,
                BLOCK_FRAMES,
                None,
            )
            .expect("persistent source");
            (source, Some(shutdown))
        }
        #[cfg(feature = "routing-tree-benchmark")]
        BenchmarkExecutorMode::RoutingTreePersistent => {
            let (source, shutdown) =
                EngineSource::with_routing_tree_persistent_workers_for_benchmark(
                    receiver,
                    SAMPLE_RATE,
                    BLOCK_FRAMES,
                    None,
                )
                .expect("routing-tree source");
            (source, Some(shutdown))
        }
        #[cfg(not(feature = "routing-tree-benchmark"))]
        BenchmarkExecutorMode::RoutingTreePersistent => {
            unreachable!("routing-tree source requires its benchmark feature")
        }
    }
}
