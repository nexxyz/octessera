use super::{
    avi, wav, AudioOledRecording, IngressState, OledIngress, OledIngressState, RecordingError,
    RecordingKind, RecordingOutcome, RecordingStartError, RecordingTap,
};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_MINUTES: u16 = 120;

struct ActiveRecording {
    tap: RecordingTap,
    oled: Option<OledIngress>,
    stop: Arc<AtomicBool>,
    partial_path: PathBuf,
    join: JoinHandle<Result<RecordingOutcome, RecordingError>>,
}

pub struct RecorderService {
    dir: PathBuf,
    active: Option<ActiveRecording>,
}

impl RecorderService {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, active: None }
    }

    pub fn start_audio(&mut self, max_minutes: u16) -> Result<RecordingTap, RecordingStartError> {
        let max_frames = u64::from(max_minutes.clamp(1, MAX_MINUTES))
            .saturating_mul(60)
            .saturating_mul(u64::from(super::SAMPLE_RATE));
        self.start_recording_worker(max_frames, RecordingKind::AudioWav)
            .map(|(tap, _)| tap)
    }

    pub fn start_audio_oled(
        &mut self,
        max_minutes: u16,
    ) -> Result<AudioOledRecording, RecordingStartError> {
        let max_frames = u64::from(max_minutes.clamp(1, MAX_MINUTES))
            .saturating_mul(60)
            .saturating_mul(u64::from(super::SAMPLE_RATE));
        let (tap, oled) = self.start_recording_worker(max_frames, RecordingKind::AudioOledAvi)?;
        Ok(AudioOledRecording {
            tap,
            oled: oled.expect("audio+OLED worker has OLED ingress"),
        })
    }

    pub fn stop_audio(&mut self) -> Result<Option<RecordingOutcome>, RecordingError> {
        let Some(active) = self.active.take() else {
            return Ok(None);
        };
        stop_active_recording(active)
    }

    pub fn is_recording(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.tap.is_active())
    }

    pub fn poll_completed(&mut self) -> Result<Option<RecordingOutcome>, RecordingError> {
        let Some(active) = self.active.as_ref() else {
            return Ok(None);
        };
        if active.tap.is_active() || !active.join.is_finished() {
            return Ok(None);
        }
        let active = self.active.take().expect("completed recording");
        join_recording(active.join, active.partial_path)
    }

    #[cfg(test)]
    pub(super) fn start_with_max_frames(
        &mut self,
        max_frames: u64,
    ) -> Result<RecordingTap, RecordingStartError> {
        self.start_recording_worker(max_frames, RecordingKind::AudioWav)
            .map(|(tap, _)| tap)
    }

    #[cfg(test)]
    pub(super) fn start_oled_with_max_frames(
        &mut self,
        max_frames: u64,
    ) -> Result<AudioOledRecording, RecordingStartError> {
        let (tap, oled) = self.start_recording_worker(max_frames, RecordingKind::AudioOledAvi)?;
        Ok(AudioOledRecording {
            tap,
            oled: oled.expect("audio+OLED worker has OLED ingress"),
        })
    }

    fn start_recording_worker(
        &mut self,
        max_frames: u64,
        kind: RecordingKind,
    ) -> Result<(RecordingTap, Option<OledIngress>), RecordingStartError> {
        if self.is_recording() {
            return Err(RecordingStartError::AlreadyActive);
        }
        self.finish_completed()
            .map_err(|error| RecordingStartError::Io(error.to_string()))?;
        fs::create_dir_all(&self.dir).map_err(|error| {
            RecordingStartError::Io(format!("recording directory unavailable: {error}"))
        })?;
        let (partial_path, final_path, file) = reserve_paths(&self.dir, kind).map_err(|error| {
            RecordingStartError::Io(format!("recording path unavailable: {error}"))
        })?;
        let (tx, rx) = mpsc::sync_channel(super::QUEUE_CAPACITY);
        let state = Arc::new(IngressState::new());
        let oled = (kind == RecordingKind::AudioOledAvi).then(|| OledIngress {
            state: Arc::new(OledIngressState::new()),
        });
        let tap = RecordingTap {
            tx,
            state: state.clone(),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker_state = state.clone();
        let worker_partial = partial_path.clone();
        let worker_oled = oled.clone();
        let worker = thread::Builder::new()
            .name(
                match kind {
                    RecordingKind::AudioWav => "octessera-wav-recorder",
                    RecordingKind::AudioOledAvi => "octessera-avi-recorder",
                }
                .into(),
            )
            .spawn(move || {
                let result = match kind {
                    RecordingKind::AudioWav => wav::write_recording(
                        file,
                        partial_path,
                        final_path,
                        rx,
                        worker_stop,
                        worker_state.clone(),
                        max_frames,
                    ),
                    RecordingKind::AudioOledAvi => avi::write_recording(avi::AviRecordingInput {
                        file,
                        partial_path,
                        final_path,
                        rx,
                        stop: worker_stop,
                        state: worker_state.clone(),
                        oled: worker_oled.expect("audio+OLED worker has OLED ingress"),
                        max_frames,
                        file_limit: super::AVI_TARGET_MAX_BYTES,
                    }),
                };
                worker_state.accepting.store(false, Ordering::Release);
                result
            })
            .map_err(|error| {
                let _ = fs::remove_file(&worker_partial);
                RecordingStartError::Io(format!("recording worker unavailable: {error}"))
            })?;
        self.active = Some(ActiveRecording {
            tap: tap.clone(),
            oled: oled.clone(),
            stop,
            partial_path: worker_partial,
            join: worker,
        });
        Ok((tap, oled))
    }

    fn finish_completed(&mut self) -> Result<(), RecordingError> {
        let Some(active) = self.active.take() else {
            return Ok(());
        };
        stop_active_recording(active).map(|_| ())
    }
}

