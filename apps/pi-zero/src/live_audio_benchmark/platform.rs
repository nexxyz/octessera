#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
use cpal::traits::DeviceTrait;
use cpal::{Device, SampleFormat, StreamConfig};
#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
use realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) const BENCHMARK_ARGUMENT: &str = "--benchmark-orange-audio";
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) const BENCHMARK_LABEL: &str = "Orange";
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) const BENCHMARK_RESULT_KIND: &str = "orange_audio_benchmark_result";
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) const BENCHMARK_PROGRESS_KIND: &str = "orange_audio_benchmark_progress";
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) const BENCHMARK_READINESS_KIND: &str = "orange_audio_benchmark_readiness";
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) const BENCHMARK_RELEASE_KIND: &str = "orange_audio_benchmark_release";
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) const DEFAULT_RESULT_PATH: &str = "/run/octessera/orange-audio-benchmark-result.json";
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) const DEFAULT_PROGRESS_PATH: &str =
    "/run/octessera/orange-audio-benchmark-progress.json";
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) const DEFAULT_READINESS_PATH: &str =
    "/run/octessera/orange-audio-benchmark-readiness.json";

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
pub(super) const BENCHMARK_ARGUMENT: &str = "--benchmark-raspberry-audio";
#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
pub(super) const BENCHMARK_LABEL: &str = "Raspberry";
#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
pub(super) const BENCHMARK_RESULT_KIND: &str = "raspberry_audio_benchmark_result";
#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
pub(super) const BENCHMARK_PROGRESS_KIND: &str = "raspberry_audio_benchmark_progress";
#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
pub(super) const BENCHMARK_READINESS_KIND: &str = "raspberry_audio_benchmark_readiness";
#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
pub(super) const BENCHMARK_RELEASE_KIND: &str = "raspberry_audio_benchmark_release";
#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
pub(super) const DEFAULT_RESULT_PATH: &str = "/run/octessera/raspberry-audio-benchmark-result.json";
#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
pub(super) const DEFAULT_PROGRESS_PATH: &str =
    "/run/octessera/raspberry-audio-benchmark-progress.json";
#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
pub(super) const DEFAULT_READINESS_PATH: &str =
    "/run/octessera/raspberry-audio-benchmark-readiness.json";

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) fn select_output_device() -> Result<Device, String> {
    crate::orange_audio::select_orange_output_device().map_err(|error| error.to_string())
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) fn select_stream_config(
    device: &Device,
) -> Result<(SampleFormat, StreamConfig), String> {
    crate::orange_audio::select_orange_stream_config(device).map_err(|error| error.to_string())
}

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
pub(super) fn select_output_device() -> Result<Device, String> {
    cpal::alsa_exact_output_device(cpal::ALSA_RASPBERRY_JACK_PCM)
        .map_err(|error| format!("failed to construct exact Raspberry ALSA PCM: {error}"))
}

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
    not(feature = "legacy-hardware-rpi-zero-2w"),
    not(feature = "legacy-hardware-pi")
))]
pub(super) fn select_stream_config(
    device: &Device,
) -> Result<(SampleFormat, StreamConfig), String> {
    let supported = device
        .default_output_config()
        .map_err(|error| format!("failed to read Raspberry output config: {error}"))?;
    if supported.channels() != 2 || supported.sample_rate().0 != DEFAULT_AUDIO_SAMPLE_RATE {
        return Err(format!(
            "Raspberry audio device lacks project stereo {} Hz output",
            DEFAULT_AUDIO_SAMPLE_RATE
        ));
    }
    let sample_format = supported.sample_format();
    if !matches!(
        sample_format,
        SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16
    ) {
        return Err(format!(
            "unsupported Raspberry benchmark sample format: {sample_format:?}"
        ));
    }
    Ok((sample_format, supported.config()))
}
