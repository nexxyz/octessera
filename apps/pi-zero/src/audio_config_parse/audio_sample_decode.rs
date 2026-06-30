use realtime_engine::synth::SampleBuffer;
use rodio::Source;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

pub(super) fn decode_sample_file(path: &Path) -> Option<SampleBuffer> {
    let file = File::open(path).ok()?;
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
