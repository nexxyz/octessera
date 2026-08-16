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

fn normalize_wav_chunks(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut fmt_offset = None;
    let mut fmt_size = None;
    let mut block_align = None;
    let mut offset: usize = 12;
    while offset.checked_add(8)? <= bytes.len() {
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
        let data_start = offset + 8;
        let data_end = data_start.checked_add(size)?;
        let next = data_end.checked_add(size & 1)?;
        if next > bytes.len() {
            return None;
        }
        if &bytes[offset..offset + 4] == b"fmt " && size >= 16 {
            fmt_offset = Some(offset);
            fmt_size = Some(size);
            block_align = Some(u16::from_le_bytes(
                bytes[data_start + 12..data_start + 14].try_into().ok()?,
            ) as usize);
        }
        offset = next;
    }
    let fmt_offset = fmt_offset?;
    let fmt_size = fmt_size?;
    let block_align = block_align?;
    let mut offset: usize = 12;
    let mut changed = fmt_size > 16;
    while offset.checked_add(8)? <= bytes.len() {
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
        let data_start = offset + 8;
        let data_end = data_start.checked_add(size)?;
        let next = data_end.checked_add(size & 1)?;
        if &bytes[offset..offset + 4] == b"data"
            && block_align > 0
            && !size.is_multiple_of(block_align)
        {
            changed = true;
        }
        offset = next;
    }
    if !changed {
        return None;
    }

    let mut normalized = Vec::with_capacity(bytes.len());
    normalized.extend_from_slice(&bytes[..12]);
    let mut offset: usize = 12;
    while offset.checked_add(8)? <= bytes.len() {
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
        let data_start = offset + 8;
        let data_end = data_start.checked_add(size)?;
        let next = data_end.checked_add(size & 1)?;
        if offset == fmt_offset {
            normalized.extend_from_slice(b"fmt ");
            normalized.extend_from_slice(&16_u32.to_le_bytes());
            normalized.extend_from_slice(&bytes[data_start..data_start + 16]);
        } else if &bytes[offset..offset + 4] == b"data"
            && block_align > 0
            && !size.is_multiple_of(block_align)
        {
            let normalized_size = size - (size % block_align);
            normalized.extend_from_slice(b"data");
            normalized.extend_from_slice(&(normalized_size as u32).to_le_bytes());
            normalized.extend_from_slice(&bytes[data_start..data_start + normalized_size]);
            if normalized_size & 1 != 0 {
                normalized.push(0);
            }
        } else {
            normalized.extend_from_slice(&bytes[offset..next]);
        }
        offset = next;
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

        for line in include_str!("../../../samples/ATTRIBUTIONS.tsv")
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
