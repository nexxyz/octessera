use crate::recording::RecordingTapState;
use crate::types::AudioRuntime;
use playback_runtime::{
    RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimePresentationMetrics,
};
use rodio_engine_source::{AudioLoadStatusReceiver, AudioLoadStatusSender, EngineEventReceiver};
use serde::Serialize;
use std::thread;
use tauri::Emitter;

#[derive(Clone, Serialize)]
struct AudioLoadPayload {
    ratio: f32,
    #[serde(rename = "voiceSteal")]
    voice_steal: bool,
    #[serde(rename = "workerUtilization")]
    worker_utilization: Option<f32>,
    #[serde(rename = "highCpuSteady")]
    high_cpu_steady: bool,
    #[serde(rename = "missedQuantumFlash")]
    missed_quantum_flash: bool,
    #[serde(rename = "blockRatioP95")]
    block_ratio_p95: f32,
    #[serde(rename = "blockRatioMax")]
    block_ratio_max: f32,
    blocks: u64,
    #[serde(rename = "controlEvents")]
    control_events: u64,
    #[serde(rename = "configEvents")]
    config_events: u64,
}

pub(crate) fn spawn_audio_engine_thread(
    control_rx: EngineEventReceiver,
    load_tx: AudioLoadStatusSender,
    failure_tx: std::sync::mpsc::Sender<RuntimeAdapterError>,
    no_audio: bool,
    recording_tap: RecordingTapState,
) {
    if no_audio {
        drop(control_rx);
        eprintln!("audio disabled (--no-audio)");
        return;
    }

    thread::spawn(move || {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
            let mut audio = AudioRuntime::new(recording_tap)?;
            audio.start_engine(control_rx, load_tx)?;
            let (_keepalive, shutdown_rx) = std::sync::mpsc::channel::<()>();
            while shutdown_rx
                .recv_timeout(std::time::Duration::from_secs(60))
                .is_err()
            {}
            Ok(())
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                eprintln!("audio error: {error}");
                let _ = failure_tx.send(RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
                    RuntimeErrorDomain::Audio,
                    RuntimeErrorCode::AudioThreadFailed,
                    RuntimeOperation::AudioThread,
                    Some(error),
                )));
            }
            Err(panic) => {
                let msg = panic
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| panic.downcast_ref::<String>().map(|s| s.as_str()))
                    .unwrap_or("unknown panic");
                eprintln!("audio thread panicked: {msg}");
                let _ = failure_tx.send(RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
                    RuntimeErrorDomain::Audio,
                    RuntimeErrorCode::AudioThreadFailed,
                    RuntimeOperation::AudioThread,
                    Some(format!("panic: {msg}")),
                )));
            }
        }
    });
}

pub(crate) fn spawn_load_listener(
    load_rx: AudioLoadStatusReceiver,
    app_handle: tauri::AppHandle,
    worker_tx: std::sync::mpsc::Sender<crate::runtime_worker::WorkerCommand>,
) {
    thread::spawn(move || {
        while let Ok(status) = load_rx.recv() {
            let _ = worker_tx.send(crate::runtime_worker::WorkerCommand::PresentationMetrics(
                RuntimePresentationMetrics {
                    audio_load_ratio: status.ratio,
                    voice_steal: status.voice_steal,
                    worker_utilization: status.worker_utilization,
                    high_cpu_steady: status.high_cpu_steady,
                    missed_quantum_flash: status.missed_quantum_flash,
                },
            ));
            let _ = app_handle.emit(
                "audio_load",
                AudioLoadPayload {
                    ratio: status.ratio,
                    voice_steal: status.voice_steal,
                    worker_utilization: status.worker_utilization,
                    high_cpu_steady: status.high_cpu_steady,
                    missed_quantum_flash: status.missed_quantum_flash,
                    block_ratio_p95: status.block_ratio_p95,
                    block_ratio_max: status.block_ratio_max,
                    blocks: status.blocks,
                    control_events: status.control_events,
                    config_events: status.config_events,
                },
            );
        }
    });
}
