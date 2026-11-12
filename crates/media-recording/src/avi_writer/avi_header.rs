//! The RIFF/AVI header layout this recorder writes, with the offsets `finish` patches.

use super::super::{CHANNELS, SAMPLE_RATE, VIDEO_FPS, VIDEO_HEIGHT, VIDEO_WIDTH};
use std::io::{self, Seek, SeekFrom, Write};

pub(super) struct HeaderOffsets {
    pub(super) avih_frames: u64,
    pub(super) avih_bytes_per_sec: u64,
    pub(super) avih_buffer: u64,
    pub(super) video_length: u64,
    pub(super) video_buffer: u64,
    pub(super) video_image_size: u64,
    pub(super) audio_length: u64,
    pub(super) audio_buffer: u64,
    pub(super) movi_list_start: u64,
}

impl HeaderOffsets {
    /// Where `finish` writes the sizes and counts; zero until then.
    pub(super) fn patched(&self) -> [u64; 10] {
        [
            4,
            self.movi_list_start + 4,
            self.avih_frames,
            self.avih_bytes_per_sec,
            self.avih_buffer,
            self.video_length,
            self.video_buffer,
            self.video_image_size,
            self.audio_length,
            self.audio_buffer,
        ]
    }
}

pub(super) fn write_headers<W: Write + Seek>(file: &mut W) -> io::Result<HeaderOffsets> {
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

fn write_avih<W: Write + Seek>(file: &mut W) -> io::Result<(u64, u64, u64)> {
    write_chunk_header(file, *b"avih", 56)?;
    let start = file.stream_position()?;
    let mut data = [0_u8; 56];
    data[0..4].copy_from_slice(&(1_000_000 / VIDEO_FPS).to_le_bytes());
    data[12..16].copy_from_slice(&0x110_u32.to_le_bytes());
    data[24..28].copy_from_slice(&2_u32.to_le_bytes());
    data[32..36].copy_from_slice(&VIDEO_WIDTH.to_le_bytes());
    data[36..40].copy_from_slice(&VIDEO_HEIGHT.to_le_bytes());
    file.write_all(&data)?;
    Ok((start + 16, start + 4, start + 28))
}

fn write_video_stream<W: Write + Seek>(file: &mut W) -> io::Result<(u64, u64, u64)> {
    write_chunk_header(file, *b"strh", 56)?;
    let start = file.stream_position()?;
    let mut data = [0_u8; 56];
    data[0..4].copy_from_slice(b"vids");
    data[4..8].copy_from_slice(b"MPNG");
    data[20..24].copy_from_slice(&1_u32.to_le_bytes());
    data[24..28].copy_from_slice(&VIDEO_FPS.to_le_bytes());
    data[40..44].copy_from_slice(&0xffff_ffff_u32.to_le_bytes());
    data[52..54].copy_from_slice(&(VIDEO_WIDTH as u16).to_le_bytes());
    data[54..56].copy_from_slice(&(VIDEO_HEIGHT as u16).to_le_bytes());
    file.write_all(&data)?;
    write_chunk_header(file, *b"strf", 40)?;
    let start_format = file.stream_position()?;
    let mut format = [0_u8; 40];
    format[0..4].copy_from_slice(&40_u32.to_le_bytes());
    format[4..8].copy_from_slice(&VIDEO_WIDTH.to_le_bytes());
    format[8..12].copy_from_slice(&VIDEO_HEIGHT.to_le_bytes());
    format[12..14].copy_from_slice(&1_u16.to_le_bytes());
    format[14..16].copy_from_slice(&24_u16.to_le_bytes());
    format[16..20].copy_from_slice(b"MPNG");
    file.write_all(&format)?;
    Ok((start + 32, start + 36, start_format + 20))
}

fn write_audio_stream<W: Write + Seek>(file: &mut W) -> io::Result<(u64, u64)> {
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

fn begin_list<W: Write + Seek>(file: &mut W, kind: [u8; 4]) -> io::Result<u64> {
    let start = file.stream_position()?;
    file.write_all(b"LIST")?;
    file.write_all(&[0; 4])?;
    file.write_all(&kind)?;
    Ok(start)
}

fn end_list<W: Write + Seek>(file: &mut W, start: u64) -> io::Result<()> {
    let end = file.stream_position()?;
    patch_u32(file, start + 4, u32::try_from(end - start - 8).unwrap())?;
    file.seek(SeekFrom::End(0))?;
    Ok(())
}

pub(super) fn write_chunk_header<W: Write + Seek>(
    file: &mut W,
    id: [u8; 4],
    size: u32,
) -> io::Result<()> {
    file.write_all(&id)?;
    file.write_all(&size.to_le_bytes())
}

pub(super) fn patch_u32<W: Write + Seek>(file: &mut W, offset: u64, value: u32) -> io::Result<()> {
    file.seek(SeekFrom::Start(offset))?;
    file.write_all(&value.to_le_bytes())
}
