use crate::audio::AudioService;
use crate::audio_event::musical_event_to_engine_event;
use crate::host_audio_command::send_audio_command;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{SampleFormat, StreamConfig};
#[cfg(test)]
use playback_runtime::RuntimeErrorCode;
use playback_runtime::{
    HostAdapter, HostMessage, MusicalEvent, RuntimeAdapterError, RuntimeAudioCommand,
    RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use realtime_engine::synth::DEFAULT_AUDIO_SAMPLE_RATE;
use rodio_engine_source::EngineEvent;
use std::path::PathBuf;

pub(crate) const ORANGE_AUDIO_DEVICE_NAME: &str = "hw:CARD=octesseradac,DEV=0";
pub(crate) const ORANGE_AUDIO_CHANNELS: u16 = 2;
pub(crate) const ORANGE_UNAVAILABLE_STATUS: &str =
    "unavailable in Orange foreground runtime-candidate";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OrangeOutputConfigCandidate {
    pub(crate) channels: u16,
    pub(crate) min_sample_rate: u32,
    pub(crate) max_sample_rate: u32,
    pub(crate) sample_format: SampleFormat,
}

pub(crate) fn select_orange_device_index(
    names: &[&str],
    expected_name: &str,
) -> Result<usize, String> {
    let available = format_output_device_names(names);
    let matches: Vec<_> = names
        .iter()
        .enumerate()
        .filter(|(_, name)| **name == expected_name)
        .map(|(index, _)| index)
        .collect();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(format!(
            "Orange audio requires exactly one output device named {expected_name:?}; found none; available output devices: {available}"
        )),
        _ => Err(format!(
            "Orange audio requires exactly one output device named {expected_name:?}; found {}; available output devices: {available}",
            matches.len(),
        )),
    }
}

