use crate::contained_audio_source::ContainedAudioSource;
use crate::late_timed_events::late_timed_events_counter;
use crate::recording::{RecordingEngineSource, RecordingTapState};
use playback_runtime::{RunnerMessage, RuntimeAdapterError};
use realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;
use rodio::{OutputStream, Sink, Source};
use rodio_engine_source::{AudioLoadStatusSender, EngineEventReceiver, EngineSource};
use serde_json::Value;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

/// The engine every desktop output pulls; it outlives an output that is lost and reopened.
pub(crate) struct SharedEngine {
    pub(crate) source: Arc<Mutex<RecordingEngineSource>>,
    pub(crate) block_samples: usize,
    /// Blocks pulled by the current output.
    pub(crate) pulls: Arc<AtomicU64>,
}

impl SharedEngine {
    pub(crate) fn new(
        control_rx: EngineEventReceiver,
        load_tx: AudioLoadStatusSender,
        recording_tap: RecordingTapState,
    ) -> Self {
        let mut engine =
            EngineSource::with_load_status_tx(control_rx, DEFAULT_AUDIO_SAMPLE_RATE, Some(load_tx));
        engine.set_late_timed_events_counter(late_timed_events_counter());
        let block_samples = engine.block_frames() * usize::from(engine.channels());
        Self {
            source: Arc::new(Mutex::new(RecordingEngineSource::new(
                engine,
                recording_tap,
            ))),
            block_samples,
            pulls: Arc::new(AtomicU64::new(0)),
        }
    }
}

/// The default output device playing the shared engine.
pub(crate) struct AudioRuntime {
    _stream: OutputStream,
    sink: Sink,
}

impl AudioRuntime {
    pub(crate) fn open(
        engine: &SharedEngine,
        failure_tx: Sender<RuntimeAdapterError>,
    ) -> Result<Self, String> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| format!("audio init failed: {e}"))?;
        let sink = Sink::try_new(&handle).map_err(|e| format!("sink create failed: {e}"))?;
        sink.append(ContainedAudioSource::new(
            engine.source.clone(),
            engine.block_samples,
            engine.pulls.clone(),
            failure_tx,
        ));
        sink.play();
        Ok(Self {
            _stream: stream,
            sink,
        })
    }
}

impl Drop for AudioRuntime {
    fn drop(&mut self) {
        self.sink.stop();
    }
}

pub(crate) const RUNTIME_MESSAGES_EVENT: &str = "runtime_messages";
pub(crate) const RUNTIME_UI_REFRESH_MS: u64 = 100;

#[derive(Clone, serde::Serialize)]
pub(crate) struct RuntimeMessagesPayload {
    pub(crate) seq: u64,
    pub(crate) messages: Vec<Value>,
}

pub(crate) fn encode_runtime_responses(
    responses: Vec<RunnerMessage>,
) -> Result<Vec<Value>, String> {
    responses
        .into_iter()
        .map(|r| {
            serde_json::to_value(r).map_err(|e| format!("failed to encode runtime response: {e}"))
        })
        .collect()
}
