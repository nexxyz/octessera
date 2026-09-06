use super::callback::{fill_callback_body, post_dsp_zero};
#[cfg(feature = "routing-tree-benchmark")]
use super::expected_routing_worker_thread_names;
#[cfg(feature = "routing-tree-benchmark")]
use super::SourceWorkerTimingProbe;
use super::{
    build_source, callback_scheduler_for_executor, stream_geometry,
    worker_thread_names_for_executor, BenchmarkExecutorMode,
};
use crate::orange_audio_benchmark::cli::parse;
#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
))]
use crate::orange_audio_benchmark::cli::{validate_recorded_geometry, RecordedGeometry};
use crate::orange_audio_benchmark::metrics::CallbackMetrics;
use crate::orange_audio_benchmark::phase::MeasurementControl;
use crate::orange_audio_benchmark::probe::ProfileProbe;
use realtime_engine::synth::SourceWorkerHealth;
use rodio_engine_source::{event_queue, EngineEvent, EngineSource};
use std::sync::Arc;

#[test]
fn stream_geometry_keeps_output_buffer_and_internal_block_distinct() {
    for (output_frames, internal_frames) in [
        (128, 32),
        (128, 64),
        (256, 64),
        (256, 128),
        (256, 256),
        (512, 128),
        (1024, 256),
    ] {
        let geometry = stream_geometry(output_frames, internal_frames).unwrap();
        assert_eq!(geometry.output_frames, output_frames);
        assert_eq!(geometry.internal_frames, internal_frames);
    }
    assert!(stream_geometry(512, 256).is_err());
    assert!(stream_geometry(64, 32).is_err());
}

#[test]
fn stream_preflight_rejects_non_analogue_128_64_before_device_access() {
    let mut config = parse(
        [
            "--benchmark-orange-audio",
            "--scenario",
            "synth_ramp_16",
            "--output-frames",
            "256",
            "--engine-block-frames",
            "64",
            "--release-gate",
            "release.json",
            "--artifact-sha256",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        ]
        .into_iter()
        .map(str::to_owned),
    )
    .unwrap();
    config.output_frames = 128;
    config.expected_alsa_period_frames = 32;
    config.internal_frames = 64;
    config.worker_timing_mode = super::super::cli::WorkerTimingMode::Disabled;
    #[cfg(feature = "routing-tree-benchmark")]
    let executor_modes = vec![
        BenchmarkExecutorMode::Inline,
        BenchmarkExecutorMode::RoutingTreePersistent,
    ];
    #[cfg(not(feature = "routing-tree-benchmark"))]
    let executor_modes = vec![BenchmarkExecutorMode::Inline];
    for executor_mode in executor_modes {
        config.executor_mode = executor_mode;
        let (_sender, receiver) = event_queue();
        let error = match super::build(
            receiver,
            &config,
            Arc::new(CallbackMetrics::new(44_100, 32, 128)),
            Arc::new(ProfileProbe::new()),
            Arc::new(MeasurementControl::new()),
            None,
        ) {
            Err(error) => error,
            Ok(_) => panic!("invalid Orange benchmark geometry unexpectedly built"),
        };
        assert_eq!(
            error,
            "unsupported Orange benchmark geometry tuple: output=128 internal=64"
        );
    }
}

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
))]
#[test]
fn inline_analogue_recorded_geometry_requires_exact_latency_evidence() {
    let valid = (32, 64, 0, Some(128));
    assert!(validate_recorded_geometry(RecordedGeometry {
        scenario: "capacity_analogue_1",
        executor_mode: BenchmarkExecutorMode::Inline,
        requested_output_buffer_frames: 128,
        expected_alsa_buffer_frames: 128,
        expected_alsa_period_frames: valid.0,
        internal_block_frames: valid.1,
        lookahead_frames: valid.2,
        effective_output_latency_frames: valid.3,
    })
    .is_ok());
    for (period, lookahead, effective) in
        [(64, 0, Some(128)), (32, 64, Some(192)), (32, 0, Some(192))]
    {
        assert!(
            validate_recorded_geometry(RecordedGeometry {
                scenario: "capacity_analogue_1",
                executor_mode: BenchmarkExecutorMode::Inline,
                requested_output_buffer_frames: 128,
                expected_alsa_buffer_frames: 128,
                expected_alsa_period_frames: period,
                internal_block_frames: 64,
                lookahead_frames: lookahead,
                effective_output_latency_frames: effective,
            })
            .is_err(),
            "mismatched recorded geometry should fail: period={period} lookahead={lookahead} effective={effective:?}"
        );
    }
}

