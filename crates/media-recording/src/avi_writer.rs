use super::{
    RecordingError, RecordingOutcome, RecordingStatus, BITS_PER_SAMPLE, CHANNELS, VIDEO_FPS,
};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub(crate) struct FinishStats {
    pub(crate) frames: u64,
    pub(crate) gap_count: u64,
    pub(crate) gap_frames: u64,
    pub(crate) overflow_count: u64,
    pub(crate) overflow_frames: u64,
    pub(crate) incomplete: bool,
    pub(crate) size_limited: bool,
}

pub(crate) struct AviWriter {
    file: File,
    partial_path: PathBuf,
    final_path: PathBuf,
    movi_list_start: u64,
    avih_frames_offset: u64,
    avih_bytes_per_sec_offset: u64,
    avih_buffer_offset: u64,
    video_length_offset: u64,
    video_buffer_offset: u64,
    video_image_size_offset: u64,
    audio_length_offset: u64,
    audio_buffer_offset: u64,
    max_video_payload: u64,
    max_audio_payload: u64,
    file_limit: u64,
    index: Vec<IndexEntry>,
    pub(crate) video_frames: u64,
    pub(crate) audio_frames: u64,
    pub(crate) write_failed: Option<String>,
}

#[derive(Clone, Copy)]
struct IndexEntry {
    id: [u8; 4],
    flags: u32,
    offset: u32,
    size: u32,
}

impl AviWriter {
    pub(crate) fn new(
        mut file: File,
        partial_path: PathBuf,
        final_path: PathBuf,
        file_limit: u64,
    ) -> Result<Self, RecordingError> {
        let offsets =
            write_headers(&mut file).map_err(|error| recording_error(&partial_path, error))?;
        Ok(Self::with_offsets(
            file,
            partial_path,
            final_path,
            file_limit,
            offsets,
        ))
    }

    /// Reopens a `.partial.avi` take whose header this writer produced, positioned at the end of
    /// the movi chunks recorded so far, for `finish` to write the sizes and index. The sizes
    /// `finish` patches are not compared, so a take cut while finishing is recognized too.
    pub(crate) fn reopen(
        mut file: File,
        partial_path: PathBuf,
        final_path: PathBuf,
    ) -> Result<Self, RecordingError> {
        let mut expected = io::Cursor::new(Vec::new());
        let offsets =
            write_headers(&mut expected).map_err(|error| recording_error(&partial_path, error))?;
        let mut header = vec![0; expected.get_ref().len()];
        file.read_exact(&mut header)
            .map_err(|error| recording_error(&partial_path, error))?;
        for offset in offsets.patched() {
            let offset = offset as usize;
            header[offset..offset + 4].fill(0);
        }
        if header != *expected.get_ref() {
            return Err(recording_error(
                &partial_path,
                "header was not written by this recorder",
            ));
        }
        Ok(Self::with_offsets(
            file,
            partial_path,
            final_path,
            u64::MAX,
            offsets,
        ))
    }

    fn with_offsets(
        file: File,
        partial_path: PathBuf,
        final_path: PathBuf,
        file_limit: u64,
        offsets: HeaderOffsets,
    ) -> Self {
        Self {
            file,
            partial_path,
            final_path,
            movi_list_start: offsets.movi_list_start,
            avih_frames_offset: offsets.avih_frames,
            avih_bytes_per_sec_offset: offsets.avih_bytes_per_sec,
            avih_buffer_offset: offsets.avih_buffer,
            video_length_offset: offsets.video_length,
            video_buffer_offset: offsets.video_buffer,
            video_image_size_offset: offsets.video_image_size,
            audio_length_offset: offsets.audio_length,
            audio_buffer_offset: offsets.audio_buffer,
            max_video_payload: 0,
            max_audio_payload: 0,
            file_limit,
            index: Vec::new(),
            video_frames: 0,
            audio_frames: 0,
            write_failed: None,
        }
    }

    pub(crate) fn movi_data_start(&self) -> u64 {
        self.movi_list_start + 12
    }

    pub(crate) fn file_mut(&mut self) -> &mut File {
        &mut self.file
    }

