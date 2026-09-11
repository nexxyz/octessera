use super::{AudioControlRequest, AudioService};
use crate::audio_replay::default_replay_events;
use crate::audio_route::{new_registry, set_status, AudioRouteRegistry};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use crate::audio_sink_registry::attach_sink_atomic;
use crate::audio_sink_registry::{new_attach_gate, AudioAttachGate};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) use crate::audio_stream_health::AudioStreamStatus as OrangeDacStatus;
mod audio_sink;
pub(crate) use audio_sink::AudioSink;
#[cfg(any(
    feature = "hardware-orange-pi-zero-2w",
    feature = "hardware-raspberry-pi-zero-2w"
))]
#[path = "audio_load_status.rs"]
mod audio_load_status;
#[cfg(feature = "hardware-raspberry-pi-zero-2w")]
pub(crate) use audio_load_status::drain_audio_load_status;
mod audio_manager_construction;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod audio_optional_recovery;
mod audio_output_open;
#[path = "audio_profile.rs"]
mod audio_profile;
mod audio_stream_lifecycle;
mod cpal_audio_callback;
#[cfg(any(
    feature = "hardware-orange-pi-zero-2w",
    all(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
        not(feature = "legacy-hardware-rpi-zero-2w"),
        not(feature = "legacy-hardware-pi")
    )
))]
pub(crate) use audio_stream_lifecycle::{
    AudioStreamBuildError, AudioStreamLifecycle, AudioStreamShutdownError,
    AudioStreamShutdownReport,
};
#[cfg(any(
    feature = "hardware-orange-pi-zero-2w",
    all(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
        not(feature = "legacy-hardware-rpi-zero-2w"),
        not(feature = "legacy-hardware-pi")
    )
))]
pub(crate) use cpal_audio_callback::CallbackSource;
#[path = "cpal_audio_output.rs"]
mod cpal_audio_output;
#[cfg(test)]
#[path = "audio_direct_cpal_tests.rs"]
mod direct_cpal_tests;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_audio_manager;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_audio_recovery;
#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
#[path = "orange_audio_recovery_tests.rs"]
mod orange_audio_recovery_tests;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use audio_output_open::open_audio_sink;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use audio_output_open::open_orange_audio_sink;
use audio_output_open::recordings_dir;
use audio_output_open::screen_recordings_dir;
use audio_output_open::{AudioConstructionConfig, AudioSinkOpener};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(super) use audio_profile::OrangeAudioProfile;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) use audio_profile::{AudioProfileGeometry, RaspberryAudioProfile};
use cpal_audio_output::probe_cpal_sink;
use cpal_audio_output::BuiltAudioStream;
use media_recording::RecordingTap;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use orange_audio_recovery::OrangeRecoveryController;
use playback_runtime::AudioOutputSet;
use playback_runtime::HostMessage;
#[cfg(any(
    feature = "hardware-orange-pi-zero-2w",
    feature = "hardware-raspberry-pi-zero-2w"
))]
use rodio_engine_source::AudioLoadStatusReceiver;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use rodio_engine_source::AudioLoadStatusSender;
use rodio_engine_source::{
    new_pcm_mirror, PcmMirrorConsumer, PcmMirrorProducer, PcmMirrorProducers,
};
use std::sync::atomic::AtomicU64;
use std::sync::mpsc;
use std::sync::RwLock;
use std::sync::{Arc, Mutex};

const JACK_AUDIO_REQUIRED_ERROR: &str = "Jack Audio is always on";

pub struct AudioManager {
    _streams: Vec<BuiltAudioStream>,
    service: AudioService,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    route_registry: AudioRouteRegistry,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    orange_dac_recovery: Option<OrangeRecoveryController>,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    _orange_recovery: Vec<OrangeRecoveryController>,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    load_tx: AudioLoadStatusSender,
    #[cfg(any(
        feature = "hardware-orange-pi-zero-2w",
        feature = "hardware-raspberry-pi-zero-2w"
    ))]
    load_rx: Option<AudioLoadStatusReceiver>,
    #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
    load_status_enabled: bool,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    load_status_reset_pending: bool,
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    #[allow(dead_code)]
    optional_recovery: Vec<audio_optional_recovery::OptionalRecoveryWorker>,
}

