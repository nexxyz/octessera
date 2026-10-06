use crate::recording::{RecordingEngineSource, RecordingTapState};
use playback_runtime::RunnerMessage;
use realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use rodio_engine_source::{AudioLoadStatusSender, EngineEventReceiver, EngineSource};
use serde_json::Value;

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
    ) -> Result<(), String> {
        self.stop();
        let source = RecordingEngineSource::new(
            EngineSource::with_load_status_tx(control_rx, DEFAULT_AUDIO_SAMPLE_RATE, Some(load_tx)),
            self.recording_tap.clone(),
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
