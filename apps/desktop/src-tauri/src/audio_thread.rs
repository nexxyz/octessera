use crate::contained_audio_source::panic_message;
use crate::recording::{RecordingEngineSource, RecordingTapState};
use crate::types::AudioRuntime;
use playback_runtime::{
    RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation,
    RuntimePresentationMetrics,
};
use realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;
use rodio::Source;
use rodio_engine_source::{
    AudioLoadStatusReceiver, AudioLoadStatusSender, EngineEventReceiver, EngineSource,
};
use serde::Serialize;
use std::thread;
use std::time::{Duration, Instant};
use tauri::Emitter;

#[derive(Clone, Serialize)]
struct AudioLoadPayload {
    ratio: f32,
    #[serde(rename = "workerUtilization")]
    worker_utilization: Option<f32>,
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
        eprintln!("audio disabled (--no-audio)");
        thread::spawn(move || render_without_output(control_rx, load_tx, recording_tap));
        return;
    }

    thread::spawn(move || {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
            let mut audio = AudioRuntime::new(recording_tap)?;
            audio.start_engine(control_rx, load_tx, failure_tx.clone())?;
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
                let msg = panic_message(panic.as_ref());
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

/// Pulls the engine at its real-time pace and drops the audio, so the runtime, recordings and
/// load reports behave as with a sound card.
fn render_without_output(
    control_rx: EngineEventReceiver,
    load_tx: AudioLoadStatusSender,
    recording_tap: RecordingTapState,
) {
    let engine =
        EngineSource::with_load_status_tx(control_rx, DEFAULT_AUDIO_SAMPLE_RATE, Some(load_tx));
    let block_frames = engine.block_frames();
    let block_samples = block_frames * usize::from(engine.channels());
    let block_time =
        Duration::from_secs_f64(block_frames as f64 / DEFAULT_AUDIO_SAMPLE_RATE as f64);
    let mut source = RecordingEngineSource::new(engine, recording_tap);
    let mut next_block_at = Instant::now();
    loop {
        if source.by_ref().take(block_samples).count() < block_samples {
            return;
        }
        next_block_at += block_time;
        thread::sleep(next_block_at.saturating_duration_since(Instant::now()));
    }
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
                    worker_utilization: status.worker_utilization,
                    missed_quantum_flash: status.missed_quantum_flash,
                    output_clipped: status.output_clipped,
                },
            ));
            let _ = app_handle.emit(
                "audio_load",
                AudioLoadPayload {
                    ratio: status.ratio,
                    worker_utilization: status.worker_utilization,
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

#[cfg(test)]
mod tests;
