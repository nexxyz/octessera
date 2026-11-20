use super::{AudioControlRequest, AudioService};
use crate::audio_engine_owner::AudioEngineOwner;
use crate::audio_replay::ReplayCache;
use crate::audio_route::{new_registry, set_status, AudioRouteRegistry, SelectedOutputs};
#[cfg(board_orange)]
pub(crate) use crate::audio_stream_health::AudioStreamStatus as OrangeDacStatus;
mod audio_sink;
pub(crate) use audio_sink::AudioSink;
#[cfg(not(board_host))]
mod audio_load_status;
#[cfg(board_raspberry)]
pub(crate) use audio_load_status::drain_audio_load_status;
mod audio_manager_construction;
mod audio_optional_recovery;
mod audio_output_open;
mod audio_profile;
mod audio_stream_lifecycle;
mod cpal_audio_callback;
mod cpal_audio_output;
#[cfg(test)]
mod direct_cpal_tests;
#[cfg(board_orange)]
mod orange_audio_manager;
#[cfg(board_orange)]
mod orange_audio_recovery;
#[cfg(all(test, board_orange))]
mod orange_audio_recovery_tests;
#[cfg(board_not_orange)]
use audio_output_open::open_audio_sink;
#[cfg(board_orange)]
use audio_output_open::open_orange_audio_sink;
use audio_output_open::recordings_dir;
use audio_output_open::screen_recordings_dir;
use audio_output_open::{AudioConstructionConfig, AudioSinkOpener};
pub(crate) use audio_profile::sequenced_output_delay;
#[cfg(all(board_not_orange, feature = "profiling"))]
pub(crate) use audio_profile::AudioProfileGeometry;
#[cfg(board_orange)]
pub(super) use audio_profile::OrangeAudioProfile;
#[cfg(board_not_orange)]
pub(crate) use audio_profile::RaspberryAudioProfile;
use cpal_audio_output::probe_cpal_sink;
use cpal_audio_output::BuiltAudioStream;
use media_recording::RecordingTap;
#[cfg(board_orange)]
use orange_audio_recovery::OrangeRecoveryController;
use playback_runtime::AudioOutputSet;
use playback_runtime::HostMessage;
use rodio_engine_source::new_pcm_mirror;
#[cfg(not(board_host))]
use rodio_engine_source::AudioLoadStatusReceiver;
#[cfg(board_orange)]
use rodio_engine_source::AudioLoadStatusSender;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc;
use std::sync::RwLock;
use std::sync::{Arc, Mutex};

const JACK_AUDIO_REQUIRED_ERROR: &str = "Jack Audio is always on";

pub struct AudioManager {
    _streams: Vec<BuiltAudioStream>,
    service: AudioService,
    #[cfg(board_orange)]
    route_registry: AudioRouteRegistry,
    #[cfg(board_orange)]
    orange_dac_recovery: Option<OrangeRecoveryController>,
    #[cfg(board_orange)]
    load_tx: AudioLoadStatusSender,
    #[cfg(not(board_host))]
    load_rx: Option<AudioLoadStatusReceiver>,
    #[cfg(board_orange)]
    load_status_reset_pending: bool,
    _optional_recovery: [audio_optional_recovery::OptionalRecoveryWorker; 2],
}

#[derive(Clone, Copy)]
enum AudioOpenPolicy {
    Outputs(AudioOutputSet),
}

#[cfg(board_orange)]
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum OrangeAudioInitError {
    Open(String),
}

#[cfg(board_orange)]
impl std::fmt::Display for OrangeAudioInitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open(error) => formatter.write_str(error),
        }
    }
}

fn require_jack_output(outputs: AudioOutputSet) -> Result<(), String> {
    outputs
        .dac()
        .then_some(())
        .ok_or_else(|| JACK_AUDIO_REQUIRED_ERROR.into())
}

fn mirror_index(sink: AudioSink) -> Option<usize> {
    match sink {
        AudioSink::Jack => None,
        AudioSink::Usb => Some(0),
        AudioSink::Hdmi => Some(1),
    }
}

