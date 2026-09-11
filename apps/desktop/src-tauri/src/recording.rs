use media_recording::{
    AudioOledRecording, OledFrame, RecorderService, RecordingOutcome, RecordingStartError,
    RecordingTap,
};
use playback_runtime::PlaybackRuntime;
use rodio::Source;
use rodio_engine_source::EngineSource;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use tauri::path::BaseDirectory;
use tauri::Manager;

pub(crate) type RecordingTapState = Arc<RwLock<Option<RecordingTap>>>;
type AcceptedOledFrameState = Arc<RwLock<Option<(u64, Arc<[u8]>)>>>;

#[derive(Clone, Copy)]
enum RecordingMode {
    Audio,
    AudioOled,
}

struct DesktopRecorderServices {
    audio: RecorderService,
    audio_oled: RecorderService,
    active: Option<RecordingMode>,
}

impl DesktopRecorderServices {
    fn new(audio_directory: PathBuf, audio_oled_directory: PathBuf) -> Self {
        Self {
            audio: RecorderService::new(audio_directory),
            audio_oled: RecorderService::new(audio_oled_directory),
            active: None,
        }
    }

    fn start_audio(&mut self, max_minutes: u16) -> Result<RecordingTap, RecordingStartError> {
        self.prepare_start()?;
        let tap = self.audio.start_audio(max_minutes)?;
        self.active = Some(RecordingMode::Audio);
        Ok(tap)
    }

    fn start_audio_oled(
        &mut self,
        max_minutes: u16,
    ) -> Result<AudioOledRecording, RecordingStartError> {
        self.prepare_start()?;
        let recording = self.audio_oled.start_audio_oled(max_minutes)?;
        self.active = Some(RecordingMode::AudioOled);
        Ok(recording)
    }

    fn prepare_start(&mut self) -> Result<(), RecordingStartError> {
        if self.active.is_some() {
            if self.is_recording() {
                return Err(RecordingStartError::AlreadyActive);
            }
            self.stop_active()
                .map_err(|error| RecordingStartError::Io(error.to_string()))?;
        }
        Ok(())
    }

    fn is_recording(&self) -> bool {
        match self.active {
            Some(RecordingMode::Audio) => self.audio.is_recording(),
            Some(RecordingMode::AudioOled) => self.audio_oled.is_recording(),
            None => false,
        }
    }

    fn stop_active(&mut self) -> Result<Option<RecordingOutcome>, media_recording::RecordingError> {
        let Some(mode) = self.active.take() else {
            return Ok(None);
        };
        match mode {
            RecordingMode::Audio => self.audio.stop_audio(),
            RecordingMode::AudioOled => self.audio_oled.stop_audio(),
        }
    }

    fn poll_completed(
        &mut self,
    ) -> Result<Option<RecordingOutcome>, media_recording::RecordingError> {
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
}

#[derive(Clone)]
pub(crate) struct DesktopRecording {
    recorder: Arc<Mutex<DesktopRecorderServices>>,
    tap: RecordingTapState,
    oled: Arc<RwLock<Option<media_recording::OledIngress>>>,
    accepted_oled: AcceptedOledFrameState,
}

impl DesktopRecording {
    #[cfg(test)]
    pub(crate) fn new(directory: PathBuf) -> Self {
        Self::with_screen_directory(directory.clone(), directory)
    }

    pub(crate) fn with_screen_directory(
        audio_directory: PathBuf,
        audio_oled_directory: PathBuf,
    ) -> Self {
        Self {
            recorder: Arc::new(Mutex::new(DesktopRecorderServices::new(
                audio_directory,
                audio_oled_directory,
            ))),
            tap: Arc::new(RwLock::new(None)),
            oled: Arc::new(RwLock::new(None)),
            accepted_oled: Arc::new(RwLock::new(None)),
        }
    }

    pub(crate) fn tap_state(&self) -> RecordingTapState {
        self.tap.clone()
    }

