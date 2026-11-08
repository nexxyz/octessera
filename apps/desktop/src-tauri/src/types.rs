use crate::contained_audio_source::ContainedAudioSource;
use crate::late_timed_events::late_timed_events_counter;
use crate::recording::{RecordingEngineSource, RecordingTapState};
use playback_runtime::{RunnerMessage, RuntimeAdapterError};
use realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use rodio_engine_source::{AudioLoadStatusSender, EngineEventReceiver, EngineSource};
use serde_json::Value;
use std::sync::mpsc::Sender;

pub(crate) struct AudioRuntime {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Option<Sink>,
    recording_tap: RecordingTapState,
}

impl AudioRuntime {
    pub(crate) fn new(recording_tap: RecordingTapState) -> Result<Self, String> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| format!("audio init failed: {e}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            sink: None,
            recording_tap,
        })
    }

    pub(crate) fn start_engine(
        &mut self,
        control_rx: EngineEventReceiver,
        load_tx: AudioLoadStatusSender,
        failure_tx: Sender<RuntimeAdapterError>,
    ) -> Result<(), String> {
        self.stop();
        let mut engine =
            EngineSource::with_load_status_tx(control_rx, DEFAULT_AUDIO_SAMPLE_RATE, Some(load_tx));
        engine.set_late_timed_events_counter(late_timed_events_counter());
        let block_samples = engine.block_frames() * usize::from(engine.channels());
        let source = ContainedAudioSource::new(
            RecordingEngineSource::new(engine, self.recording_tap.clone()),
            block_samples,
            failure_tx,
        );
        let sink = match Sink::try_new(&self.handle) {
            Ok(sink) => sink,
            Err(error) => {
                drop(source);
                self.stop();
                return Err(format!("sink create failed: {error}"));
            }
        };
        sink.append(source);
        sink.play();
        self.sink = Some(sink);
        Ok(())
    }

    pub(crate) fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
            drop(sink);
        }
    }
}

impl Drop for AudioRuntime {
    fn drop(&mut self) {
        self.stop();
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
