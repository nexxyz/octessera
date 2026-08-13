use realtime_engine::synth::SampleBuffer;
use rodio::Source;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub fn decode_sample_file(path: impl AsRef<Path>) -> Option<SampleBuffer> {
    let file = File::open(path.as_ref()).ok()?;
    let decoder = rodio::Decoder::new(BufReader::new(file)).ok()?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let samples = decoder.convert_samples::<f32>().collect::<Vec<_>>();
    if samples.is_empty() {
        return None;
    }
    Some(SampleBuffer {
        samples: samples.into(),
        channels,
        sample_rate,
    })
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
}