/// Copies of the final mix taken in the primary callback: the recorder and
/// the Bluetooth monitor. Both are bounded and never block the callback.
#[derive(Default)]
pub(crate) struct MixTaps {
    pub(crate) recording: Option<RecordingTap>,
    pub(crate) monitor: Option<RecordingTap>,
}

pub(crate) type MixTapState = Arc<RwLock<MixTaps>>;

impl AudioManager {
    #[cfg(board_not_orange)]
    pub fn new<T: Into<AudioOutputSet>>(
        optimization: playback_runtime::AudioOptimization,
        outputs: T,
    ) -> Result<Self, String> {
        Self::new_raspberry_profile(
            RaspberryAudioProfile::from_optimization(optimization),
            outputs,
        )
    }

    #[cfg(all(board_not_orange, feature = "profiling"))]
    pub(crate) fn new_timing_probe<T: Into<AudioOutputSet>>(
        output_buffer_frames: Option<u32>,
        internal_block_frames: Option<usize>,
        outputs: T,
    ) -> Result<(Self, AudioProfileGeometry), String> {
        let profile =
            RaspberryAudioProfile::from_timing_probe(output_buffer_frames, internal_block_frames);
        let geometry = profile.geometry();
        Self::new_raspberry_profile(profile, outputs).map(|manager| (manager, geometry))
    }

    #[cfg(board_not_orange)]
    fn new_raspberry_profile<T: Into<AudioOutputSet>>(
        profile: RaspberryAudioProfile,
        outputs: T,
    ) -> Result<Self, String> {
        let outputs = outputs.into();
        require_jack_output(outputs)?;
        let route_registry = new_registry(outputs);
        if let Err(error) = probe_cpal_sink(AudioSink::Jack) {
            set_status(&route_registry, AudioSink::Jack, error.status());
            return Err(error.to_string());
        }
        crate::audio_recording::spawn_partial_take_repair([
            recordings_dir(),
            screen_recordings_dir(),
        ]);
        Self::new_with_opener(
            AudioConstructionConfig::raspberry(profile),
            AudioOpenPolicy::Outputs(outputs),
            open_audio_sink,
            route_registry.clone(),
        )
    }

    #[cfg(board_orange)]
    pub fn new_orange(
        profile: OrangeAudioProfile,
        outputs: AudioOutputSet,
    ) -> Result<Self, OrangeAudioInitError> {
        require_jack_output(outputs).map_err(OrangeAudioInitError::Open)?;
        let route_registry = new_registry(outputs);
        if let Err(error) = probe_cpal_sink(AudioSink::Jack) {
            set_status(&route_registry, AudioSink::Jack, error.status());
            return Err(OrangeAudioInitError::Open(error.to_string()));
        }
        crate::audio_recording::spawn_partial_take_repair([
            recordings_dir(),
            screen_recordings_dir(),
        ]);
        Self::new_with_opener(
            AudioConstructionConfig::orange(profile),
            AudioOpenPolicy::Outputs(outputs),
            open_orange_audio_sink,
            route_registry.clone(),
        )
        .map_err(OrangeAudioInitError::Open)
    }

    pub fn service(&self) -> AudioService {
        self.service.clone()
    }

    #[cfg(board_raspberry)]
    pub(crate) fn take_load_status_receiver(&mut self) -> Option<AudioLoadStatusReceiver> {
        self.load_rx.take()
    }

    #[cfg(board_orange)]
    pub(crate) fn required_jack_runtime_status(
        &self,
    ) -> crate::audio_stream_health::AudioStreamStatus {
        self.orange_dac_recovery
            .as_ref()
            .map(OrangeRecoveryController::runtime_status)
            .unwrap_or(OrangeDacStatus::Healthy)
    }
}

#[cfg(all(test, board_orange))]
mod audio_load_status_tests;
#[cfg(all(test, board_raspberry))]
mod raspberry_audio_load_status_tests;
#[cfg(test)]
mod tests;