#[derive(Clone, Copy)]
enum AudioOpenPolicy {
    Outputs(AudioOutputSet),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StartupOpenAction {
    Wait,
    Ignore,
    Fail,
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum OrangeAudioInitError {
    Open(String),
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
impl std::fmt::Display for OrangeAudioInitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open(error) => formatter.write_str(error),
        }
    }
}

fn required_sink(policy: AudioOpenPolicy, sink: AudioSink) -> bool {
    let _ = policy;
    sink == AudioSink::Jack
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

fn startup_open_action(
    policy: AudioOpenPolicy,
    sink: AudioSink,
    allow_partial: bool,
    error: &crate::audio_route::RouteOpenError,
) -> StartupOpenAction {
    if allow_partial && !required_sink(policy, sink) {
        if error.is_waiting() {
            StartupOpenAction::Wait
        } else {
            StartupOpenAction::Ignore
        }
    } else {
        StartupOpenAction::Fail
    }
}

pub(super) type RecordingTapState = Arc<RwLock<Option<RecordingTap>>>;

impl AudioManager {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    pub fn new<T: Into<AudioOutputSet>>(
        optimization: playback_runtime::AudioOptimization,
        outputs: T,
    ) -> Result<Self, String> {
        Self::new_raspberry_profile(
            RaspberryAudioProfile::from_optimization(optimization),
            outputs,
        )
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
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

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
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
        Self::new_with_opener(
            AudioConstructionConfig::raspberry(profile),
            AudioSink::startup(outputs),
            true,
            AudioOpenPolicy::Outputs(outputs),
            open_audio_sink,
            route_registry.clone(),
            new_attach_gate(),
        )
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub fn new_orange(
        profile: OrangeAudioProfile,
        outputs: AudioOutputSet,
    ) -> Result<Self, OrangeAudioInitError> {
        require_jack_output(outputs).map_err(OrangeAudioInitError::Open)?;
        let sinks = AudioSink::startup(outputs);
        let route_registry = new_registry(outputs);
        if let Err(error) = probe_cpal_sink(AudioSink::Jack) {
            set_status(&route_registry, AudioSink::Jack, error.status());
            return Err(OrangeAudioInitError::Open(error.to_string()));
        }
        Self::new_with_opener(
            AudioConstructionConfig::orange(profile),
            sinks,
            true,
            AudioOpenPolicy::Outputs(outputs),
            open_orange_audio_sink,
            route_registry.clone(),
            new_attach_gate(),
        )
        .map_err(OrangeAudioInitError::Open)
    }

    pub fn service(&self) -> AudioService {
        self.service.clone()
    }

    #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
    pub(crate) fn take_load_status_receiver(&mut self) -> Option<AudioLoadStatusReceiver> {
        self.load_status_enabled
            .then(|| self.load_rx.take())
            .flatten()
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub(crate) fn required_jack_runtime_status(
        &self,
    ) -> crate::audio_stream_health::AudioStreamStatus {
        self.orange_dac_recovery
            .as_ref()
            .map(OrangeRecoveryController::runtime_status)
            .unwrap_or(OrangeDacStatus::Healthy)
    }
}

#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
#[path = "audio_load_status_tests.rs"]
mod audio_load_status_tests;
#[cfg(all(test, feature = "hardware-raspberry-pi-zero-2w"))]
#[path = "raspberry_audio_load_status_tests.rs"]
mod raspberry_audio_load_status_tests;
#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
#[path = "audio_output_route_tests.rs"]
mod route_tests;
#[cfg(test)]
#[path = "audio_output_tests.rs"]
mod tests;