fn format_output_device_names(names: &[&str]) -> String {
    if names.is_empty() {
        return "[]".into();
    }
    let entries = names
        .iter()
        .enumerate()
        .map(|(index, name)| format!("{index}: {name:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{entries}]")
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

pub(crate) fn select_orange_output_device(host: &cpal::Host) -> Result<cpal::Device, String> {
    let devices: Vec<_> = host
        .output_devices()
        .map_err(|e| format!("failed to enumerate Orange audio output devices: {e}"))?
        .collect();
    let names: Vec<_> = devices
        .iter()
        .map(|device| device.name().unwrap_or_default())
        .collect();
    let names: Vec<_> = names.iter().map(String::as_str).collect();
    let index = select_orange_device_index(&names, ORANGE_AUDIO_DEVICE_NAME)?;
    devices
        .into_iter()
        .nth(index)
        .ok_or_else(|| "Orange audio device selection became inconsistent".into())
}

pub(crate) fn select_orange_stream_config(
    device: &cpal::Device,
) -> Result<(SampleFormat, StreamConfig), String> {
    let ranges: Vec<_> = device
        .supported_output_configs()
        .map_err(|e| format!("failed to read Orange audio device capabilities: {e}"))?
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
    let index = select_orange_output_config(&candidates)?;
    let supported = ranges
        .into_iter()
        .nth(index)
        .ok_or_else(|| "Orange audio config selection became inconsistent".to_string())?;
    let sample_format = supported.sample_format();
    let config = supported
        .with_sample_rate(cpal::SampleRate(DEFAULT_AUDIO_SAMPLE_RATE))
        .config();
    Ok((sample_format, config))
}

pub(crate) struct OrangeAudioHost {
    audio: AudioService,
    samples_dir: PathBuf,
}

impl OrangeAudioHost {
    pub(crate) fn new(audio: AudioService, samples_dir: PathBuf) -> Self {
        Self { audio, samples_dir }
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

impl HostAdapter for OrangeAudioHost {
    fn handle_musical_event(&mut self, event: &MusicalEvent) -> Result<(), RuntimeAdapterError> {
        self.audio
            .send_realtime(musical_event_to_engine_event(event))
    }

    fn handle_platform_effect(
        &mut self,
        request: &RuntimePlatformRequest,
    ) -> Result<Vec<HostMessage>, RuntimeAdapterError> {
        if let RuntimePlatformEffect::AudioCommand { command } = &request.effect {
            self.handle_audio_command(command)?;
            return Ok(Vec::new());
        }
        Ok(vec![HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure {
                error: request.unsupported_facts(ORANGE_UNAVAILABLE_STATUS.into()),
            },
        }])
    }

    fn handle_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        send_audio_command(Some(self.audio.clone()), command, &self.samples_dir)
    }

    fn handle_midi_message(&mut self, _bytes: &[u8]) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), RuntimeAdapterError> {
        self.audio.send_realtime(EngineEvent::AllNotesOff)
    }

    fn panic_external_midi(&mut self) -> Result<(), RuntimeAdapterError> {
        Ok(())
    }
}

pub(crate) fn orange_samples_dir() -> Result<PathBuf, String> {
    validate_orange_samples_dir(crate::main_paths::default_samples_dir())
}

fn validate_orange_samples_dir(configured: PathBuf) -> Result<PathBuf, String> {
    std::fs::create_dir_all(&configured).map_err(|error| {
        format!(
            "Orange sample directory is not usable at {}: {error}",
            configured.display()
        )
    })?;
    let canonical = configured.canonicalize().map_err(|error| {
        format!(
            "Orange sample directory cannot be resolved at {}: {error}",
            configured.display()
        )
    })?;
    canonical.is_dir().then_some(canonical).ok_or_else(|| {
        format!(
            "Orange sample path is not a directory: {}",
            configured.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{test_service, AudioControlRequest};
    use playback_runtime::RuntimeAudioCommand;

    #[test]
    fn orange_device_selection_requires_one_exact_named_match() {
        let names = [
            "hw:CARD=Codec,DEV=0",
            "hw:CARD=octesseradac,DEV=0",
            "hw:CARD=HDMI,DEV=0",
        ];
        assert_eq!(
            select_orange_device_index(&names, ORANGE_AUDIO_DEVICE_NAME),
            Ok(1)
        );
        let missing = select_orange_device_index(
            &["hw:CARD=Codec,DEV=0", "hw:CARD=HDMI,DEV=0"],
            ORANGE_AUDIO_DEVICE_NAME,
        )
        .unwrap_err();
        assert!(missing.contains(
            "available output devices: [0: \"hw:CARD=Codec,DEV=0\", 1: \"hw:CARD=HDMI,DEV=0\"]"
        ));

        let near_match = select_orange_device_index(
            &[
                "hw:CARD=Codec,DEV=0",
                "hw:CARD=HDMI,DEV=0",
                "hw:CARD=octesseradac,DEV=1",
            ],
            ORANGE_AUDIO_DEVICE_NAME,
        )
        .unwrap_err();
        assert!(near_match.contains("2: \"hw:CARD=octesseradac,DEV=1\""));

        let duplicate = select_orange_device_index(
            &["hw:CARD=octesseradac,DEV=0", "hw:CARD=octesseradac,DEV=0"],
            ORANGE_AUDIO_DEVICE_NAME,
        )
        .unwrap_err();
        assert!(duplicate.contains("found 2"));
        assert!(duplicate
            .contains("0: \"hw:CARD=octesseradac,DEV=0\", 1: \"hw:CARD=octesseradac,DEV=0\""));

        let empty = select_orange_device_index(&[], ORANGE_AUDIO_DEVICE_NAME).unwrap_err();
        assert!(empty.contains("available output devices: []"));
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

    #[test]
    fn orange_sample_directory_rejects_non_directory_path() {
        let path = std::env::temp_dir().join(format!(
            "octessera-orange-sample-file-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, b"not a directory").unwrap();
        let error = validate_orange_samples_dir(path.clone()).unwrap_err();
        assert!(error.contains("Orange sample directory is not usable"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn orange_audio_host_forwards_events_commands_and_silence() {
        let (audio, command_rx, mut event_rx) = test_service();
        let mut host = OrangeAudioHost::new(audio, PathBuf::from("samples"));
        host.handle_musical_event(&MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100,
            duration_ms: Some(25),
        })
        .unwrap();
        assert!(matches!(
            event_rx.try_recv(),
            Ok(EngineEvent::NoteOn { note: 60, .. })
        ));

        host.handle_audio_command(&RuntimeAudioCommand::SetMasterVolume { volume_pct: 80.0 })
            .unwrap();
        assert!(matches!(
            command_rx.recv().unwrap(),
            AudioControlRequest::Dynamic(event)
                if matches!(*event, EngineEvent::SetMasterVolume { volume_pct } if volume_pct == 80.0)
        ));

        host.silence_internal_audio().unwrap();
        assert!(matches!(event_rx.try_recv(), Ok(EngineEvent::AllNotesOff)));
        assert!(event_rx.try_recv().is_err());
    }

    #[test]
    fn orange_midi_is_a_disabled_noop() {
        let (audio, _, _) = test_service();
        let mut host = OrangeAudioHost::new(audio, PathBuf::from("samples"));
        host.handle_midi_message(&[0x90, 60, 100]).unwrap();
        host.panic_external_midi().unwrap();
    }

    #[test]
    fn orange_midi_platform_effect_is_typed_unavailable() {
        let (audio, _, _) = test_service();
        let mut host = OrangeAudioHost::new(audio, PathBuf::from("samples"));
        let request = RuntimePlatformRequest::new(
            RuntimePlatformEffect::MidiListOutputsRequest,
            "test".into(),
            None,
        );
        let responses = host.handle_platform_effect(&request).unwrap();
        let [HostMessage::RuntimeResult { result }] = responses.as_slice() else {
            panic!("expected one unavailable result");
        };
        assert!(matches!(
            result,
            RuntimeStoreResult::RuntimeFailure { error }
                if error.code == RuntimeErrorCode::Unsupported
        ));
    }
}
