use super::AudioService;
use crate::audio_recording;
use media_recording::{OledFrame, RecordingOutcome, RecordingStartError};
use playback_runtime::RuntimeStoreResult;

impl AudioService {
    pub fn start_recording(&self, max_minutes: u16) -> Result<(), RecordingStartError> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| RecordingStartError::Io("recorder lock poisoned".into()))?;
        let tap = recorder.start_audio(max_minutes)?;
        *self
            .recording_oled
            .write()
            .map_err(|_| RecordingStartError::Io("OLED recording lock poisoned".into()))? = None;
        *self
            .recording_tap
            .write()
            .map_err(|_| RecordingStartError::Io("recording tap lock poisoned".into()))? =
            Some(tap);
        Ok(())
    }

    pub(crate) fn start_recording_audio_oled_with_seed(
        &self,
        max_minutes: u16,
        seed: Option<(u64, Vec<u8>)>,
    ) -> Result<(), RecordingStartError> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| RecordingStartError::Io("recorder lock poisoned".into()))?;
        let recording = recorder.start_audio_oled(max_minutes)?;
        if let Some((revision, pixels)) = seed {
            let frame = OledFrame::from_bytes(revision, 0, pixels)
                .map_err(|error| RecordingStartError::Io(error.to_string()))?;
            let _ = recording.oled.try_submit(frame);
        }
        *self
            .recording_tap
            .write()
            .map_err(|_| RecordingStartError::Io("recording tap lock poisoned".into()))? =
            Some(recording.tap);
        *self
            .recording_oled
            .write()
            .map_err(|_| RecordingStartError::Io("OLED recording lock poisoned".into()))? =
            Some(recording.oled);
        Ok(())
    }

    pub fn stop_recording(&self) -> Result<(), String> {
        self.stop_recording_with_outcome().map(|_| ())
    }

    pub(crate) fn stop_recording_with_outcome(&self) -> Result<Option<RecordingOutcome>, String> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?;
        *self
            .recording_tap
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())? = None;
        *self
            .recording_oled
            .write()
            .map_err(|_| "OLED recording lock poisoned".to_string())? = None;
        let outcome = recorder.stop_audio().map_err(|error| error.to_string())?;
        if let Some(outcome) = &outcome {
            println!(
                "recording stopped: path={} frames={} status={:?}",
                outcome.path.display(),
                outcome.frames_written,
                outcome.status
            );
        }
        Ok(outcome)
    }

    pub(crate) fn poll_recording_status(&self) -> Option<RuntimeStoreResult> {
        audio_recording::poll_recording_status(
            &self.recorder,
            &self.recording_tap,
            &self.recording_oled,
        )
    }

    pub(crate) fn prepare_restore(&self) -> Result<(), String> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?;
        if recorder.is_recording() {
            recorder.stop_audio().map_err(|error| error.to_string())?;
        }
        *self
            .recording_tap
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())? = None;
        *self
            .recording_oled
            .write()
            .map_err(|_| "OLED recording lock poisoned".to_string())? = None;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn is_recording(&self) -> Result<bool, String> {
        self.recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())
            .map(|recorder| recorder.is_recording())
    }

    pub(crate) fn submit_accepted_oled_frame(
        &self,
        revision: u64,
        pixels: &[u8],
    ) -> Result<(), String> {
        if !self.is_recording()? {
            return Ok(());
        }
        let tap = self
            .recording_tap
            .read()
            .map_err(|_| "recording tap lock poisoned".to_string())?
            .clone();
        let oled = self
            .recording_oled
            .read()
            .map_err(|_| "OLED recording lock poisoned".to_string())?
            .clone();
        let Some((tap, oled)) = tap.zip(oled) else {
            return Ok(());
        };
        let frame = OledFrame::from_bytes(revision, tap.audio_frame_cursor(), pixels)
            .map_err(|error| error.to_string())?;
        let _ = oled.try_submit(frame);
        Ok(())
    }

    #[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn test_push_recording_samples(&self, samples: &[i16]) -> Result<(), String> {
        let tap = self
            .recording_tap
            .read()
            .map_err(|_| "recording tap lock poisoned".to_string())?
            .clone()
            .ok_or_else(|| "recording tap is inactive".to_string())?;
        let mut chunk = tap.new_chunk();
        let (frames, _) = samples.as_chunks::<2>();
        for frame in frames {
            if !chunk.push_frame(frame[0], frame[1]) {
                tap.push_chunk(chunk);
                chunk = tap.new_chunk();
                assert!(chunk.push_frame(frame[0], frame[1]));
            }
        }
        if !chunk.is_empty() {
            tap.push_chunk(chunk);
        }
        Ok(())
    }
}