#[cfg(not(feature = "routing-tree-benchmark"))]
#[test]
fn disabled_routing_tree_source_fails_without_starting_workers() {
    let (_engine_tx, engine_rx) = event_queue();
    let result = build_source(
        engine_rx,
        BenchmarkExecutorMode::RoutingTreePersistent,
        44_100,
        128,
        None,
    );
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("routing-tree source unexpectedly built without its feature"),
    };
    assert_eq!(
        error,
        "routing_tree_persistent executor requires a binary built with routing-tree-benchmark"
    );
}

#[cfg(feature = "routing-tree-benchmark")]
#[test]
fn routing_tree_benchmark_source_reports_actual_lookahead_and_names() {
    let (_engine_tx, engine_rx) = event_queue();
    let (source, shutdown_owner) = build_source(
        engine_rx,
        BenchmarkExecutorMode::RoutingTreePersistent,
        44_100,
        128,
        Some(Arc::new(SourceWorkerTimingProbe::new(None))),
    )
    .unwrap();
    assert_eq!(source.lookahead_frames(), 128);
    assert_eq!(
        worker_thread_names_for_executor(BenchmarkExecutorMode::RoutingTreePersistent),
        expected_routing_worker_thread_names()
    );
    drop(source);
    assert_eq!(shutdown_owner.unwrap().shutdown().joined_workers, 2);
}

#[cfg(feature = "routing-tree-benchmark")]
#[test]
fn routing_tree_stream_rejects_large_output_before_device_access() {
    let mut config = parse(
        [
            "--benchmark-orange-audio",
            "--scenario",
            "synth_ramp_16",
            "--output-frames",
            "256",
            "--engine-block-frames",
            "128",
            "--executor",
            "routing_tree_persistent",
            "--release-gate",
            "release.json",
            "--artifact-sha256",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        ]
        .into_iter()
        .map(str::to_owned),
    )
    .unwrap();
    config.output_frames = 512;
    let (_sender, receiver) = event_queue();
    let result = super::build(
        receiver,
        &config,
        Arc::new(CallbackMetrics::new(44_100, 128, 512)),
        Arc::new(ProfileProbe::new()),
        Arc::new(MeasurementControl::new()),
        None,
    );
    match result {
        Err(error) => assert_eq!(
            error,
            "routing_tree_persistent executor requires output frames <= 256"
        ),
        Ok(_) => panic!("routing-tree stream unexpectedly accepted a large output buffer"),
    }
}

#[test]
fn inline_benchmark_source_matches_serial_oracle_without_workers() {
    let (inline_tx, inline_rx) = event_queue();
    let (mut inline, shutdown_owner) =
        build_source(inline_rx, BenchmarkExecutorMode::Inline, 44_100, 128, None).unwrap();
    let (oracle_tx, oracle_rx) = event_queue();
    let mut oracle = EngineSource::with_block_frames(oracle_rx, 44_100, 128);
    let note_on = EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 1_000,
    };
    let note_off = EngineEvent::NoteOff {
        instrument_slot: 0,
        note: 60,
    };
    inline_tx.send(note_on.clone()).unwrap();
    oracle_tx.send(note_on).unwrap();
    let mut inline_bits: Vec<_> = (0..64).map(|_| inline.next().unwrap().to_bits()).collect();
    let mut oracle_bits: Vec<_> = (0..64).map(|_| oracle.next().unwrap().to_bits()).collect();
    inline_tx.send(note_off.clone()).unwrap();
    oracle_tx.send(note_off).unwrap();
    inline_bits.extend((0..192).map(|_| inline.next().unwrap().to_bits()));
    oracle_bits.extend((0..192).map(|_| oracle.next().unwrap().to_bits()));

    assert_eq!(inline_bits, oracle_bits);
    assert_eq!(inline.profile_snapshot(), oracle.profile_snapshot());
    assert!(inline_bits.iter().any(|sample| *sample != 0));
    assert_eq!(inline.source_worker_health(), SourceWorkerHealth::Disabled);
    assert!(shutdown_owner.is_none());
    assert_eq!(
        worker_thread_names_for_executor(BenchmarkExecutorMode::Inline),
        [String::new(), String::new()]
    );
    drop(inline);
    drop(oracle);
}

