use realtime_engine::synth::SampleBuffer;
use std::io::Cursor;
use std::path::Path;

pub fn decode_sample_file(path: impl AsRef<Path>) -> Option<SampleBuffer> {
    let bytes = std::fs::read(path.as_ref()).ok()?;
    let mut reader = wav_reader(bytes.clone()).or_else(|| {
        let normalized = normalize_wav_chunks(&bytes)?;
        hound::WavReader::new(Cursor::new(normalized)).ok()
    })?;
    let spec = reader.spec();
    let channels = spec.channels;
    let sample_rate = spec.sample_rate;
    let samples = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .ok()?,
        hound::SampleFormat::Int => {
            if spec.bits_per_sample == 0 || spec.bits_per_sample > 32 {
                return None;
            }
            let scale = (1_u64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|sample| sample as f32 / scale))
                .collect::<Result<Vec<_>, _>>()
                .ok()?
        }
    };
    if channels == 0
        || sample_rate == 0
        || samples.is_empty()
        || samples.len() % usize::from(channels) != 0
        || samples.iter().any(|sample| !sample.is_finite())
    {
        return None;
    }
    Some(SampleBuffer {
        samples: samples.into(),
        channels,
        sample_rate,
    })
}

fn wav_reader(bytes: Vec<u8>) -> Option<hound::WavReader<Cursor<Vec<u8>>>> {
    hound::WavReader::new(Cursor::new(bytes)).ok()
}

struct RiffChunk<'a> {
    offset: usize,
    id: &'a [u8],
    data: &'a [u8],
    raw: &'a [u8],
}

struct RiffChunkIter<'a> {
    bytes: &'a [u8],
    offset: usize,
    finished: bool,
}

impl<'a> RiffChunkIter<'a> {
    fn malformed(&mut self) -> Option<Result<RiffChunk<'a>, RiffChunkError>> {
        self.finished = true;
        Some(Err(RiffChunkError))
    }
}

impl<'a> Iterator for RiffChunkIter<'a> {
    type Item = Result<RiffChunk<'a>, RiffChunkError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished || self.offset == self.bytes.len() {
            return None;
        }

        let offset = self.offset;
        let Some(header_end) = offset.checked_add(8) else {
            return self.malformed();
        };
        if header_end > self.bytes.len() {
            self.finished = true;
            return None;
        }
        let size = u32::from_le_bytes([
            self.bytes[header_end - 4],
            self.bytes[header_end - 3],
            self.bytes[header_end - 2],
            self.bytes[header_end - 1],
        ]) as usize;
        let Some(data_end) = header_end.checked_add(size) else {
            return self.malformed();
        };
        let Some(next) = data_end.checked_add(size & 1) else {
            return self.malformed();
        };
        if next > self.bytes.len() {
            return self.malformed();
        }

        self.offset = next;
        Some(Ok(RiffChunk {
            offset,
            id: &self.bytes[offset..header_end - 4],
            data: &self.bytes[header_end..data_end],
            raw: &self.bytes[offset..next],
        }))
    }
}

struct RiffChunkError;

fn riff_chunks(bytes: &[u8]) -> RiffChunkIter<'_> {
    RiffChunkIter {
        bytes,
        offset: 12,
        finished: false,
    }
}