impl Drop for RecorderService {
    fn drop(&mut self) {
        if let Err(error) = self.stop_audio() {
            eprintln!("audio recording stopped with an error: {error}");
        }
    }
}

fn stop_active_recording(
    active: ActiveRecording,
) -> Result<Option<RecordingOutcome>, RecordingError> {
    active.tap.deactivate_and_flush();
    if let Some(oled) = &active.oled {
        oled.close();
    }
    active.stop.store(true, Ordering::Release);
    drop(active.tap);
    join_recording(active.join, active.partial_path)
}

fn join_recording(
    join: JoinHandle<Result<RecordingOutcome, RecordingError>>,
    partial_path: PathBuf,
) -> Result<Option<RecordingOutcome>, RecordingError> {
    match join.join() {
        Ok(result) => result.map(Some),
        Err(_) => Err(RecordingError {
            message: "recording worker panicked; partial recording was retained".into(),
            partial_path,
        }),
    }
}

fn reserve_paths(dir: &Path, kind: RecordingKind) -> std::io::Result<(PathBuf, PathBuf, File)> {
    let stem = recording_stem();
    reserve_paths_with_stem_and_extension(dir, &stem, kind.extension())
}

fn reserve_paths_with_stem_and_extension(
    dir: &Path,
    stem: &str,
    extension: &str,
) -> std::io::Result<(PathBuf, PathBuf, File)> {
    let mut suffix = 0_u32;
    loop {
        let name = match suffix {
            0 => stem.to_string(),
            value => format!("{stem}-{value}"),
        };
        let partial = dir.join(format!("{name}.partial.{extension}"));
        let final_path = dir.join(format!("{name}.{extension}"));
        let incomplete_path = dir.join(format!("{name}.incomplete.{extension}"));
        if final_path.exists() || incomplete_path.exists() {
            suffix = suffix.saturating_add(1);
            continue;
        }
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
        {
            Ok(file) => return Ok((partial, final_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                suffix = suffix.saturating_add(1);
            }
            Err(error) => return Err(error),
        }
    }
}

impl RecordingKind {
    fn extension(self) -> &'static str {
        match self {
            Self::AudioWav => "wav",
            Self::AudioOledAvi => "avi",
        }
    }
}

#[cfg(test)]
pub(crate) fn reserve_paths_for_test(
    dir: &Path,
    stem: &str,
) -> std::io::Result<(PathBuf, PathBuf, File)> {
    reserve_paths_with_stem_and_extension(dir, stem, "wav")
}

#[cfg(test)]
pub(crate) fn reserve_avi_paths_for_test(
    dir: &Path,
    stem: &str,
) -> std::io::Result<(PathBuf, PathBuf, File)> {
    reserve_paths_with_stem_and_extension(dir, stem, "avi")
}

fn recording_stem() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("octessera-{seconds}")
}
