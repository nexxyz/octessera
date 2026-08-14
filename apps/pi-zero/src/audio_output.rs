use super::{AudioControlRequest, AudioService};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use crate::audio_recording::recording_owner;
use crate::audio_replay::default_replay_events;
use crate::audio_route::{new_registry, set_status, AudioRouteRegistry};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use crate::audio_sink_registry::attach_sink_atomic;
use crate::audio_sink_registry::{new_attach_gate, AudioAttachGate};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) use crate::audio_stream_health::AudioStreamStatus as OrangeDacStatus;
mod audio_sink;
pub(crate) use audio_sink::AudioSink;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod audio_optional_recovery;
mod audio_output_open;
#[path = "cpal_audio_output.rs"]
mod cpal_audio_output;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_audio_recovery;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use crate::recording::RecordingTap;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use audio_output_open::open_audio_sink;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use audio_output_open::open_orange_audio_sink;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use audio_output_open::recordings_dir;
use audio_output_open::AudioSinkOpener;
use cpal::Stream;
use cpal_audio_output::probe_cpal_sink;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use orange_audio_recovery::OrangeRecoveryController;
use playback_runtime::AudioOutputSet;
use playback_runtime::HostMessage;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::sync::RwLock;
use std::sync::{Arc, Mutex};

pub struct AudioManager {
    _streams: Vec<Stream>,
    service: AudioService,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    route_registry: AudioRouteRegistry,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    orange_dac_recovery: Option<OrangeRecoveryController>,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    _orange_recovery: Vec<OrangeRecoveryController>,
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
    match policy {
        AudioOpenPolicy::Outputs(outputs) => outputs.dac() && sink == AudioSink::Jack,
    }
}

