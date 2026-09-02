use super::audio_output_open::source_execution_mode;
use super::cpal_audio_output::{build_engine_source, AudioSourceExecutionMode};
use super::AudioSink;
use realtime_engine::synth::SourceWorkerHealth;
use rodio_engine_source::event_queue;

#[test]
fn persistent_cpal_mode_starts_two_workers_before_stream_construction() {
    let (_engine_tx, engine_rx) = event_queue();
    let (source, shutdown_owner) = build_engine_source(
        engine_rx,
        48_000,
        AudioSourceExecutionMode::PersistentTwoWorkers,
    )
    .unwrap();

    assert_eq!(source.source_worker_health(), SourceWorkerHealth::Healthy);
    drop(source);
    let shutdown = shutdown_owner
        .expect("persistent mode must return a shutdown owner")
        .shutdown();
    assert_eq!(shutdown.joined_workers, 2);
    assert_eq!(shutdown.retirement_error, None);
}

#[test]
fn cpal_uses_persistent_workers_only_for_orange_jack() {
    let expected_jack_mode = if cfg!(feature = "hardware-orange-pi-zero-2w") {
        AudioSourceExecutionMode::PersistentTwoWorkers
    } else {
        AudioSourceExecutionMode::Inline
    };
    assert_eq!(source_execution_mode(AudioSink::Jack), expected_jack_mode);
    for sink in [AudioSink::Usb, AudioSink::Hdmi] {
        assert_eq!(
            source_execution_mode(sink),
            AudioSourceExecutionMode::Inline
        );
    }
}

#[test]
fn cpal_worker_status_reports_only_terminal_worker_failures() {
    for health in [SourceWorkerHealth::Disabled, SourceWorkerHealth::Healthy] {
        assert!(!health.is_terminal());
    }
    for health in [
        SourceWorkerHealth::DeadlineMiss,
        SourceWorkerHealth::DispatchFailed,
        SourceWorkerHealth::CompletionFailed,
        SourceWorkerHealth::WorkerExited,
        SourceWorkerHealth::InvalidBlock,
    ] {
        assert!(health.is_terminal());
    }
}
