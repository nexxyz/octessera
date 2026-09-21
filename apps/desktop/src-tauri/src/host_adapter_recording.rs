use crate::recording::DesktopRecording;
use media_recording::{RecordingStartError, RecordingStatus};
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
            .map(|_| Some(recording_started_status()))
            .or_else(|error| recording_start_result(error, request)),
        RuntimePlatformEffect::RecordingStartAudioOled { max_minutes } => recording
            .start_audio_oled(*max_minutes)
            .map(|_| Some(recording_started_status()))
            .or_else(|error| recording_start_result(error, request)),
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
    _request: &RuntimePlatformRequest,
) -> Result<Option<RuntimeStoreResult>, RuntimeAdapterError> {
    recording
        .stop_audio()
        .map(|outcome| outcome.map(|outcome| status_for_recording(outcome.status)))
        .map_err(recording_finalization_error)
}

pub(crate) fn status_for_recording(status: RecordingStatus) -> RuntimeStoreResult {
    status_for_recording_with_message(status, "Recording saved")
}

pub(crate) fn max_time_status_for_recording(status: RecordingStatus) -> RuntimeStoreResult {
    status_for_recording_with_message(status, "Max time: saved")
}

fn status_for_recording_with_message(
    status: RecordingStatus,
    complete_message: &str,
) -> RuntimeStoreResult {
    match status {
        RecordingStatus::Complete => RuntimeStoreResult::RecordingStatus {
            ok: true,
            message: complete_message.into(),
            active: false,
        },
        RecordingStatus::Incomplete { .. } => RuntimeStoreResult::RecordingStatus {
            ok: false,
            message: "Recording incomplete".into(),
            active: false,
        },
    }
}

fn recording_started_status() -> RuntimeStoreResult {
    RuntimeStoreResult::RecordingStatus {
        ok: true,
        message: "Recording started".into(),
        active: true,
    }
}

fn recording_start_result(
    error: RecordingStartError,
    request: &RuntimePlatformRequest,
) -> Result<Option<RuntimeStoreResult>, RuntimeAdapterError> {
    match error {
        RecordingStartError::AlreadyActive => Ok(Some(RuntimeStoreResult::RecordingStatus {
            ok: true,
            message: "Recording is already running".into(),
            active: true,
        })),
        RecordingStartError::Io(error) => Err(RuntimeAdapterError::from_facts(
            request.failure_facts(error),
        )),
    }
}

fn recording_finalization_error(error: String) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Recording,
        RuntimeErrorCode::OperationFailed,
        RuntimeOperation::Recording,
        Some(error),
    ))
}

pub(crate) fn poll_status(recording: &DesktopRecording) -> Option<RuntimeStoreResult> {
    match recording.poll_completion() {
        Ok(Some(outcome)) => Some(max_time_status_for_recording(outcome.status)),
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
