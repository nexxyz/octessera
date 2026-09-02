use super::{
    build_persistent_source, expected_worker_thread_names, fill_callback_body, post_dsp_zero,
    stream_geometry, SourceWorkerTimingProbe, EXECUTOR_MODE,
};
use realtime_engine::synth::SourceWorkerHealth;
use rodio_engine_source::{event_queue, SOURCE_REAPER_THREAD_NAME};
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