    pub(crate) fn write_unit(&mut self, frame: &[u8], audio: &[i16]) -> Result<bool, String> {
        let audio_bytes = audio.len().saturating_mul(2);
        let projected_index =
            8_u64.saturating_add((self.index.len() as u64 + 2).saturating_mul(16));
        let projected = self
            .file
            .stream_position()
            .unwrap_or(u64::MAX)
            .saturating_add(chunk_size(frame.len()))
            .saturating_add(chunk_size(audio_bytes))
            .saturating_add(projected_index);
        if projected > self.file_limit || projected.saturating_sub(8) > u64::from(u32::MAX) {
            return Ok(false);
        }
        self.write_chunk(*b"00dc", frame)
            .map_err(|error| error.to_string())?;
        self.write_audio(audio).map_err(|error| error.to_string())?;
        Ok(true)
    }

    fn write_audio(&mut self, samples: &[i16]) -> io::Result<()> {
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        self.write_chunk(*b"01wb", &bytes)
    }

    fn write_chunk(&mut self, id: [u8; 4], payload: &[u8]) -> io::Result<()> {
        let offset = self.file.stream_position()? - (self.movi_list_start + 8);
        let offset = u32::try_from(offset).map_err(|_| io::Error::other("AVI index overflow"))?;
        let size =
            u32::try_from(payload.len()).map_err(|_| io::Error::other("AVI chunk overflow"))?;
        self.file.write_all(&id)?;
        self.file.write_all(&size.to_le_bytes())?;
        self.file.write_all(payload)?;
        if !payload.len().is_multiple_of(2) {
            self.file.write_all(&[0])?;
        }
        self.record_chunk(id, offset, size);
        Ok(())
    }

    /// Indexes a movi chunk; `offset` counts from the `movi` fourcc, as idx1 expects.
    pub(crate) fn record_chunk(&mut self, id: [u8; 4], offset: u32, size: u32) {
        let flags = if id == *b"00dc" { 0x10 } else { 0 };
        self.index.push(IndexEntry {
            id,
            flags,
            offset,
            size,
        });
        if id == *b"00dc" {
            self.max_video_payload = self.max_video_payload.max(u64::from(size));
        } else if id == *b"01wb" {
            self.max_audio_payload = self.max_audio_payload.max(u64::from(size));
        }
        self.video_frames += u64::from(id == *b"00dc");
        self.audio_frames += if id == *b"01wb" {
            u64::from(size) / u64::from(CHANNELS * BITS_PER_SAMPLE / 8)
        } else {
            0
        };
    }

    pub(crate) fn fail(&mut self, error: String) {
        self.write_failed = Some(error);
    }

    pub(crate) fn partial_path(&self) -> &Path {
        &self.partial_path
    }

