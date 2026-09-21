use media_recording::{
    AudioOledRecording, OledIngress, RecorderService, RecordingError, RecordingOutcome,
    RecordingStartError, RecordingStatus, RecordingTap,
};
use playback_runtime::{
    HostMessage, RuntimeAdapterError, RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorFacts,
    RuntimeOperation, RuntimePlatformRequest, RuntimeStoreResult,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

#[cfg(test)]
use crate::audio::AudioSink;

#[derive(Clone, Copy)]
enum RecordingMode {
    Audio,
    AudioOled,
}

pub(crate) struct RecordingServices {
    audio: RecorderService,
    audio_oled: RecorderService,
    active: Option<RecordingMode>,
}

impl RecordingServices {
    pub(crate) fn new(audio_directory: PathBuf, audio_oled_directory: PathBuf) -> Self {
        Self {
            audio: RecorderService::new(audio_directory),
            audio_oled: RecorderService::new(audio_oled_directory),
            active: None,
        }
    }

    pub(crate) fn start_audio(
        &mut self,
        max_minutes: u16,
    ) -> Result<RecordingTap, RecordingStartError> {
        self.prepare_start()?;
        let tap = self.audio.start_audio(max_minutes)?;
        self.active = Some(RecordingMode::Audio);
        Ok(tap)
    }

    pub(crate) fn start_audio_oled(
        &mut self,
        max_minutes: u16,
    ) -> Result<AudioOledRecording, RecordingStartError> {
        self.prepare_start()?;
        let recording = self.audio_oled.start_audio_oled(max_minutes)?;
        self.active = Some(RecordingMode::AudioOled);
        Ok(recording)
    }

    pub(crate) fn stop_audio(&mut self) -> Result<Option<RecordingOutcome>, RecordingError> {
        let Some(mode) = self.active.take() else {
            return Ok(None);
        };
        match mode {
            RecordingMode::Audio => self.audio.stop_audio(),
            RecordingMode::AudioOled => self.audio_oled.stop_audio(),
        }
    }

    pub(crate) fn poll_completed(&mut self) -> Result<Option<RecordingOutcome>, RecordingError> {
        let Some(mode) = self.active else {
            return Ok(None);
        };
        let result = match mode {
            RecordingMode::Audio => self.audio.poll_completed(),
            RecordingMode::AudioOled => self.audio_oled.poll_completed(),
        };
        if !matches!(&result, Ok(None)) {
            self.active = None;
        }
        result
    }

    pub(crate) fn is_recording(&self) -> bool {
        match self.active {
            Some(RecordingMode::Audio) => self.audio.is_recording(),
            Some(RecordingMode::AudioOled) => self.audio_oled.is_recording(),
            None => false,
        }
    }

    fn prepare_start(&mut self) -> Result<(), RecordingStartError> {
        if self.active.is_some() {
            if self.is_recording() {
                return Err(RecordingStartError::AlreadyActive);
            }
            self.stop_audio()
                .map_err(|error| RecordingStartError::Io(error.to_string()))?;
        }
        Ok(())
    }
}

pub(crate) fn poll_recording_status(
    recorder: &Arc<Mutex<RecordingServices>>,
    recording_tap: &Arc<RwLock<Option<RecordingTap>>>,
    recording_oled: &Arc<RwLock<Option<OledIngress>>>,
) -> Option<RuntimeStoreResult> {
    let mut recorder = match recorder.lock() {
        Ok(recorder) => recorder,
        Err(_) => {
            let clear_error = clear_recording_ingress(recording_tap, recording_oled).err();
            return Some(recording_failure(clear_error.map_or_else(
                || "recorder lock poisoned".to_string(),
                |error| format!("recorder lock poisoned; {error}"),
            )));
        }
    };
    let result = recorder.poll_completed();
    if !matches!(&result, Ok(None)) {
        if let Err(error) = clear_recording_ingress(recording_tap, recording_oled) {
            return Some(recording_failure(error));
        }
    }
    match result {
        Ok(Some(outcome)) => Some(max_time_recording_status(outcome.status)),
        Ok(None) => None,
        Err(error) => Some(recording_failure(error.to_string())),
    }
}

fn clear_recording_ingress(
    recording_tap: &Arc<RwLock<Option<RecordingTap>>>,
    recording_oled: &Arc<RwLock<Option<OledIngress>>>,
) -> Result<(), String> {
    *recording_tap
        .write()
        .map_err(|_| "recording tap lock poisoned".to_string())? = None;
    *recording_oled
        .write()
        .map_err(|_| "OLED recording lock poisoned".to_string())? = None;
    Ok(())
}

pub(crate) fn recording_status(status: RecordingStatus) -> RuntimeStoreResult {
    recording_status_with_complete_message(status, "Recording saved")
}

pub(crate) fn max_time_recording_status(status: RecordingStatus) -> RuntimeStoreResult {
    recording_status_with_complete_message(status, "Max time: saved")
}

fn recording_status_with_complete_message(
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

pub(crate) fn recording_start_result(
    result: Result<(), RecordingStartError>,
    request: &RuntimePlatformRequest,
) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
    let status = match result {
        Ok(()) => RuntimeStoreResult::RecordingStatus {
            ok: true,
            message: "Recording started".into(),
            active: true,
        },
        Err(RecordingStartError::AlreadyActive) => RuntimeStoreResult::RecordingStatus {
            ok: true,
            message: "Recording is already running".into(),
            active: true,
        },
        Err(RecordingStartError::Io(error)) => {
            return Err(RuntimeAdapterError::from_facts(
                request.failure_facts(error),
            ));
        }
    };
    Ok(vec![HostMessage::RuntimeResult { result: status }])
}

fn recording_failure(message: impl Into<String>) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Recording,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::Recording,
            Some(message.into()),
        ),
    }
}

#[cfg(test)]
pub(crate) fn recording_owner(_outputs: playback_runtime::AudioOutputSet) -> Option<AudioSink> {
    Some(AudioSink::Jack)
}

#[cfg(test)]
mod tests {
    #[test]
    fn recording_owner_covers_every_non_empty_output_set() {
        use super::recording_owner;
        use crate::audio::AudioSink;
        let cases = [
            ((true, false, false), Some(AudioSink::Jack)),
            ((false, true, false), Some(AudioSink::Jack)),
            ((false, false, true), Some(AudioSink::Jack)),
            ((true, true, false), Some(AudioSink::Jack)),
            ((true, false, true), Some(AudioSink::Jack)),
            ((false, true, true), Some(AudioSink::Jack)),
            ((true, true, true), Some(AudioSink::Jack)),
        ];
        for ((dac, usb, hdmi), owner) in cases {
            let outputs = playback_runtime::AudioOutputSet::from_flags(dac, usb, hdmi).unwrap();
            assert_eq!(recording_owner(outputs), owner);
        }
    }

    #[test]
    fn jack_recording_tap_policy_keeps_single_callback_owner() {
        use super::recording_owner;
        use crate::audio::AudioSink;

        assert_eq!(
            recording_owner(
                playback_runtime::AudioOutputSet::from_flags(false, true, false).unwrap()
            ),
            Some(AudioSink::Jack)
        );
        assert_eq!(
            recording_owner(
                playback_runtime::AudioOutputSet::from_flags(true, true, false).unwrap()
            ),
            Some(AudioSink::Jack)
        );
        assert_eq!(
            recording_owner(playback_runtime::AudioOutputSet::jack()),
            Some(AudioSink::Jack)
        );
    }
}
