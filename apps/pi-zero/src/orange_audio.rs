use crate::audio_route::RouteOpenError;
use cpal::traits::DeviceTrait;
use cpal::{SampleFormat, StreamConfig};
use realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;

pub(crate) const ORANGE_AUDIO_DEVICE_NAME: &str = cpal::ALSA_ORANGE_JACK_PCM;
pub(crate) const ORANGE_UAC2_AUDIO_DEVICE_NAME: &str = cpal::ALSA_ORANGE_USB_PCM;
pub(crate) const ORANGE_HDMI_AUDIO_DEVICE_NAME: &str = cpal::ALSA_ORANGE_HDMI_PCM;
pub(crate) const ORANGE_AUDIO_CHANNELS: u16 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OrangeOutputConfigCandidate {
    pub(crate) channels: u16,
    pub(crate) min_sample_rate: u32,
    pub(crate) max_sample_rate: u32,
    pub(crate) sample_format: SampleFormat,
}

pub(crate) fn select_orange_output_config(
    candidates: &[OrangeOutputConfigCandidate],
) -> Result<usize, String> {
    candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let format_rank = orange_sample_format_rank(candidate.sample_format)?;
            (candidate.channels == ORANGE_AUDIO_CHANNELS
                && candidate.min_sample_rate <= DEFAULT_AUDIO_SAMPLE_RATE
                && candidate.max_sample_rate >= DEFAULT_AUDIO_SAMPLE_RATE)
                .then_some((
                    format_rank,
                    candidate.min_sample_rate,
                    candidate.max_sample_rate,
                    index,
                ))
        })
        .min_by_key(|candidate| (candidate.0, candidate.1, candidate.2, candidate.3))
        .map(|candidate| candidate.3)
        .ok_or_else(|| {
            format!(
                "Orange audio device does not support {} Hz stereo output",
                DEFAULT_AUDIO_SAMPLE_RATE
            )
        })
}

pub(crate) fn select_orange_output_device() -> Result<cpal::Device, RouteOpenError> {
    select_orange_named_output_device(ORANGE_AUDIO_DEVICE_NAME)
}

pub(crate) fn select_orange_uac2_output_device() -> Result<cpal::Device, RouteOpenError> {
    select_orange_named_output_device(ORANGE_UAC2_AUDIO_DEVICE_NAME)
}

pub(crate) fn select_orange_hdmi_output_device() -> Result<cpal::Device, RouteOpenError> {
    select_orange_named_output_device(ORANGE_HDMI_AUDIO_DEVICE_NAME)
}

fn select_orange_named_output_device(expected_name: &str) -> Result<cpal::Device, RouteOpenError> {
    cpal::alsa_exact_output_device(expected_name).map_err(|error| {
        RouteOpenError::Fault(format!(
            "failed to construct exact Orange ALSA PCM {expected_name:?}: {error}"
        ))
    })
}

#[cfg(test)]
fn open_exact_orange_output_device<T>(
    expected_name: &str,
    opener: impl FnOnce(&str) -> Result<T, String>,
) -> Result<T, String> {
    opener(expected_name)
}

pub(crate) fn select_orange_stream_config(
    device: &cpal::Device,
) -> Result<(SampleFormat, StreamConfig), RouteOpenError> {
    let ranges: Vec<_> = device
        .supported_output_configs()
        .map_err(map_supported_configs_error)?
        .collect();
    let candidates: Vec<_> = ranges
        .iter()
        .map(|range| OrangeOutputConfigCandidate {
            channels: range.channels(),
            min_sample_rate: range.min_sample_rate().0,
            max_sample_rate: range.max_sample_rate().0,
            sample_format: range.sample_format(),
        })
        .collect();
    let index = select_orange_output_config(&candidates).map_err(RouteOpenError::Unsupported)?;
    let supported = ranges.into_iter().nth(index).ok_or_else(|| {
        RouteOpenError::Fault("Orange audio config selection became inconsistent".into())
    })?;
    let sample_format = supported.sample_format();
    let config = supported
        .with_sample_rate(cpal::SampleRate(DEFAULT_AUDIO_SAMPLE_RATE))
        .config();
    Ok((sample_format, config))
}

fn map_supported_configs_error(error: cpal::SupportedStreamConfigsError) -> RouteOpenError {
    match error {
        cpal::SupportedStreamConfigsError::DeviceNotAvailable => RouteOpenError::Disconnected,
        cpal::SupportedStreamConfigsError::DeviceBusy => RouteOpenError::Busy,
        cpal::SupportedStreamConfigsError::InvalidArgument => {
            RouteOpenError::Unsupported(error.to_string())
        }
        cpal::SupportedStreamConfigsError::BackendSpecific { .. } => {
            RouteOpenError::Fault(error.to_string())
        }
    }
}

fn orange_sample_format_rank(sample_format: SampleFormat) -> Option<u8> {
    match sample_format {
        SampleFormat::F32 => Some(0),
        SampleFormat::I16 => Some(1),
        SampleFormat::U16 => Some(2),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orange_selection_requests_exact_pcm_ids_without_enumeration() {
        let mut requested = Vec::<String>::new();
        for expected_name in [
            ORANGE_AUDIO_DEVICE_NAME,
            ORANGE_UAC2_AUDIO_DEVICE_NAME,
            ORANGE_HDMI_AUDIO_DEVICE_NAME,
        ] {
            open_exact_orange_output_device(expected_name, |name| {
                requested.push(name.to_owned());
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(
            requested,
            vec![
                ORANGE_AUDIO_DEVICE_NAME.to_owned(),
                ORANGE_UAC2_AUDIO_DEVICE_NAME.to_owned(),
                ORANGE_HDMI_AUDIO_DEVICE_NAME.to_owned(),
            ]
        );
    }

    #[test]
    fn orange_config_selection_requires_shared_default_stereo_rate() {
        let below_default = DEFAULT_AUDIO_SAMPLE_RATE - 1;
        assert_eq!(
            select_orange_output_config(&[
                OrangeOutputConfigCandidate {
                    channels: 2,
                    min_sample_rate: below_default,
                    max_sample_rate: DEFAULT_AUDIO_SAMPLE_RATE,
                    sample_format: SampleFormat::I32,
                },
                OrangeOutputConfigCandidate {
                    channels: 1,
                    min_sample_rate: DEFAULT_AUDIO_SAMPLE_RATE,
                    max_sample_rate: DEFAULT_AUDIO_SAMPLE_RATE,
                    sample_format: SampleFormat::F32,
                },
                OrangeOutputConfigCandidate {
                    channels: 2,
                    min_sample_rate: below_default,
                    max_sample_rate: DEFAULT_AUDIO_SAMPLE_RATE,
                    sample_format: SampleFormat::I16,
                },
                OrangeOutputConfigCandidate {
                    channels: 2,
                    min_sample_rate: below_default,
                    max_sample_rate: DEFAULT_AUDIO_SAMPLE_RATE,
                    sample_format: SampleFormat::F32,
                },
            ]),
            Ok(3)
        );
        assert!(select_orange_output_config(&[OrangeOutputConfigCandidate {
            channels: 2,
            min_sample_rate: 0,
            max_sample_rate: below_default,
            sample_format: SampleFormat::F32,
        }])
        .is_err());
    }
}
