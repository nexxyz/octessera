use super::PiPlaybackHostAdapter;
use media_recording::RecordingStartError;
use playback_runtime::{
    HostMessage, RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts,
    RuntimeOperation, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};

impl PiPlaybackHostAdapter {
    pub(super) fn stop_recording_for_transition(
        &self,
        _request: &RuntimePlatformRequest,
    ) -> Result<Option<RuntimeStoreResult>, RuntimeAdapterError> {
        let Some(audio) = &self.audio else {
            return Ok(None);
        };
        audio
            .stop_recording_with_outcome()
            .map(|outcome| {
                outcome.map(|outcome| crate::audio_recording::recording_status(outcome.status))
            })
            .map_err(recording_finalization_error)
    }

    pub(super) fn handle_recording_effect(
        &self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        match &request.effect {
            RuntimePlatformEffect::RecordingStartAudio { max_minutes } => {
                crate::audio_recording::recording_start_result(
                    self.audio.as_ref().map_or_else(
                        || Err(recording_unavailable()),
                        |audio| audio.start_recording(*max_minutes),
                    ),
                    request,
                )
            }
            RuntimePlatformEffect::RecordingStartAudioOled { max_minutes } => {
                let seed = self
                    .oled_frame_cache
                    .accepted_frame()
                    .map(|frame| (frame.revision(), frame.pixels().to_vec()));
                crate::audio_recording::recording_start_result(
                    self.audio.as_ref().map_or_else(
                        || Err(recording_unavailable()),
                        |audio| audio.start_recording_audio_oled_with_seed(*max_minutes, seed),
                    ),
                    request,
                )
            }
            RuntimePlatformEffect::RecordingStop => Ok(self
                .stop_recording_for_transition(request)?
                .into_iter()
                .map(|result| HostMessage::RuntimeResult { result })
                .collect()),
            _ => Err(RuntimeAdapterError::from(
                "non-recording effect sent to Pi recording adapter",
            )),
        }
    }
}

fn recording_unavailable() -> RecordingStartError {
    RecordingStartError::Io("audio recording unavailable".into())
}

fn recording_finalization_error(error: String) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Recording,
        RuntimeErrorCode::OperationFailed,
        RuntimeOperation::Recording,
        Some(error),
    ))
}