#[test]
fn invalid_stream_geometry_fails_build_for_both_executors() {
    for executor_mode in [
        BenchmarkExecutorMode::Inline,
        BenchmarkExecutorMode::RoutingTreePersistent,
    ] {
        let mut config = parse(
            [
                "--benchmark-orange-audio",
                "--scenario",
                "synth_ramp_16",
                "--output-frames",
                "256",
                "--engine-block-frames",
                "64",
                "--release-gate",
                "release.json",
                "--artifact-sha256",
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .unwrap();
        config.executor_mode = executor_mode;
        config.internal_frames = 32;
        let (_sender, receiver) = event_queue();
        let result = super::build(
            receiver,
            &config,
            Arc::new(CallbackMetrics::new(
                44_100,
                config.expected_alsa_period_frames,
                config.output_frames,
            )),
            Arc::new(ProfileProbe::new()),
            Arc::new(MeasurementControl::new()),
            None,
        );
        assert!(result.is_err());
    }
}

#[test]
fn benchmark_executor_priorities_preserve_orange_scheduling_policy() {
    assert_eq!(crate::audio_priority::ORANGE_WORKER_PRIORITY, 70);
    assert_eq!(crate::audio_priority::PI_JACK_CALLBACK_PRIORITY, 70);
}

#[test]
fn benchmark_executors_use_strict_jack_scheduler() {
    let inline = callback_scheduler_for_executor(BenchmarkExecutorMode::Inline);
    assert_eq!(inline.requested_cpu(), 1);
    assert_eq!(inline.requested_priority(), 70);

    let routing = callback_scheduler_for_executor(BenchmarkExecutorMode::RoutingTreePersistent);
    assert_eq!(routing.requested_cpu(), 1);
    assert_eq!(routing.requested_priority(), 70);
}

#[test]
fn post_dsp_zero_supports_all_orange_sample_formats() {
    assert_eq!(post_dsp_zero::<f32>(), 0.0);
    assert_eq!(post_dsp_zero::<i16>(), 0);
    assert_eq!(post_dsp_zero::<u16>(), 32_768);
}

#[test]
fn callback_body_consumes_and_mutes_f32_output() {
    let mut data = [1.0_f32; 3];
    let mut source = [0.25, -0.5, 0.0].into_iter();
    let stats = fill_callback_body(&mut data, &mut source);
    assert_eq!(data, [0.0; 3]);
    assert_eq!(source.next(), None);
    assert_eq!(stats.pre_mute_nonzero, 2);
    assert_eq!(stats.post_mute_nonzero, 0);
}

#[test]
fn callback_body_converts_and_mutes_i16_output() {
    let mut data = [1_i16; 3];
    let mut source = [0.25, -0.5, 0.0].into_iter();
    let stats = fill_callback_body(&mut data, &mut source);
    assert_eq!(data, [0; 3]);
    assert_eq!(source.next(), None);
    assert_eq!(stats.pre_mute_nonzero, 2);
}

#[test]
fn callback_body_converts_and_mutes_u16_output() {
    let mut data = [1_u16; 3];
    let mut source = [0.25, -0.5, 0.0].into_iter();
    let stats = fill_callback_body(&mut data, &mut source);
    assert_eq!(data, [32_768; 3]);
    assert_eq!(source.next(), None);
    assert_eq!(stats.pre_mute_nonzero, 2);
}
