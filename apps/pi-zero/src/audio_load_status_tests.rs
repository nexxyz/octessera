use super::audio_output_open::OpenedAudioSink;
use super::{AudioManager, AudioOpenPolicy, AudioSink};
use crate::audio_route::new_registry;
use crate::audio_sink_registry::new_attach_gate;
use crate::audio_stream_health::AudioStreamHealth;
use crate::orange_host_adapter::OrangeHostAdapter;
use playback_runtime::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage, RuntimeConfig,
    RuntimePlatformEffect, SyncSource,
};
use realtime_engine::synth::AudioLoadStatus;
use rodio_engine_source::{event_queue, AudioLoadStatusSender, EngineEvent, EngineSource};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn orange_jack_status_reaches_the_runtime_snapshot_and_oled() {
    let outputs = playback_runtime::AudioOutputSet::jack();
    let mut manager = AudioManager::new_with_opener(
        None,
        vec![AudioSink::Jack],
        true,
        AudioOpenPolicy::Outputs(outputs),
        test_opener,
        new_registry(outputs),
        new_attach_gate(),
    )
    .unwrap();
    let (mut playback, mut runner, mut host, root) = runtime_with_snapshot(manager.service());

    let (source_tx, source_rx) = event_queue();
    let (mut source, shutdown) = EngineSource::with_persistent_workers(
        source_rx,
        48_000,
        128,
        Some(manager.load_tx.clone()),
    )
    .unwrap();
    source_tx
        .send(EngineEvent::NoteOn {
            instrument_slot: 0,
            note: 60,
            velocity: 100,
            duration_ms: 1_000,
        })
        .unwrap();
    let status_deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < status_deadline
        && !playback
            .last_snapshot()
            .and_then(|snapshot| snapshot["workerUtilization"].as_f64())
            .is_some_and(|value| value.is_finite())
    {
        for _ in 0..256 {
            source.next();
        }
        let output = manager.drain_audio_load_status(&mut playback);
        crate::orange_candidate::process_runtime_output(
            &mut playback,
            &mut runner,
            &mut host,
            output,
        )
        .unwrap();
    }
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
    let output = manager.drain_audio_load_status(&mut playback);
    crate::orange_candidate::process_runtime_output(&mut playback, &mut runner, &mut host, output)
        .unwrap();

    let snapshot = playback.last_snapshot().expect("runtime snapshot");
    assert!(snapshot["workerUtilization"].as_f64().unwrap().is_finite());
    assert_eq!(snapshot["highCpuSteady"], false);
    assert!(playback.last_oled_frame().is_some());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn orange_status_drain_keeps_newest_and_absent_evidence_stays_hidden() {
    let outputs = playback_runtime::AudioOutputSet::jack();
    let mut manager = AudioManager::new_with_opener(
        None,
        vec![AudioSink::Jack],
        true,
        AudioOpenPolicy::Outputs(outputs),
        test_opener,
        new_registry(outputs),
        new_attach_gate(),
    )
    .unwrap();
    let (mut playback, mut runner, mut host, root) = runtime_with_snapshot(manager.service());

    manager.load_tx.try_send(status(Some(0.8), false, true));
    manager.load_tx.try_send(status(Some(0.9), true, true));
    let output = manager.drain_audio_load_status(&mut playback);
    crate::orange_candidate::process_runtime_output(&mut playback, &mut runner, &mut host, output)
        .unwrap();
    assert!(
        (playback.last_snapshot().unwrap()["workerUtilization"]
            .as_f64()
            .unwrap()
            - 0.9)
            .abs()
            < 1e-6
    );
    assert_eq!(playback.last_snapshot().unwrap()["highCpuSteady"], true);
    assert_eq!(
        playback.last_snapshot().unwrap()["missedQuantumFlash"],
        true
    );

    manager.load_tx.try_send(status(None, true, false));
    let output = manager.drain_audio_load_status(&mut playback);
    crate::orange_candidate::process_runtime_output(&mut playback, &mut runner, &mut host, output)
        .unwrap();
    let snapshot = playback.last_snapshot().unwrap();
    assert!(!snapshot
        .as_object()
        .unwrap()
        .contains_key("workerUtilization"));
    assert_eq!(snapshot["highCpuSteady"], false);
    assert_eq!(snapshot["missedQuantumFlash"], false);
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

fn runtime_with_snapshot(
    audio: super::super::AudioService,
) -> (PlaybackRuntime, NativeRunner, OrangeHostAdapter, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "octessera-orange-audio-status-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    let mut host = OrangeHostAdapter::with_directories(
        audio,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();
    let output = playback
        .dispatch_runner_messages(
            vec![RunnerMessage::PlatformEffects {
                effects: vec![
                    RuntimePlatformEffect::StoreLoadDefault,
                    RuntimePlatformEffect::MidiListOutputsRequest,
                    RuntimePlatformEffect::MidiListInputsRequest,
                ],
            }],
            &mut runner,
            &mut host,
        )
        .unwrap();
    crate::orange_candidate::process_runtime_output(&mut playback, &mut runner, &mut host, output)
        .unwrap();
    crate::orange_candidate::dispatch(
        &mut playback,
        &mut runner,
        &mut host,
        HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        },
    )
    .unwrap();
    (playback, runner, host, root)
}

fn test_opener(
    _output_buffer_frames: Option<u32>,
    sink: AudioSink,
    _recording_tap: Option<super::RecordingTapState>,
    _load_tx: Option<AudioLoadStatusSender>,
) -> Result<OpenedAudioSink, crate::audio_route::RouteOpenError> {
    let (engine_tx, engine_rx) = event_queue();
    Ok(OpenedAudioSink {
        engine_tx,
        _stream: None,
        health: AudioStreamHealth::new(format!("{sink:?}")),
        _test_engine_rx: Some(Arc::new(std::sync::Mutex::new(engine_rx))),
    })
}
