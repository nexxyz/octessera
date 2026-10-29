use super::*;
use crate::audio_recording::RecordingServices;

impl AudioManager {
    pub(super) fn new_with_opener(
        construction: AudioConstructionConfig,
        policy: AudioOpenPolicy,
        open_sink: AudioSinkOpener,
        route_registry: AudioRouteRegistry,
    ) -> Result<Self, String> {
        let AudioOpenPolicy::Outputs(outputs) = policy;
        require_jack_output(outputs)?;
        let (control_tx, control_rx) = mpsc::sync_channel::<AudioControlRequest>(32);
        let (prep_result_tx, prep_result_rx) = mpsc::channel::<HostMessage>();
        #[cfg(any(
            feature = "hardware-orange-pi-zero-2w",
            feature = "hardware-raspberry-pi-zero-2w"
        ))]
        let (load_tx, load_rx) = rodio_engine_source::audio_load_status_channel();
        #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
        let load_status_enabled = construction.load_status_enabled();
        let selected_outputs = SelectedOutputs::new(outputs);
        let side_producers = [new_pcm_mirror().producer, new_pcm_mirror().producer];
        for sink in [AudioSink::Usb, AudioSink::Hdmi] {
            if !AudioSink::selected(outputs).contains(&sink) {
                side_producers[mirror_index(sink).expect("side mirror index")].invalidate();
            }
        }
        let engine = AudioEngineOwner::new(ReplayCache::default());
        let recorder = Arc::new(Mutex::new(RecordingServices::new(
            recordings_dir(),
            screen_recordings_dir(),
        )));
        let mix_taps = Arc::new(RwLock::new(super::MixTaps::default()));
        let recording_oled = Arc::new(RwLock::new(None));
        #[cfg(any(
            feature = "hardware-orange-pi-zero-2w",
            feature = "hardware-raspberry-pi-zero-2w"
        ))]
        let jack_load_tx = super::audio_output_open::load_status_sender_for_sink(
            construction,
            AudioSink::Jack,
            &load_tx,
        );
        #[cfg(not(any(
            feature = "hardware-orange-pi-zero-2w",
            feature = "hardware-raspberry-pi-zero-2w"
        )))]
        let jack_load_tx = None;
        let jack = open_sink(
            construction,
            AudioSink::Jack,
            Some(mix_taps.clone()),
            jack_load_tx,
            side_producers.clone().map(Some),
            None,
        )
        .inspect_err(|error| set_status(&route_registry, AudioSink::Jack, error.status()))
        .map_err(|error| error.to_string())?;
        set_status(
            &route_registry,
            AudioSink::Jack,
            crate::audio_route::AudioRouteStatus::Active,
        );
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let required_jack_health = Some(jack.health.clone());
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        let streams = vec![*jack
            ._stream
            .expect("Raspberry audio stream must be present")];
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        engine.attach(jack.engine_tx.expect("Jack engine event sender"))?;
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let streams = Vec::new();
        let service = AudioService {
            engine: engine.clone(),
            control_tx,
            config_revision: Arc::new(AtomicU64::new(0)),
            sample_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            sample_bank_signature: Arc::new(Mutex::new(String::new())),
            preview_generation: Arc::new(AtomicU64::new(0)),
            momentary_fx_types: Arc::new(Mutex::new(std::collections::BTreeMap::new())),
            next_sequence: Arc::new(AtomicU64::new(0)),
            latest_full_sequence: Arc::new(AtomicU64::new(0)),
            generations: Arc::new(Mutex::new(crate::audio::AudioGenerationState::default())),
            prep_result_rx: Arc::new(Mutex::new(prep_result_rx)),
            route_registry: route_registry.clone(),
            selected_outputs: selected_outputs.clone(),
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            required_jack_health: required_jack_health.clone(),
            recorder,
            mix_taps: mix_taps.clone(),
            recording_oled: recording_oled.clone(),
            accepted_oled_frame: Arc::new(RwLock::new(None)),
            accepted_leds: Arc::new(RwLock::new(None)),
        };
        crate::host_audio_prep::spawn_audio_control_worker(
            control_rx,
            service.clone(),
            prep_result_tx,
        );
        let optional_recovery = [AudioSink::Usb, AudioSink::Hdmi].map(|sink| {
            audio_optional_recovery::spawn(
                construction,
                route_registry.clone(),
                sink,
                side_producers[mirror_index(sink).expect("side mirror index")].clone(),
                selected_outputs.clone(),
            )
        });
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let AudioConstructionConfig::Orange(profile) = construction;
        #[cfg(feature = "hardware-orange-pi-zero-2w")]
        let orange_dac_recovery = Some(OrangeRecoveryController::new_required(
            jack,
            profile,
            engine.clone(),
            Some(mix_taps.clone()),
            side_producers.clone().map(Some),
        )?);
        Ok(Self {
            _streams: streams,
            service,
            _optional_recovery: optional_recovery,
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            route_registry,
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            orange_dac_recovery,
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