    pub(crate) fn finish(mut self, stats: FinishStats) -> Result<RecordingOutcome, RecordingError> {
        if let Some(error) = self.write_failed.take() {
            return Err(recording_error(&self.partial_path, error));
        }
        let movi_size = self
            .file
            .stream_position()
            .map_err(|error| recording_error(&self.partial_path, error))?
            - (self.movi_list_start + 8);
        patch_u32(&mut self.file, self.movi_list_start + 4, movi_size as u32)
            .map_err(|error| recording_error(&self.partial_path, error))?;
        self.file
            .seek(SeekFrom::End(0))
            .map_err(|error| recording_error(&self.partial_path, error))?;
        let index_payload = self.index.len().saturating_mul(16);
        write_chunk_header(&mut self.file, *b"idx1", index_payload as u32)
            .map_err(|error| recording_error(&self.partial_path, error))?;
        for entry in &self.index {
            self.file
                .write_all(&entry.id)
                .map_err(|error| recording_error(&self.partial_path, error))?;
            self.file
                .write_all(&entry.flags.to_le_bytes())
                .map_err(|error| recording_error(&self.partial_path, error))?;
            self.file
                .write_all(&entry.offset.to_le_bytes())
                .map_err(|error| recording_error(&self.partial_path, error))?;
            self.file
                .write_all(&entry.size.to_le_bytes())
                .map_err(|error| recording_error(&self.partial_path, error))?;
        }
        if !index_payload.is_multiple_of(2) {
            self.file
                .write_all(&[0])
                .map_err(|error| recording_error(&self.partial_path, error))?;
        }
        let max_buffer = self.max_buffer() as u32;
        let max_bytes_per_sec = self
            .max_video_payload
            .saturating_mul(u64::from(VIDEO_FPS))
            .saturating_add(176_400) as u32;
        patch_u32(
            &mut self.file,
            self.avih_frames_offset,
            self.video_frames as u32,
        )
        .map_err(|error| recording_error(&self.partial_path, error))?;
        patch_u32(
            &mut self.file,
            self.avih_bytes_per_sec_offset,
            max_bytes_per_sec,
        )
        .map_err(|error| recording_error(&self.partial_path, error))?;
        patch_u32(&mut self.file, self.avih_buffer_offset, max_buffer)
            .map_err(|error| recording_error(&self.partial_path, error))?;
        patch_u32(
            &mut self.file,
            self.video_length_offset,
            self.video_frames as u32,
        )
        .map_err(|error| recording_error(&self.partial_path, error))?;
        patch_u32(&mut self.file, self.video_buffer_offset, max_buffer)
            .map_err(|error| recording_error(&self.partial_path, error))?;
        patch_u32(
            &mut self.file,
            self.video_image_size_offset,
            self.max_video_payload as u32,
        )
        .map_err(|error| recording_error(&self.partial_path, error))?;
        let audio_sample_length = self.audio_sample_length() as u32;
        patch_u32(
            &mut self.file,
            self.audio_length_offset,
            audio_sample_length,
        )
        .map_err(|error| recording_error(&self.partial_path, error))?;
        patch_u32(&mut self.file, self.audio_buffer_offset, max_buffer)
            .map_err(|error| recording_error(&self.partial_path, error))?;
        let file_size = self
            .file
            .seek(SeekFrom::End(0))
            .map_err(|error| recording_error(&self.partial_path, error))?;
        patch_u32(&mut self.file, 4, u32::try_from(file_size - 8).unwrap())
            .map_err(|error| recording_error(&self.partial_path, error))?;
        self.file
            .flush()
            .map_err(|error| recording_error(&self.partial_path, error))?;
        self.file
            .sync_all()
            .map_err(|error| recording_error(&self.partial_path, error))?;
        let final_path = if stats.incomplete {
            self.final_path.with_extension("incomplete.avi")
        } else {
            self.final_path.clone()
        };
        if final_path.exists() {
            return Err(recording_error(
                &self.partial_path,
                format!(
                    "final recording path already exists: {}",
                    final_path.display()
                ),
            ));
        }
        fs::rename(&self.partial_path, &final_path)
            .map_err(|error| recording_error(&self.partial_path, error))?;
        let status = if !stats.incomplete {
            RecordingStatus::Complete
        } else {
            RecordingStatus::Incomplete {
                gap_count: stats.gap_count,
                gap_frames: stats.gap_frames,
                overflow_count: stats.overflow_count,
                overflow_frames: stats.overflow_frames,
            }
        };
        Ok(RecordingOutcome {
            path: final_path,
            frames_written: stats.frames,
            status,
            size_limited: stats.size_limited,
        })
    }

    fn max_buffer(&self) -> u64 {
        self.max_video_payload.max(self.max_audio_payload)
    }

    fn audio_sample_length(&self) -> u64 {
        self.audio_frames
    }
}

fn chunk_size(payload: usize) -> u64 {
    8 + payload as u64 + (payload % 2) as u64
}

fn recording_error(partial_path: &Path, error: impl std::fmt::Display) -> RecordingError {
    RecordingError {
        message: format!(
            "audio+video recording failed for {}: {error}",
            partial_path.display()
        ),
        partial_path: partial_path.to_path_buf(),
    }
}

mod avi_header;
use avi_header::{patch_u32, write_chunk_header, write_headers, HeaderOffsets};