fn normalize_wav_chunks(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut fmt_offset = None;
    let mut fmt_size = None;
    let mut block_align = None;
    for chunk in riff_chunks(bytes) {
        let chunk = chunk.ok()?;
        let size = chunk.data.len();
        if chunk.id == b"fmt " && size >= 16 {
            fmt_offset = Some(chunk.offset);
            fmt_size = Some(size);
            block_align = Some(u16::from_le_bytes(chunk.data[12..14].try_into().ok()?) as usize);
        }
    }
    let fmt_offset = fmt_offset?;
    let fmt_size = fmt_size?;
    let block_align = block_align?;
    let mut changed = fmt_size > 16;
    for chunk in riff_chunks(bytes) {
        let chunk = chunk.ok()?;
        let size = chunk.data.len();
        if chunk.id == b"data" && block_align > 0 && !size.is_multiple_of(block_align) {
            changed = true;
        }
    }
    if !changed {
        return None;
    }

    let mut normalized = Vec::with_capacity(bytes.len());
    normalized.extend_from_slice(&bytes[..12]);
    for chunk in riff_chunks(bytes) {
        let chunk = chunk.ok()?;
        let size = chunk.data.len();
        if chunk.offset == fmt_offset {
            normalized.extend_from_slice(b"fmt ");
            normalized.extend_from_slice(&16_u32.to_le_bytes());
            normalized.extend_from_slice(&chunk.data[..16]);
        } else if chunk.id == b"data" && block_align > 0 && !size.is_multiple_of(block_align) {
            let normalized_size = size - (size % block_align);
            normalized.extend_from_slice(b"data");
            normalized.extend_from_slice(&(normalized_size as u32).to_le_bytes());
            normalized.extend_from_slice(&chunk.data[..normalized_size]);
            if normalized_size & 1 != 0 {
                normalized.push(0);
            }
        } else {
            normalized.extend_from_slice(chunk.raw);
        }
    }
    let riff_size = (normalized.len() - 8) as u32;
    normalized[4..8].copy_from_slice(&riff_size.to_le_bytes());
    Some(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempFixture {
        directory: PathBuf,
    }

    impl TempFixture {
        fn new() -> Self {
            let directory = std::env::temp_dir().join(format!(
                "octessera-rodio-sample-decode-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&directory).unwrap();
            Self { directory }
        }

        fn path(&self, name: &str) -> PathBuf {
            self.directory.join(name)
        }

        fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.path(name);
            std::fs::write(&path, bytes).unwrap();
            path
        }
    }

    impl Drop for TempFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }

    fn wav_bytes(channels: u16, sample_rate: u32, samples: &[i16]) -> Vec<u8> {
        let data_length = std::mem::size_of_val(samples) as u32;
        let block_align = channels * std::mem::size_of::<i16>() as u16;
        let byte_rate = sample_rate * u32::from(block_align);
        let riff_length = 4 + 8 + 16 + 8 + data_length;
        let mut bytes = Vec::with_capacity((riff_length + 8) as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&riff_length.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&block_align.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_length.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }

    fn riff_with_chunks(chunks: &[(&[u8; 4], &[u8], u8)]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        for (id, data, pad) in chunks {
            bytes.extend_from_slice(*id);
            bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
            bytes.extend_from_slice(data);
            if data.len() & 1 != 0 {
                bytes.push(*pad);
            }
        }
        let riff_length = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&riff_length.to_le_bytes());
        bytes
    }

    #[test]
    fn trailing_short_chunk_header_is_ignored_by_normalization() {
        let mut bytes = wav_bytes(2, 44_100, &[0, 16_384, -16_384]);
        bytes.extend_from_slice(b"JUNK");

        assert!(normalize_wav_chunks(&bytes).is_some());
    }

    #[test]
    fn malformed_truncated_chunk_payload_fails_normalization() {
        let mut bytes = wav_bytes(2, 44_100, &[0]);
        bytes.extend_from_slice(b"JUNK");
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.push(0);

        assert!(normalize_wav_chunks(&bytes).is_none());
    }

    #[test]
    fn malformed_truncated_chunk_pad_fails_normalization() {
        let mut bytes = wav_bytes(2, 44_100, &[0]);
        bytes.extend_from_slice(b"JUNK");
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.push(0);

        assert!(normalize_wav_chunks(&bytes).is_none());
    }

    #[test]
    fn normalization_preserves_odd_intervening_and_trailing_chunks() {
        let base = wav_bytes(2, 44_100, &[0]);
        let mut extended_fmt = base[20..36].to_vec();
        extended_fmt.extend_from_slice(&[0x34, 0x12]);
        let bytes = riff_with_chunks(&[
            (b"fmt ", &extended_fmt, 0),
            (b"JUNK", b"abc", 0xa5),
            (b"data", &[1, 2, 3, 4, 5], 0xa6),
            (b"LIST", b"z", 0xa7),
        ]);

        let normalized = normalize_wav_chunks(&bytes).expect("normalization should be needed");
        let chunks = riff_chunks(&normalized)
            .map(|chunk| chunk.ok().expect("normalized chunks should be valid"))
            .collect::<Vec<_>>();

        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0].id, b"fmt ");
        assert_eq!(chunks[0].data, &extended_fmt[..16]);
        assert_eq!(chunks[1].id, b"JUNK");
        assert_eq!(chunks[1].data, b"abc");
        assert_eq!(chunks[1].raw.last(), Some(&0xa5));
        assert_eq!(chunks[2].id, b"data");
        assert_eq!(chunks[2].data, &[1, 2, 3, 4]);
        assert_eq!(chunks[3].id, b"LIST");
        assert_eq!(chunks[3].data, b"z");
        assert_eq!(chunks[3].raw.last(), Some(&0xa7));
    }

    #[test]
    fn aligned_chunks_need_no_normalization() {
        let bytes = wav_bytes(1, 44_100, &[0, 1]);

        assert!(normalize_wav_chunks(&bytes).is_none());
    }

    #[test]
    fn public_decoder_repairs_wav_with_short_trailing_suffixes() {
        let fixture = TempFixture::new();

        for suffix_len in 1..=7 {
            let mut bytes = wav_bytes(2, 44_100, &[0, 16_384, -16_384]);
            let suffix = [0xa5; 7];
            bytes.extend_from_slice(&suffix[..suffix_len]);
            let path = fixture.write(&format!("repairable-{suffix_len}.wav"), &bytes);

            let buffer = decode_sample_file(path).expect("repairable WAV should decode");

            assert_eq!(buffer.channels, 2);
            assert_eq!(buffer.sample_rate, 44_100);
            assert_eq!(buffer.samples.len(), 2);
        }
    }

    #[test]
    fn decodes_wav_preserving_channels_rate_and_data() {
        let fixture = TempFixture::new();
        let samples = [0, 16_384, -16_384, 32_767];
        let path = fixture.write("valid.wav", &wav_bytes(2, 22_050, &samples));

        let buffer = decode_sample_file(path).expect("valid WAV should decode");

        assert_eq!(buffer.channels, 2);
        assert_eq!(buffer.sample_rate, 22_050);
        assert_eq!(buffer.samples.len(), samples.len());
        let expected = [
            0.0,
            16_384.0 / 32_768.0,
            -16_384.0 / 32_768.0,
            32_767.0 / 32_768.0,
        ];
        const TOLERANCE: f32 = 1.0e-6;
        for (actual, expected) in buffer.samples.iter().zip(expected) {
            assert!((actual - expected).abs() <= TOLERANCE);
        }
    }

    #[test]
    fn missing_sample_file_fails_to_decode() {
        let fixture = TempFixture::new();

        assert!(decode_sample_file(fixture.path("missing.wav")).is_none());
    }

    #[test]
    fn malformed_sample_file_fails_to_decode() {
        let fixture = TempFixture::new();
        let path = fixture.write("malformed.wav", b"not a WAV file");

        assert!(decode_sample_file(path).is_none());
    }

    #[test]
    fn empty_sample_file_fails_to_decode() {
        let fixture = TempFixture::new();
        let path = fixture.write("empty.wav", &wav_bytes(1, 44_100, &[]));

        assert!(decode_sample_file(path).is_none());
    }

    #[test]
    fn default_library_wav_rows_decode_and_aiff_rows_remain_metadata_only() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples")
            .canonicalize()
            .expect("repository sample root");
        let mut playable_wav_count = 0;
        let mut metadata_only_count = 0;
        let mut metadata_only_paths = Vec::new();

        for line in include_str!("../../../samples/MANIFEST.tsv")
            .lines()
            .skip(1)
        {
            let relative = line.split('\t').next().expect("inventory path");
            let path = root.join(relative);
            let canonical = path.canonicalize().expect("inventory sample path");
            assert!(canonical.starts_with(&root));
            assert!(canonical.is_file());
            if relative.to_ascii_lowercase().ends_with(".wav") {
                playable_wav_count += 1;
                let buffer = decode_sample_file(&canonical).unwrap_or_else(|| {
                    panic!("playable inventory WAV does not decode: {relative}")
                });
                assert!(buffer.channels > 0);
                assert!(buffer.sample_rate > 0);
                assert!(!buffer.samples.is_empty());
                assert_eq!(buffer.samples.len() % usize::from(buffer.channels), 0);
                assert!(buffer.samples.iter().all(|sample| sample.is_finite()));
                assert!(buffer.samples.len() / usize::from(buffer.channels) > 0);
            } else {
                metadata_only_count += 1;
                metadata_only_paths.push(relative);
                assert!(relative.to_ascii_lowercase().ends_with(".aiff"));
                assert!(decode_sample_file(&canonical).is_none());
            }
        }

        assert_eq!(playable_wav_count, 318);
        assert_eq!(metadata_only_count, 2);
        assert_eq!(
            metadata_only_paths,
            vec![
                "Drum/hihat closed/132415__sajmund__hi-hat-hit.aiff",
                "Drum/rimshot/132418__sajmund__rimshot-sweet.aiff",
            ]
        );
    }
}
