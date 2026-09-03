use super::{
    build_persistent_source, build_source, callback_priority_for_executor,
    callback_scheduler_for_executor, expected_worker_thread_names, fill_callback_body,
    post_dsp_zero, stream_geometry, worker_thread_names_for_executor, BenchmarkExecutorMode,
    SourceWorkerTimingProbe, EXECUTOR_MODE,
};
use crate::orange_audio_benchmark::cli::parse;
use crate::orange_audio_benchmark::metrics::CallbackMetrics;
use crate::orange_audio_benchmark::phase::MeasurementControl;
use crate::orange_audio_benchmark::probe::ProfileProbe;
use realtime_engine::synth::SourceWorkerHealth;
use rodio_engine_source::{event_queue, EngineEvent, EngineSource, SOURCE_REAPER_THREAD_NAME};
use std::sync::Arc;

#[test]
fn stream_geometry_keeps_output_buffer_and_internal_block_distinct() {
    for (output_frames, internal_frames) in [
        (128, 32),
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
    assert!(stream_geometry(128, 64).is_err());
    assert!(stream_geometry(64, 32).is_err());
}

#[test]
fn benchmark_source_is_persistent_and_reports_worker_health() {
    let (_engine_tx, engine_rx) = event_queue();
    let (source, shutdown_owner) = build_persistent_source(
        engine_rx,
        44_100,
        128,
        Some(Arc::new(SourceWorkerTimingProbe::new(None))),
    )
    .unwrap();

    assert_eq!(EXECUTOR_MODE, "persistent_two_workers");
    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    assert_eq!(source.block_frames(), 128);
    assert_eq!(source.source_worker_health().name(), "healthy");
    assert_eq!(
        expected_worker_thread_names(),
        ["oct-dsp-src-0", "oct-dsp-src-1"]
    );
    assert_eq!(SOURCE_REAPER_THREAD_NAME, "oct-src-reaper");
    assert!(SOURCE_REAPER_THREAD_NAME.len() <= 15);

    drop(source);
    let shutdown = shutdown_owner.shutdown();
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.retirement_error, None);
}

#[test]
fn disabled_benchmark_source_is_persistent_without_timing_probe() {
    let (_engine_tx, engine_rx) = event_queue();
    let (source, shutdown_owner) = build_persistent_source(engine_rx, 44_100, 128, None).unwrap();

    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    assert_eq!(source.block_frames(), 128);
    drop(source);
    assert_eq!(shutdown_owner.shutdown().joined_workers, 2);
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
        BenchmarkExecutorMode::PersistentTwoWorkers,
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
    assert_eq!(
        callback_priority_for_executor(BenchmarkExecutorMode::Inline),
        70
    );
    assert_eq!(
        callback_priority_for_executor(BenchmarkExecutorMode::PersistentTwoWorkers),
        70
    );
}

#[test]
fn persistent_benchmark_uses_strict_jack_scheduler_and_inline_stays_legacy() {
    let inline = callback_scheduler_for_executor(BenchmarkExecutorMode::Inline);
    assert!(!inline.is_strict());
    assert_eq!(inline.requested_priority(), 70);

    let persistent = callback_scheduler_for_executor(BenchmarkExecutorMode::PersistentTwoWorkers);
    assert!(persistent.is_strict());
    assert_eq!(persistent.requested_priority(), 70);
}

#[test]
fn benchmark_source_uses_requested_frames() {
    for block_frames in [64, 128, 256, 512, 1024, 2048] {
        let (_engine_tx, engine_rx) = event_queue();
        let (source, shutdown_owner) = build_persistent_source(
            engine_rx,
            44_100,
            block_frames,
            Some(Arc::new(SourceWorkerTimingProbe::new(None))),
        )
        .unwrap();
        assert_eq!(source.block_frames(), block_frames);
        drop(source);
        assert_eq!(shutdown_owner.shutdown().joined_workers, 2);
    }
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
