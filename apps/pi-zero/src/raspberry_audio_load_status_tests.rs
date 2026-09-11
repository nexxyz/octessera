use super::audio_output_open::{load_status_sender_for_sink, AudioConstructionConfig};
use super::audio_profile::RaspberryAudioProfile;
use super::AudioSink;
use crate::host_adapter::PiPlaybackHostAdapter;
use playback_runtime::{
    AudioOptimization, AudioOutputSet, NativeRunner, NativeRunnerConfig, PlaybackRuntime,
    RuntimeConfig, SyncSource,
};
use realtime_engine::synth::AudioLoadStatus;
use std::sync::Arc;

#[test]
fn raspberry_load_status_sender_is_capacity_jack_only() {
    let (load_tx, _load_rx) = rodio_engine_source::audio_load_status_channel();
    let capacity = AudioConstructionConfig::raspberry(RaspberryAudioProfile::from_optimization(
        AudioOptimization::Capacity,
    ));
    let latency = AudioConstructionConfig::raspberry(RaspberryAudioProfile::from_optimization(
        AudioOptimization::Latency,
    ));

    assert!(load_status_sender_for_sink(capacity, AudioSink::Jack, &load_tx).is_some());
    assert!(load_status_sender_for_sink(capacity, AudioSink::Usb, &load_tx).is_none());
    assert!(load_status_sender_for_sink(latency, AudioSink::Jack, &load_tx).is_none());
}

#[test]
fn raspberry_capacity_load_status_presentation_drains_status() {
    let outputs = AudioOutputSet::jack();
    let root = std::env::temp_dir().join(format!(
        "octessera-raspberry-audio-status-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let mut host = PiPlaybackHostAdapter::new(
        None,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        outputs,
    );
    crate::runtime_loop::initialize_host_state(&mut playback, &mut runner, &mut host).unwrap();
    crate::runtime_loop::dispatch_runtime_message(
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

    let (load_tx, load_rx) = rodio_engine_source::audio_load_status_channel();
    assert!(load_tx.try_send(status(Some(0.9), true, true)));
    let output = crate::audio::drain_audio_load_status(&load_rx, &mut playback, false);
    crate::runtime_loop::process_runtime_output(&mut playback, &mut runner, &mut host, output)
        .unwrap();

    let snapshot = playback.last_snapshot().expect("runtime snapshot");
    assert_eq!(snapshot["workerUtilization"], 0.9);
    assert_eq!(snapshot["highCpuSteady"], true);
    assert_eq!(snapshot["missedQuantumFlash"], true);
    let _ = std::fs::remove_dir_all(root);
}

fn status(
    worker_utilization: Option<f32>,
    high_cpu_steady: bool,
    missed_quantum_flash: bool,
) -> AudioLoadStatus {
    AudioLoadStatus {
        ratio: 0.2,
        voice_steal: false,
        worker_utilization,
        high_cpu_steady,
        missed_quantum_flash,
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
    }
}
