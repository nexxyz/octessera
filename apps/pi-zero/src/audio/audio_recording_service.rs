use super::{AudioService, PhysicalOledFrame};
use crate::audio_recording;
use media_recording::{OledFrame, RecordingOutcome, RecordingStartError};
use playback_runtime::RuntimeStoreResult;
use std::sync::Arc;

impl AudioService {
    pub(crate) fn mix_taps(&self) -> super::MixTapState {
        self.mix_taps.clone()
    }

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
        self.mix_taps
            .write()
            .map_err(|_| RecordingStartError::Io("recording tap lock poisoned".into()))?
            .recording = Some(tap);
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
        self.mix_taps
            .write()
            .map_err(|_| RecordingStartError::Io("recording tap lock poisoned".into()))?
            .recording = Some(recording.tap);
        *self
            .recording_oled
            .write()
            .map_err(|_| RecordingStartError::Io("OLED recording lock poisoned".into()))? =
            Some(recording.oled);
        Ok(())
    }

    pub(crate) fn start_recording_audio_oled_from_latest(
        &self,
        max_minutes: u16,
    ) -> Result<(), RecordingStartError> {
        let accepted = self
            .accepted_oled_frame
            .read()
            .map_err(|_| RecordingStartError::Io("accepted OLED frame lock poisoned".into()))?;
        let seed = accepted
            .as_ref()
            .map(|frame| (frame.revision, frame.pixels.to_vec()));
        self.start_recording_audio_oled_with_seed(max_minutes, seed)
    }

    #[cfg(test)]
    pub(crate) fn latest_physical_oled_frame(&self) -> Option<(u64, Arc<[u8]>)> {
        self.accepted_oled_frame.read().ok().and_then(|frame| {
            frame
                .as_ref()
                .map(|frame| (frame.revision, Arc::clone(&frame.pixels)))
        })
    }

    pub(crate) fn clear_latest_physical_oled_frame(&self) -> Result<(), String> {
        *self
            .accepted_oled_frame
            .write()
            .map_err(|_| "accepted OLED frame lock poisoned".to_string())? = None;
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
        self.mix_taps
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())?
            .recording = None;
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
        audio_recording::poll_recording_status(&self.recorder, &self.mix_taps, &self.recording_oled)
    }

    pub(crate) fn prepare_restore(&self) -> Result<(), String> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?;
        if recorder.is_recording() {
            recorder.stop_audio().map_err(|error| error.to_string())?;
        }
        self.mix_taps
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())?
            .recording = None;
        *self
            .recording_oled
            .write()
            .map_err(|_| "OLED recording lock poisoned".to_string())? = None;
        Ok(())
    }

    pub fn is_recording(&self) -> Result<bool, String> {
        self.recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())
            .map(|recorder| recorder.is_recording())
    }

    #[cfg(test)]
    pub(crate) fn submit_accepted_oled_frame(
        &self,
        revision: u64,
        pixels: &[u8],
    ) -> Result<(), String> {
        self.submit_accepted_oled_frame_shared(revision, Arc::from(pixels.to_vec()))
    }

    pub(crate) fn submit_accepted_oled_frame_shared(
        &self,
        revision: u64,
        pixels: Arc<[u8]>,
    ) -> Result<(), String> {
        let mut accepted = self
            .accepted_oled_frame
            .write()
            .map_err(|_| "accepted OLED frame lock poisoned".to_string())?;
        if accepted.as_ref().is_some_and(|frame| {
            frame.revision == revision && frame.pixels.as_ref() == pixels.as_ref()
        }) {
            return Ok(());
        }
        *accepted = Some(PhysicalOledFrame {
            revision,
            pixels: Arc::clone(&pixels),
        });
        drop(accepted);
        if !self.is_recording()? {
            return Ok(());
        }
        let tap = self
            .mix_taps
            .read()
            .map_err(|_| "recording tap lock poisoned".to_string())?
            .recording
            .clone();
        let oled = self
            .recording_oled
            .read()
            .map_err(|_| "OLED recording lock poisoned".to_string())?
            .clone();
        let Some((tap, oled)) = tap.zip(oled) else {
            return Ok(());
        };
        let frame = OledFrame::from_bytes(revision, tap.audio_frame_cursor(), Arc::clone(&pixels))
            .map_err(|error| error.to_string())?;
        let _ = oled.try_submit(frame);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn test_push_recording_samples(&self, samples: &[i16]) -> Result<(), String> {
        let tap = self
            .mix_taps
            .read()
            .map_err(|_| "recording tap lock poisoned".to_string())?
            .recording
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
