use super::avi_writer::{AviWriter, FinishStats};
use super::{
    IngressState, OledFrame, OledIngress, RecordingChunk, RecordingError, RecordingOutcome,
    OLED_FRAME_BYTES, OLED_HEIGHT, OLED_WIDTH, SAMPLE_RATE,
};
use image::codecs::jpeg::JpegEncoder;
use image::ColorType;
use std::fs::File;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;

const RECEIVE_TIMEOUT: Duration = Duration::from_millis(100);
const VIDEO_FPS: u64 = 10;
const VIDEO_UNIT_FRAMES: usize = SAMPLE_RATE as usize / VIDEO_FPS as usize;
const JPEG_QUALITY: u8 = 82;
const TIMELINE_PENDING_CAP: usize = super::OLED_QUEUE_CAPACITY;

pub(crate) struct AviRecordingInput {
    pub(crate) file: File,
    pub(crate) partial_path: PathBuf,
    pub(crate) final_path: PathBuf,
    pub(crate) rx: Receiver<RecordingChunk>,
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) state: Arc<IngressState>,
    pub(crate) oled: OledIngress,
    pub(crate) max_frames: u64,
    pub(crate) file_limit: u64,
}

pub(crate) fn write_recording(
    input: AviRecordingInput,
) -> Result<RecordingOutcome, RecordingError> {
    let AviRecordingInput {
        file,
        partial_path,
        final_path,
        rx,
        stop,
        state,
        oled,
        max_frames,
        file_limit,
    } = input;
    let writer = AviWriter::new(file, partial_path, final_path, file_limit)?;
    let mut timeline = Timeline::new(writer, oled.clone());
    let mut next_input_frame = 0_u64;
    let mut gap_count = 0_u64;
    let mut gap_frames = 0_u64;
    let mut size_limited = false;

    loop {
        let chunk = if stop.load(Ordering::Acquire) {
            match rx.try_recv() {
                Ok(chunk) => chunk,
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => break,
            }
        } else {
            match rx.recv_timeout(RECEIVE_TIMEOUT) {
                Ok(chunk) => chunk,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        };
        if chunk.frame_offset() > next_input_frame {
            let gap = chunk.frame_offset() - next_input_frame;
            gap_count = gap_count.saturating_add(1);
            gap_frames = gap_frames.saturating_add(gap);
            let inserted = gap.min(max_frames.saturating_sub(next_input_frame));
            size_limited |= timeline.push_silence(inserted as usize);
            next_input_frame = next_input_frame.saturating_add(inserted);
        }
        if size_limited || next_input_frame >= max_frames {
            break;
        }
        let skip = next_input_frame.saturating_sub(chunk.frame_offset()) as usize;
        let available = chunk.frame_count().saturating_sub(skip);
        let frames = available.min((max_frames - next_input_frame) as usize);
        if frames != 0 {
            let samples = &chunk.samples()[skip * 2..(skip + frames) * 2];
            size_limited |= timeline.push_samples(samples);
            next_input_frame = next_input_frame.saturating_add(frames as u64);
        }
        if size_limited || next_input_frame >= max_frames {
            break;
        }
    }

    if !size_limited {
        let cursor = state.next_frame.load(Ordering::Acquire).min(max_frames);
        if cursor > next_input_frame {
            let gap = cursor - next_input_frame;
            gap_count = gap_count.saturating_add(1);
            gap_frames = gap_frames.saturating_add(gap);
            let _ = timeline.push_silence(gap as usize);
        }
    }

    let stats = FinishStats {
        frames: timeline.audio_frames(),
        gap_count,
        gap_frames,
        overflow_count: state.overflow_chunks.load(Ordering::Acquire),
        overflow_frames: state.overflow_frames.load(Ordering::Acquire),
        material_loss: oled.material_loss_count(),
        incomplete: gap_count != 0
            || state.overflow_chunks.load(Ordering::Acquire) != 0
            || oled.material_loss_count() != 0,
    };
    timeline.finish(stats)
}

struct Timeline {
    writer: AviWriter,
    oled: OledIngress,
    pending_frames: Vec<OledFrame>,
    pending_audio: Vec<i16>,
    current_pixels: Box<[u8; OLED_FRAME_BYTES]>,
    encoded_pixels: Option<Vec<u8>>,
    material_loss: u64,
}

impl Timeline {
    fn new(writer: AviWriter, oled: OledIngress) -> Self {
        Self {
            writer,
            oled,
            pending_frames: Vec::new(),
            pending_audio: Vec::with_capacity(VIDEO_UNIT_FRAMES * 2),
            current_pixels: Box::new([0; OLED_FRAME_BYTES]),
            encoded_pixels: None,
            material_loss: 0,
        }
    }

    fn audio_frames(&self) -> u64 {
        self.writer.audio_frames + (self.pending_audio.len() / 2) as u64
    }

    fn push_silence(&mut self, frames: usize) -> bool {
        let mut remaining = frames;
        while remaining != 0 {
            let take = remaining.min(VIDEO_UNIT_FRAMES);
            self.pending_audio
                .extend(std::iter::repeat_n(0_i16, take * 2));
            if self.flush_units() {
                return true;
            }
            remaining -= take;
        }
        false
    }

    fn push_samples(&mut self, samples: &[i16]) -> bool {
        self.pending_audio.extend_from_slice(samples);
        self.flush_units()
    }

    fn flush_units(&mut self) -> bool {
        while self.pending_audio.len() >= VIDEO_UNIT_FRAMES * 2 {
            let jpeg = match self.video_bytes(self.writer.audio_frames) {
                Ok(jpeg) => jpeg,
                Err(error) => {
                    self.writer.fail(error);
                    return true;
                }
            };
            let audio = self.pending_audio[..VIDEO_UNIT_FRAMES * 2].to_vec();
            match self.writer.write_unit(&jpeg, &audio) {
                Ok(true) => {}
                Ok(false) => return true,
                Err(error) => {
                    self.writer.fail(error);
                    return true;
                }
            }
            self.pending_audio.drain(..VIDEO_UNIT_FRAMES * 2);
        }
        false
    }

    fn video_bytes(&mut self, tick: u64) -> Result<Vec<u8>, String> {
        self.pending_frames.extend(self.oled.drain());
        let selected = self
            .pending_frames
            .iter()
            .filter(|frame| frame.audio_frame() <= tick)
            .max_by_key(|frame| (frame.audio_frame(), frame.revision()));
        if let Some(frame) = selected {
            if self.current_pixels.as_ref() != frame.pixels() {
                self.current_pixels.copy_from_slice(frame.pixels());
                self.encoded_pixels = None;
            }
        }
        self.pending_frames
            .retain(|frame| frame.audio_frame() > tick);
        self.bound_pending_frames();
        if let Some(encoded) = &self.encoded_pixels {
            return Ok(encoded.clone());
        }
        let encoded = encode_jpeg(&self.current_pixels)?;
        self.encoded_pixels = Some(encoded.clone());
        Ok(encoded)
    }

    fn finish(mut self, mut stats: FinishStats) -> Result<RecordingOutcome, RecordingError> {
        if !self.pending_audio.is_empty() {
            let jpeg = self
                .video_bytes(self.writer.audio_frames)
                .map_err(|error| recording_error(self.writer.partial_path(), error))?;
            let audio = std::mem::take(&mut self.pending_audio);
            match self.writer.write_unit(&jpeg, &audio) {
                Ok(true) | Ok(false) => {}
                Err(error) => self.writer.fail(error),
            }
        }
        stats.material_loss = stats.material_loss.saturating_add(self.material_loss);
        stats.incomplete |= stats.material_loss != 0;
        stats.incomplete |= self.writer.write_failed.is_some();
        stats.frames = stats.frames.min(self.writer.audio_frames);
        self.writer.finish(stats)
    }

    fn bound_pending_frames(&mut self) {
        self.pending_frames
            .sort_by_key(|frame| (frame.audio_frame(), frame.revision()));
        let mut compacted: Vec<OledFrame> = Vec::with_capacity(self.pending_frames.len());
        for frame in self.pending_frames.drain(..) {
            if let Some(previous) = compacted.last_mut() {
                if previous.audio_frame() == frame.audio_frame() {
                    *previous = frame;
                    continue;
                }
                if previous.pixels() == frame.pixels() {
                    continue;
                }
            }
            compacted.push(frame);
        }
        self.pending_frames = compacted;
        if self.pending_frames.len() > TIMELINE_PENDING_CAP {
            let newest = self.pending_frames.pop().expect("future OLED frame");
            let dropped = self.pending_frames.len() - (TIMELINE_PENDING_CAP - 1);
            self.pending_frames.truncate(TIMELINE_PENDING_CAP - 1);
            self.pending_frames.push(newest);
            self.material_loss = self.material_loss.saturating_add(dropped as u64);
        }
        self.pending_frames.shrink_to(TIMELINE_PENDING_CAP);
    }
}

fn encode_jpeg(pixels: &[u8; OLED_FRAME_BYTES]) -> Result<Vec<u8>, String> {
    let mut rgb = vec![0_u8; (OLED_WIDTH * OLED_HEIGHT * 3) as usize];
    for (index, pixel) in pixels.chunks_exact(2).enumerate() {
        let value = u16::from_be_bytes([pixel[0], pixel[1]]);
        let red = u8::try_from((u32::from(value >> 11) * 255 + 15) / 31).unwrap_or(u8::MAX);
        let green =
            u8::try_from((u32::from((value >> 5) & 0x3f) * 255 + 31) / 63).unwrap_or(u8::MAX);
        let blue = u8::try_from((u32::from(value & 0x1f) * 255 + 15) / 31).unwrap_or(u8::MAX);
        let offset = index * 3;
        rgb[offset..offset + 3].copy_from_slice(&[red, green, blue]);
    }
    let mut encoded = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, JPEG_QUALITY)
        .encode(&rgb, OLED_WIDTH, OLED_HEIGHT, ColorType::Rgb8.into())
        .map_err(|error| error.to_string())?;
    Ok(encoded)
}

