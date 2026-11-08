use super::audio_output_open::load_status_sender_for_sink;
use super::direct_cpal_tests::play_clipping_latency_jack_source;
use super::AudioSink;
use crate::host_adapter::PiHostAdapter;
use playback_runtime::{
    AudioOutputSet, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig, SyncSource,
};
use realtime_engine::synth::AudioLoadStatus;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn raspberry_load_status_sender_is_jack_only() {
    let (load_tx, _load_rx) = rodio_engine_source::audio_load_status_channel();

    assert!(load_status_sender_for_sink(AudioSink::Jack, &load_tx).is_some());
    assert!(load_status_sender_for_sink(AudioSink::Usb, &load_tx).is_none());
    assert!(load_status_sender_for_sink(AudioSink::Hdmi, &load_tx).is_none());
}

#[test]
fn raspberry_capacity_load_status_presentation_drains_status() {
    let (mut playback, mut runner, mut host, root) = runtime_with_snapshot();

    let (load_tx, load_rx) = rodio_engine_source::audio_load_status_channel();
    assert!(load_tx.try_send(status(Some(0.9), true)));
    let output = crate::audio::drain_audio_load_status(&load_rx, &mut playback, false);
    crate::runtime_dispatch::process_runtime_output(&mut playback, &mut runner, &mut host, output)
        .unwrap();

    let snapshot = playback.last_snapshot().expect("runtime snapshot");
    assert_eq!(
        snapshot["workerUtilization"].as_f64(),
        Some(f64::from(0.9_f32))
    );
    assert_eq!(snapshot["missedQuantumFlash"], true);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn raspberry_latency_jack_clipping_reaches_the_runtime() {
    let (mut playback, mut runner, mut host, root) = runtime_with_snapshot();

    let (load_tx, load_rx) = rodio_engine_source::audio_load_status_channel();
    play_clipping_latency_jack_source(load_status_sender_for_sink(AudioSink::Jack, &load_tx));
    let output = crate::audio::drain_audio_load_status(&load_rx, &mut playback, false);
    crate::runtime_dispatch::process_runtime_output(&mut playback, &mut runner, &mut host, output)
        .unwrap();

    let (metrics, _) = playback.native_presentation_state();
    assert!(metrics.output_clipping, "CLIP did not reach the runtime");
    assert_eq!(metrics.worker_utilization, None);
    assert!(!metrics.missed_quantum_flash);
    let _ = std::fs::remove_dir_all(root);
}

fn runtime_with_snapshot() -> (PlaybackRuntime, NativeRunner, PiHostAdapter, PathBuf) {
    let outputs = AudioOutputSet::jack();
    let root = crate::test_temp_dir::unique_temp_path("octessera-raspberry-audio-status");
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let mut host = PiHostAdapter::new(
        None,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        outputs,
    );
    crate::runtime_output::initialize_host_state(&mut playback, &mut runner, &mut host).unwrap();
    crate::runtime_dispatch::dispatch_runtime_message(
        &mut playback,
        &mut runner,
        &mut host,
        playback_runtime::HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        },
    )
    .unwrap();
    (playback, runner, host, root)
}

fn status(worker_utilization: Option<f32>, missed_quantum_flash: bool) -> AudioLoadStatus {
    AudioLoadStatus {
        ratio: 0.2,
        worker_utilization,
        missed_quantum_flash,
        output_clipped: false,
        block_ratio_p95: 0.2,
        block_ratio_max: 0.2,
        blocks: 1,
        control_events: 0,
        config_events: 0,
        rendered_quantums: 0,
        repeated_quantums: 0,
        dropped_quantums: 0,
        deadline_misses: 0,
        deadline_recoveries: 0,
        peak: realtime_engine::synth::BlockPeak::default(),
    }
}
