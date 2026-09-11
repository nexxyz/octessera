use super::*;
use crate::audio_recording::RecordingServices;

impl AudioManager {
    pub(super) fn new_with_opener(
        construction: AudioConstructionConfig,
        sinks: Vec<AudioSink>,
        allow_partial: bool,
        policy: AudioOpenPolicy,
        open_sink: AudioSinkOpener,
        route_registry: AudioRouteRegistry,
        attach_gate: AudioAttachGate,
    ) -> Result<Self, String> {
        let AudioOpenPolicy::Outputs(outputs) = policy;
        require_jack_output(outputs)?;
        let (control_tx, control_rx) = mpsc::channel::<AudioControlRequest>();
        let (prep_result_tx, prep_result_rx) = mpsc::channel::<HostMessage>();
        #[cfg(any(
            feature = "hardware-orange-pi-zero-2w",
            feature = "hardware-raspberry-pi-zero-2w"
        ))]
        let (load_tx, load_rx) = rodio_engine_source::audio_load_status_channel();
        #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
        let load_status_enabled = construction.load_status_enabled();
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
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let mut terminal_optional_sinks = Vec::new();
        let mut mirror_producers: Option<PcmMirrorProducers> = Some([None, None]);
        let mut mirror_producers_for_recovery: [Option<PcmMirrorProducer>; 2] = [None, None];
        let mut mirror_consumers: [Option<PcmMirrorConsumer>; 2] = [None, None];
        for sink in [AudioSink::Usb, AudioSink::Hdmi] {
            if AudioSink::selected(match policy {
                AudioOpenPolicy::Outputs(outputs) => outputs,
            })
            .contains(&sink)
            {
                let pair = new_pcm_mirror();
                let index = mirror_index(sink).expect("secondary mirror index");
                mirror_producers.as_mut().expect("mirror producers")[index] =
                    Some(pair.producer.clone());
                mirror_producers_for_recovery[index] = Some(pair.producer);
                mirror_consumers[index] = Some(pair.consumer);
            }
        }
        let realtime_txs = Arc::new(Mutex::new(Vec::new()));
        let replay_events = Arc::new(Mutex::new(default_replay_events()));
        let recorder = Arc::new(Mutex::new(RecordingServices::new(
            recordings_dir(),
            screen_recordings_dir(),
        )));
        let recording_tap = Arc::new(RwLock::new(None));
        let recording_oled = Arc::new(RwLock::new(None));
        for sink in sinks {
            let tap = (sink == AudioSink::Jack).then(|| recording_tap.clone());
            #[cfg(any(
                feature = "hardware-orange-pi-zero-2w",
                feature = "hardware-raspberry-pi-zero-2w"
            ))]
            let source_load_tx =
                super::audio_output_open::load_status_sender_for_sink(construction, sink, &load_tx);
            #[cfg(not(any(
                feature = "hardware-orange-pi-zero-2w",
                feature = "hardware-raspberry-pi-zero-2w"
            )))]
            let source_load_tx = None;
            let sink_mirror_producers = if sink == AudioSink::Jack {
                mirror_producers.take().expect("Jack mirror producers")
            } else {
                [None, None]
            };
            let mirror_consumer =
                mirror_index(sink).and_then(|index| mirror_consumers[index].take());
            match open_sink(
                construction,
                sink,
                tap,
                source_load_tx,
                sink_mirror_producers,
                mirror_consumer,
            ) {
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
                            *opened
                                ._stream
                                .expect("Raspberry audio stream must be present"),
                        );
                        attach_sink_atomic(
                            &attach_gate,
                            &realtime_txs,
                            &replay_events,
                            sink,
                            opened.engine_tx.expect("Jack engine event sender"),
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
                Err(error)
                    if startup_open_action(policy, sink, allow_partial, &error)
                        == StartupOpenAction::Ignore =>
                {
                    set_status(&route_registry, sink, error.status());
                    #[cfg(feature = "hardware-orange-pi-zero-2w")]
                    terminal_optional_sinks.push(sink);
                    eprintln!("{sink:?} audio init failed: {error} (optional route disabled)");
                }
                Err(error) => {
                    set_status(&route_registry, sink, error.status());
                    return Err(error.to_string());
                }
            }
        }
        let requires_stream = true;
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
            recorder,
            recording_tap: recording_tap.clone(),
            recording_oled: recording_oled.clone(),
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
                    construction,
                    route_registry.clone(),
                    sink,
                    mirror_producers_for_recovery[mirror_index(sink).expect("optional mirror")]
                        .clone()
                        .expect("optional mirror producer"),
                ));
            }
        }
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let AudioOpenPolicy::Outputs(outputs) = policy;
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let AudioConstructionConfig::Orange(profile) = construction;
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let orange_dac_recovery = orange_jack_opened
            .map(|opened| {
                OrangeRecoveryController::new_required(
                    opened,
                    profile,
                    realtime_txs.clone(),
                    replay_events.clone(),
                    Some(recording_tap.clone()),
                    attach_gate.clone(),
                    mirror_producers_for_recovery.clone(),
                )
            })
            .transpose()?;
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let orange_recovery = [AudioSink::Usb, AudioSink::Hdmi]
            .into_iter()
            .filter(|sink| {
                AudioSink::selected(outputs).contains(sink)
                    && !terminal_optional_sinks.contains(sink)
            })
            .map(|sink| {
                let initial = orange_optional_opened
                    .iter()
                    .position(|(opened_sink, _)| *opened_sink == sink)
                    .map(|index| orange_optional_opened.swap_remove(index).1);
                match initial {
                    Some(opened) => OrangeRecoveryController::new_optional_initial(
                        sink,
                        opened,
                        profile,
                        realtime_txs.clone(),
                        replay_events.clone(),
                        attach_gate.clone(),
                        mirror_producers_for_recovery[mirror_index(sink).expect("optional mirror")]
                            .clone()
                            .expect("optional mirror producer"),
                    ),
                    None => Ok(OrangeRecoveryController::new_optional_missing(
                        sink,
                        profile,
                        realtime_txs.clone(),
                        replay_events.clone(),
                        attach_gate.clone(),
                        mirror_producers_for_recovery[mirror_index(sink).expect("optional mirror")]
                            .clone()
                            .expect("optional mirror producer"),
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
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            load_tx,
            #[cfg(any(
                feature = "hardware-orange-pi-zero-2w",
                feature = "hardware-raspberry-pi-zero-2w"
            ))]
            load_rx: Some(load_rx),
            #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
            load_status_enabled,
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            load_status_reset_pending: false,
        })
    }
}