fn recording_error(
    partial_path: &std::path::Path,
    error: impl std::fmt::Display,
) -> RecordingError {
    RecordingError {
        message: format!(
            "audio+OLED recording failed for {}: {error}",
            partial_path.display()
        ),
        partial_path: partial_path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OledIngressState, RecordingStatus};
    use std::fs::{self, OpenOptions};

    #[test]
    fn future_frames_stay_bounded_and_report_loss_on_slow_timeline() {
        let dir =
            std::env::temp_dir().join(format!("octessera-avi-timeline-cap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let partial = dir.join("take.partial.avi");
        let final_path = dir.join("take.avi");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
            .unwrap();
        let oled = OledIngress {
            state: Arc::new(OledIngressState::new()),
        };
        let writer = AviWriter::new(
            file,
            partial,
            final_path,
            super::super::AVI_TARGET_MAX_BYTES,
        )
        .unwrap();
        let mut timeline = Timeline::new(writer, oled.clone());

        for revision in 0..TIMELINE_PENDING_CAP as u64 {
            assert!(
                oled.try_submit(future_frame(revision, 10_000 + revision, revision as u8))
                    .accepted
            );
        }
        timeline.video_bytes(0).unwrap();
        assert_eq!(timeline.pending_frames.len(), TIMELINE_PENDING_CAP);
        assert_eq!(timeline.material_loss, 0);

        for revision in TIMELINE_PENDING_CAP as u64..(TIMELINE_PENDING_CAP * 2) as u64 {
            assert!(
                oled.try_submit(future_frame(revision, 20_000 + revision, revision as u8))
                    .accepted
            );
        }
        timeline.video_bytes(0).unwrap();
        assert_eq!(timeline.pending_frames.len(), TIMELINE_PENDING_CAP);
        assert!(timeline.material_loss > 0);

        assert!(!timeline.push_samples(&vec![0_i16; VIDEO_UNIT_FRAMES * 2]));
        let frames = timeline.audio_frames();
        let outcome = timeline
            .finish(FinishStats {
                frames,
                gap_count: 0,
                gap_frames: 0,
                overflow_count: 0,
                overflow_frames: 0,
                material_loss: 0,
                incomplete: false,
            })
            .unwrap();
        assert!(outcome.path.to_string_lossy().contains(".incomplete.avi"));
        assert!(matches!(outcome.status, RecordingStatus::Incomplete { .. }));
        let bytes = fs::read(outcome.path).unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
            bytes.len() - 8
        );
        let _ = fs::remove_dir_all(dir);
    }

    fn future_frame(revision: u64, audio_frame: u64, value: u8) -> OledFrame {
        let mut pixels = [0_u8; OLED_FRAME_BYTES];
        pixels[0] = value;
        pixels[1] = 0xff;
        OledFrame::new(revision, audio_frame, pixels)
    }
}
