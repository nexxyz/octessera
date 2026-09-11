use super::{
    RecordingError, RecordingOutcome, RecordingStatus, BITS_PER_SAMPLE, CHANNELS, OLED_HEIGHT,
    OLED_WIDTH, SAMPLE_RATE,
};
use std::fs::{self, File};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub(crate) struct FinishStats {
    pub(crate) frames: u64,
    pub(crate) gap_count: u64,
    pub(crate) gap_frames: u64,
    pub(crate) overflow_count: u64,
    pub(crate) overflow_frames: u64,
    pub(crate) material_loss: u64,
    pub(crate) incomplete: bool,
}

struct HeaderOffsets {
    avih_frames: u64,
    avih_bytes_per_sec: u64,
    avih_buffer: u64,
    video_length: u64,
    video_buffer: u64,
    video_image_size: u64,
    audio_length: u64,
    audio_buffer: u64,
    movi_list_start: u64,
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
        Ok(Self {
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
        })
    }

    pub(crate) fn write_unit(&mut self, jpeg: &[u8], audio: &[i16]) -> Result<bool, String> {
        let audio_bytes = audio.len().saturating_mul(2);
        let projected_index =
            8_u64.saturating_add((self.index.len() as u64 + 2).saturating_mul(16));
        let projected = self
            .file
            .stream_position()
            .unwrap_or(u64::MAX)
            .saturating_add(chunk_size(jpeg.len()))
            .saturating_add(chunk_size(audio_bytes))
            .saturating_add(projected_index);
        if projected > self.file_limit || projected.saturating_sub(8) > u64::from(u32::MAX) {
            return Ok(false);
        }
        self.write_chunk(*b"00dc", jpeg, 0x10)
            .map_err(|error| error.to_string())?;
        self.write_audio(audio).map_err(|error| error.to_string())?;
        Ok(true)
    }

    fn write_audio(&mut self, samples: &[i16]) -> io::Result<()> {
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        self.write_chunk(*b"01wb", &bytes, 0)
    }

    fn write_chunk(&mut self, id: [u8; 4], payload: &[u8], flags: u32) -> io::Result<()> {
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
        Ok(())
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
            .saturating_mul(10)
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
                overflow_count: stats.overflow_count.saturating_add(stats.material_loss),
                overflow_frames: stats.overflow_frames.saturating_add(stats.material_loss),
            }
        };
        Ok(RecordingOutcome {
            path: final_path,
            frames_written: stats.frames,
            status,
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

fn write_headers(file: &mut File) -> io::Result<HeaderOffsets> {
    file.write_all(b"RIFF")?;
    file.write_all(&[0; 4])?;
    file.write_all(b"AVI ")?;
    let hdrl = begin_list(file, *b"hdrl")?;
    let avih = write_avih(file)?;
    let video = begin_list(file, *b"strl")?;
    let (video_length, video_buffer, video_image_size) = write_video_stream(file)?;
    end_list(file, video)?;
    let audio = begin_list(file, *b"strl")?;
    let (audio_length, audio_buffer) = write_audio_stream(file)?;
    end_list(file, audio)?;
    end_list(file, hdrl)?;
    let movi_list_start = file.stream_position()?;
    file.write_all(b"LIST")?;
    file.write_all(&[0; 4])?;
    file.write_all(b"movi")?;
    Ok(HeaderOffsets {
        avih_frames: avih.0,
        avih_bytes_per_sec: avih.1,
        avih_buffer: avih.2,
        video_length,
        video_buffer,
        video_image_size,
        audio_length,
        audio_buffer,
        movi_list_start,
    })
}

fn write_avih(file: &mut File) -> io::Result<(u64, u64, u64)> {
    write_chunk_header(file, *b"avih", 56)?;
    let start = file.stream_position()?;
    let mut data = [0_u8; 56];
    data[0..4].copy_from_slice(&100_000_u32.to_le_bytes());
    data[12..16].copy_from_slice(&0x110_u32.to_le_bytes());
    data[24..28].copy_from_slice(&2_u32.to_le_bytes());
    data[32..36].copy_from_slice(&OLED_WIDTH.to_le_bytes());
    data[36..40].copy_from_slice(&OLED_HEIGHT.to_le_bytes());
    file.write_all(&data)?;
    Ok((start + 16, start + 4, start + 28))
}

fn write_video_stream(file: &mut File) -> io::Result<(u64, u64, u64)> {
    write_chunk_header(file, *b"strh", 56)?;
    let start = file.stream_position()?;
    let mut data = [0_u8; 56];
    data[0..4].copy_from_slice(b"vids");
    data[4..8].copy_from_slice(b"MJPG");
    data[20..24].copy_from_slice(&1_u32.to_le_bytes());
    data[24..28].copy_from_slice(&10_u32.to_le_bytes());
    data[40..44].copy_from_slice(&0xffff_ffff_u32.to_le_bytes());
    data[52..54].copy_from_slice(&(OLED_WIDTH as u16).to_le_bytes());
    data[54..56].copy_from_slice(&(OLED_HEIGHT as u16).to_le_bytes());
    file.write_all(&data)?;
    write_chunk_header(file, *b"strf", 40)?;
    let start_format = file.stream_position()?;
    let mut format = [0_u8; 40];
    format[0..4].copy_from_slice(&40_u32.to_le_bytes());
    format[4..8].copy_from_slice(&OLED_WIDTH.to_le_bytes());
    format[8..12].copy_from_slice(&OLED_HEIGHT.to_le_bytes());
    format[12..14].copy_from_slice(&1_u16.to_le_bytes());
    format[14..16].copy_from_slice(&24_u16.to_le_bytes());
    format[16..20].copy_from_slice(b"MJPG");
    file.write_all(&format)?;
    Ok((start + 32, start + 36, start_format + 20))
}

fn write_audio_stream(file: &mut File) -> io::Result<(u64, u64)> {
    write_chunk_header(file, *b"strh", 56)?;
    let start = file.stream_position()?;
    let mut data = [0_u8; 56];
    data[0..4].copy_from_slice(b"auds");
    data[20..24].copy_from_slice(&1_u32.to_le_bytes());
    data[24..28].copy_from_slice(&SAMPLE_RATE.to_le_bytes());
    data[40..44].copy_from_slice(&0xffff_ffff_u32.to_le_bytes());
    data[44..48].copy_from_slice(&4_u32.to_le_bytes());
    file.write_all(&data)?;
    write_chunk_header(file, *b"strf", 16)?;
    let format = [
        1,
        0,
        CHANNELS as u8,
        0,
        0x44,
        0xac,
        0,
        0,
        0x10,
        0xb1,
        2,
        0,
        4,
        0,
        16,
        0,
    ];
    file.write_all(&format)?;
    Ok((start + 32, start + 36))
}

fn begin_list(file: &mut File, kind: [u8; 4]) -> io::Result<u64> {
    let start = file.stream_position()?;
    file.write_all(b"LIST")?;
    file.write_all(&[0; 4])?;
    file.write_all(&kind)?;
    Ok(start)
}

fn end_list(file: &mut File, start: u64) -> io::Result<()> {
    let end = file.stream_position()?;
    patch_u32(file, start + 4, u32::try_from(end - start - 8).unwrap())?;
    file.seek(SeekFrom::End(0))?;
    Ok(())
}

fn write_chunk_header(file: &mut File, id: [u8; 4], size: u32) -> io::Result<()> {
    file.write_all(&id)?;
    file.write_all(&size.to_le_bytes())
}

fn patch_u32(file: &mut File, offset: u64, value: u32) -> io::Result<()> {
    file.seek(SeekFrom::Start(offset))?;
    file.write_all(&value.to_le_bytes())
}

fn recording_error(partial_path: &Path, error: impl std::fmt::Display) -> RecordingError {
    RecordingError {
        message: format!(
            "audio+OLED recording failed for {}: {error}",
            partial_path.display()
        ),
        partial_path: partial_path.to_path_buf(),
    }
}
