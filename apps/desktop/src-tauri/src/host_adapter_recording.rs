use crate::recording::DesktopRecording;
use media_recording::RecordingStatus;
use playback_runtime::{
    PlaybackRuntime, RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts,
    RuntimeOperation, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};

pub(crate) fn handle_effect(
    recording: &DesktopRecording,
    request: &RuntimePlatformRequest,
) -> Result<Option<RuntimeStoreResult>, RuntimeAdapterError> {
    match &request.effect {
        RuntimePlatformEffect::RecordingStartAudio { max_minutes } => recording
            .start_audio(*max_minutes)
            .map(|_| None)
            .map_err(|error| RuntimeAdapterError::from_facts(request.failure_facts(error))),
        RuntimePlatformEffect::RecordingStartAudioOled { max_minutes } => recording
            .start_audio_oled(*max_minutes)
            .map(|_| None)
            .map_err(|error| RuntimeAdapterError::from_facts(request.failure_facts(error))),
        RuntimePlatformEffect::RecordingStop => recording
            .stop_audio()
            .map(|outcome| outcome.map(|outcome| status_for_recording(outcome.status)))
            .map_err(|error| RuntimeAdapterError::from_facts(request.failure_facts(error))),
        _ => Err(RuntimeAdapterError::from(
            "non-recording effect sent to desktop recording adapter",
        )),
    }
}

pub(crate) fn finalize_for_shutdown(
    recording: &DesktopRecording,
    request: &RuntimePlatformRequest,
) -> Result<Option<RuntimeStoreResult>, RuntimeAdapterError> {
    recording
        .stop_audio()
        .map(|outcome| outcome.map(|outcome| status_for_recording(outcome.status)))
        .map_err(|error| RuntimeAdapterError::from_facts(request.failure_facts(error)))
}

pub(crate) fn status_for_recording(status: RecordingStatus) -> RuntimeStoreResult {
    match status {
        RecordingStatus::Complete => RuntimeStoreResult::RecordingStatus {
            ok: true,
            message: "Recording saved".into(),
        },
        RecordingStatus::Incomplete { .. } => RuntimeStoreResult::RecordingStatus {
            ok: false,
            message: "Recording incomplete".into(),
        },
    }
}

pub(crate) fn poll_status(recording: &DesktopRecording) -> Option<RuntimeStoreResult> {
    match recording.poll_completion() {
        Ok(Some(outcome)) => Some(status_for_recording(outcome.status)),
        Ok(None) => None,
        Err(error) => Some(RuntimeStoreResult::RuntimeFailure {
            error: RuntimeErrorFacts::new(
                RuntimeErrorDomain::Recording,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::Recording,
                Some(error),
            ),
        }),
    }
}

impl super::DesktopPlaybackHostAdapter {
    pub(crate) fn poll_recording_status(&self) -> Option<RuntimeStoreResult> {
        poll_status(&self.audio.recording)
    }

    pub(crate) fn submit_accepted_oled_frame(
        &self,
        playback: &PlaybackRuntime,
    ) -> Result<(), String> {
        self.audio
            .recording
            .submit_current_accepted_oled_frame(playback)
    }
}