fn startup_open_action(
    policy: AudioOpenPolicy,
    sink: AudioSink,
    allow_partial: bool,
    error: &crate::audio_route::RouteOpenError,
) -> StartupOpenAction {
    if allow_partial && !required_sink(policy, sink) && error.is_waiting() {
        StartupOpenAction::Wait
    } else {
        StartupOpenAction::Fail
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(super) type RecordingTapState = Arc<RwLock<Option<RecordingTap>>>;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
type RecordingTapState = ();

impl AudioManager {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    pub fn new<T: Into<AudioOutputSet>>(
        output_buffer_frames: Option<u32>,
        outputs: T,
    ) -> Result<Self, String> {
        let outputs = outputs.into();
        let route_registry = new_registry(outputs);
        for sink in AudioSink::selected(outputs) {
            if let Err(error) = probe_cpal_sink(sink) {
                set_status(&route_registry, sink, error.status());
                if sink == AudioSink::Jack || !error.is_waiting() {
                    return Err(error.to_string());
                }
            }
        }
        Self::new_with_opener(
            output_buffer_frames,
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
        output_buffer_frames: Option<u32>,
        outputs: AudioOutputSet,
    ) -> Result<Self, OrangeAudioInitError> {
        let sinks = AudioSink::selected(outputs);
        let route_registry = new_registry(outputs);
        for sink in AudioSink::selected(outputs) {
            if let Err(error) = probe_cpal_sink(sink) {
                set_status(&route_registry, sink, error.status());
                if sink == AudioSink::Jack || !error.is_waiting() {
                    return Err(OrangeAudioInitError::Open(error.to_string()));
                }
            }
        }
        Self::new_with_opener(
            output_buffer_frames,
            sinks,
            true,
            AudioOpenPolicy::Outputs(outputs),
            open_orange_audio_sink,
            route_registry.clone(),
            new_attach_gate(),
        )
        .map_err(OrangeAudioInitError::Open)
    }

    fn new_with_opener(
        output_buffer_frames: Option<u32>,
        sinks: Vec<AudioSink>,
        allow_partial: bool,
        policy: AudioOpenPolicy,
        open_sink: AudioSinkOpener,
        route_registry: AudioRouteRegistry,
        attach_gate: AudioAttachGate,
    ) -> Result<Self, String> {
        let (control_tx, control_rx) = mpsc::channel::<AudioControlRequest>();
        let (prep_result_tx, prep_result_rx) = mpsc::channel::<HostMessage>();
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let mut streams = Vec::new();
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let streams = Vec::new();
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let mut orange_jack_opened = None;
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let mut orange_optional_opened = Vec::new();
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let mut required_jack_health = None;
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let mut optional_recovery = Vec::new();
        let realtime_txs = Arc::new(Mutex::new(Vec::new()));
        let replay_events = Arc::new(Mutex::new(default_replay_events()));
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let recorder = Arc::new(Mutex::new(crate::recording::RecorderService::new(
            recordings_dir(),
        )));
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let recording_tap = Arc::new(RwLock::new(None));
        for sink in sinks {
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            let tap = (recording_owner(match policy {
                AudioOpenPolicy::Outputs(outputs) => outputs,
            }) == Some(sink))
            .then(|| recording_tap.clone());
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            let tap = None;
            match open_sink(output_buffer_frames, sink, tap) {
                Ok(opened) => {
                    set_status(
                        &route_registry,
                        sink,
                        crate::audio_route::AudioRouteStatus::Active,
                    );
                    #[cfg(feature = "hardware-orange-pi-zero-2w")]
                    {
                        if sink == AudioSink::Jack {
                            orange_jack_opened = Some(opened);
                        } else {
                            orange_optional_opened.push((sink, opened));
                        }
                    }
                    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
                    {
                        required_jack_health =
                            (sink == AudioSink::Jack).then(|| opened.health.clone());
                        streams.push(
                            opened
                                ._stream
                                .expect("Raspberry audio stream must be present"),
                        );
                        attach_sink_atomic(
                            &attach_gate,
                            &realtime_txs,
                            &replay_events,
                            sink,
                            opened.engine_tx,
                        )
                        .map_err(|error| error.to_string())?;
                    }
                }
                Err(error)
                    if startup_open_action(policy, sink, allow_partial, &error)
                        == StartupOpenAction::Wait =>
                {
                    set_status(&route_registry, sink, error.status());
                    eprintln!("{sink:?} audio init failed: {error} (continuing with other sinks)");
                }
                Err(error) => {
                    set_status(&route_registry, sink, error.status());
                    return Err(error.to_string());
                }
            }
        }
        let requires_stream = match policy {
            AudioOpenPolicy::Outputs(outputs) => outputs.dac(),
        };
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let no_required_stream = orange_jack_opened.is_none();
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let no_required_stream = streams.is_empty();
        if no_required_stream && requires_stream {
            return Err("no requested audio outputs opened".into());
        }
        let service = AudioService {
            realtime_txs: realtime_txs.clone(),
            replay_events: replay_events.clone(),
            attach_gate: attach_gate.clone(),
            control_tx,
            config_revision: Arc::new(AtomicU64::new(0)),
            sample_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            sample_bank_signature: Arc::new(Mutex::new(String::new())),
            prep_result_rx: Arc::new(Mutex::new(prep_result_rx)),
            route_registry: route_registry.clone(),
            audio_outputs: match policy {
                AudioOpenPolicy::Outputs(outputs) => outputs,
            },
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            required_jack_health: required_jack_health.clone(),
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            recorder,
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            recording_tap: recording_tap.clone(),
        };
        crate::host_audio_prep::spawn_audio_control_worker(
            control_rx,
            service.clone(),
            prep_result_tx,
        );
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        {
            let AudioOpenPolicy::Outputs(outputs) = policy;
            for sink in AudioSink::optional_recovery(outputs) {
                optional_recovery.push(audio_optional_recovery::spawn(
                    output_buffer_frames,
                    realtime_txs.clone(),
                    replay_events.clone(),
                    recording_tap.clone(),
                    route_registry.clone(),
                    attach_gate.clone(),
                    sink,
                    recording_owner(outputs) == Some(sink),
                ));
            }
        }
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let orange_dac_recovery = orange_jack_opened
            .map(|opened| {
                OrangeRecoveryController::new_required(
                    opened,
                    output_buffer_frames,
                    realtime_txs.clone(),
                    replay_events.clone(),
                    attach_gate.clone(),
                )
            })
            .transpose()?;
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let AudioOpenPolicy::Outputs(outputs) = policy;
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let orange_recovery = [AudioSink::Usb, AudioSink::Hdmi]
            .into_iter()
            .filter(|sink| AudioSink::selected(outputs).contains(sink))
            .map(|sink| {
                let initial = orange_optional_opened
                    .iter()
                    .position(|(opened_sink, _)| *opened_sink == sink)
                    .map(|index| orange_optional_opened.swap_remove(index).1);
                match initial {
                    Some(opened) => OrangeRecoveryController::new_optional_initial(
                        sink,
                        opened,
                        output_buffer_frames,
                        realtime_txs.clone(),
                        replay_events.clone(),
                        attach_gate.clone(),
                    ),
                    None => Ok(OrangeRecoveryController::new_optional_missing(
                        sink,
                        output_buffer_frames,
                        realtime_txs.clone(),
                        replay_events.clone(),
                        attach_gate.clone(),
                    )),
                }
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            _streams: streams,
            service,
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            optional_recovery,
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            route_registry,
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            orange_dac_recovery,
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            _orange_recovery: orange_recovery,
        })
    }

    pub fn service(&self) -> AudioService {
        self.service.clone()
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub(crate) fn required_jack_status(&self) -> crate::audio_stream_health::AudioStreamStatus {
        self.orange_dac_recovery
            .as_ref()
            .map(OrangeRecoveryController::status)
            .unwrap_or(OrangeDacStatus::Healthy)
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
impl AudioManager {
    pub(crate) fn recover_audio_if_due(&mut self) {
        if let Some(recovery) = self.orange_dac_recovery.as_mut() {
            recovery.recover_if_due();
        }
        if let Some(recovery) = &self.orange_dac_recovery {
            set_status(
                &self.route_registry,
                AudioSink::Jack,
                match recovery.status() {
                    OrangeDacStatus::Healthy => crate::audio_route::AudioRouteStatus::Active,
                    OrangeDacStatus::Recovering => crate::audio_route::AudioRouteStatus::Waiting,
                    OrangeDacStatus::Terminal => crate::audio_route::AudioRouteStatus::Faulted,
                },
            );
        }
        for recovery in &mut self._orange_recovery {
            recovery.recover_if_due();
        }
        for recovery in &self._orange_recovery {
            set_status(
                &self.route_registry,
                recovery.sink(),
                if recovery.status() == OrangeDacStatus::Terminal {
                    crate::audio_route::AudioRouteStatus::Faulted
                } else if recovery.status() == OrangeDacStatus::Healthy {
                    crate::audio_route::AudioRouteStatus::Active
                } else {
                    crate::audio_route::AudioRouteStatus::Waiting
                },
            );
        }
    }

    pub(crate) fn ensure_selected_routes(&self) -> Result<(), String> {
        if let Some(recovery) = &self.orange_dac_recovery {
            if recovery.status() == OrangeDacStatus::Terminal {
                return Err("Orange Jack audio stream is not active".into());
            }
        }
        for recovery in &self._orange_recovery {
            if recovery.status() == OrangeDacStatus::Terminal {
                return Err(format!(
                    "selected {:?} audio route faulted",
                    recovery.sink()
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "audio_output_tests.rs"]
mod tests;