    pub(crate) fn start_audio(&self, max_minutes: u16) -> Result<(), String> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?;
        let recording_tap = recorder
            .start_audio(max_minutes)
            .map_err(|error| error.to_string())?;
        self.clear_oled_ingress()?;
        let mut tap = self
            .tap
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())?;
        *tap = Some(recording_tap);
        Ok(())
    }

    pub(crate) fn start_audio_oled(&self, max_minutes: u16) -> Result<(), String> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?;
        let recording = recorder
            .start_audio_oled(max_minutes)
            .map_err(|error| error.to_string())?;
        if let Some((revision, pixels)) = self
            .accepted_oled
            .read()
            .map_err(|_| "accepted OLED frame lock poisoned".to_string())?
            .clone()
        {
            let frame = OledFrame::from_bytes(revision, 0, pixels.as_ref())
                .map_err(|error| error.to_string())?;
            let _ = recording.oled.try_submit(frame);
        }
        let mut tap = self
            .tap
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())?;
        let mut oled = self
            .oled
            .write()
            .map_err(|_| "OLED recording lock poisoned".to_string())?;
        *tap = Some(recording.tap);
        *oled = Some(recording.oled);
        Ok(())
    }

    pub(crate) fn stop_audio(&self) -> Result<Option<RecordingOutcome>, String> {
        let mut recorder = self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?;
        self.clear_ingress()?;
        recorder.stop_active().map_err(|error| error.to_string())
    }

    pub(crate) fn poll_completion(&self) -> Result<Option<RecordingOutcome>, String> {
        let mut recorder = match self.recorder.lock() {
            Ok(recorder) => recorder,
            Err(_) => {
                let clear_error = self.clear_ingress().err();
                return Err(clear_error.map_or_else(
                    || "recorder lock poisoned".to_string(),
                    |error| format!("recorder lock poisoned; {error}"),
                ));
            }
        };
        let result = recorder.poll_completed().map_err(|error| error.to_string());
        if !matches!(&result, Ok(None)) {
            self.clear_ingress()?;
        }
        result
    }

    fn clear_ingress(&self) -> Result<(), String> {
        *self
            .tap
            .write()
            .map_err(|_| "recording tap lock poisoned".to_string())? = None;
        self.clear_oled_ingress()?;
        Ok(())
    }

    fn clear_oled_ingress(&self) -> Result<(), String> {
        *self
            .oled
            .write()
            .map_err(|_| "OLED recording lock poisoned".to_string())? = None;
        Ok(())
    }

    pub(crate) fn remember_accepted_oled_frame(
        &self,
        revision: u64,
        pixels: &[u8],
    ) -> Result<(), String> {
        *self
            .accepted_oled
            .write()
            .map_err(|_| "accepted OLED frame lock poisoned".to_string())? =
            Some((revision, Arc::from(pixels.to_vec())));
        Ok(())
    }

    pub(crate) fn submit_accepted_oled_frame(
        &self,
        revision: u64,
        pixels: &[u8],
    ) -> Result<(), String> {
        if !self
            .recorder
            .lock()
            .map_err(|_| "recorder lock poisoned".to_string())?
            .is_recording()
        {
            return Ok(());
        }
        let tap = self
            .tap
            .read()
            .map_err(|_| "recording tap lock poisoned".to_string())?
            .clone();
        let oled = self
            .oled
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

    pub(crate) fn submit_current_accepted_oled_frame(
        &self,
        playback: &PlaybackRuntime,
    ) -> Result<(), String> {
        let Some(snapshot) = playback.last_snapshot() else {
            return Ok(());
        };
        let Some(revision) = snapshot
            .get("oledFrameRevision")
            .and_then(serde_json::Value::as_u64)
        else {
            return Ok(());
        };
        if revision != playback.oled_frame_revision() {
            return Ok(());
        }
        let Some(pixels) = playback.last_oled_frame() else {
            return Ok(());
        };
        self.remember_accepted_oled_frame(revision, pixels)?;
        self.submit_accepted_oled_frame(revision, pixels)
    }
}

pub(crate) struct RecordingEngineSource {
    source: EngineSource,
    tap: RecordingTapState,
    pending_left: Option<(f32, Option<RecordingTap>)>,
}

impl RecordingEngineSource {
    pub(crate) fn new(source: EngineSource, tap: RecordingTapState) -> Self {
        Self {
            source,
            tap,
            pending_left: None,
        }
    }
}

impl Iterator for RecordingEngineSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.source.next()?;
        if let Some((left, tap)) = self.pending_left.take() {
            if let Some(tap) = tap {
                tap.push_frame(float_to_i16(left), float_to_i16(sample));
            }
        } else {
            let tap = self.tap.try_read().ok().and_then(|tap| tap.clone());
            self.pending_left = Some((sample, tap));
        }
        Some(sample)
    }
}

impl Source for RecordingEngineSource {
    fn current_frame_len(&self) -> Option<usize> {
        self.source.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.source.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.source.total_duration()
    }
}

pub(crate) fn resolve_recordings_dir(app: &tauri::App) -> Result<PathBuf, String> {
    app.path()
        .resolve("Octessera/recordings", BaseDirectory::Audio)
        .map_err(|error| format!("recording directory resolve failed: {error}"))
}

pub(crate) fn resolve_screen_recordings_dir(app: &tauri::App) -> Result<PathBuf, String> {
    app.path()
        .resolve("Octessera/screen-recordings", BaseDirectory::Audio)
        .map_err(|error| format!("screen recording directory resolve failed: {error}"))
}

fn float_to_i16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}

#[cfg(test)]
#[path = "recording_tests.rs"]
mod recording_tests;
