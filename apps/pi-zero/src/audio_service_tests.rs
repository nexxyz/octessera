use super::*;
use std::sync::mpsc::Sender;

#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn test_service() -> (
    AudioService,
    Receiver<AudioControlRequest>,
    EngineEventReceiver,
) {
    let (service, control_rx, event_rx, _) = test_service_with_prep_sender();
    (service, control_rx, event_rx)
}

#[cfg(test)]
pub(crate) fn test_service_for_sample_prep() -> AudioService {
    test_service_with_prep_result_sender().0
}

#[cfg(test)]
pub(crate) fn test_service_with_prep_result_sender() -> (AudioService, Sender<HostMessage>) {
    let (control_tx, _control_rx) = std::sync::mpsc::sync_channel(32);
    let (prep_result_tx, prep_result_rx) = std::sync::mpsc::channel();
    let service = AudioService {
        realtime_txs: Arc::new(Mutex::new(Vec::new())),
        replay_events: Arc::new(Mutex::new(ReplayCache::default())),
        attach_gate: crate::audio_sink_registry::new_attach_gate(),
        control_tx,
        config_revision: Arc::new(AtomicU64::new(0)),
        sample_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        sample_bank_signature: Arc::new(Mutex::new(String::new())),
        preview_generation: Arc::new(AtomicU64::new(0)),
        momentary_fx_types: Arc::new(Mutex::new(std::collections::BTreeMap::new())),
        next_sequence: Arc::new(AtomicU64::new(0)),
        latest_full_sequence: Arc::new(AtomicU64::new(0)),
        generations: Arc::new(Mutex::new(AudioGenerationState::default())),
        route_registry: crate::audio_route::new_registry(AudioOutputSet::jack()),
        audio_outputs: AudioOutputSet::jack(),
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        required_jack_health: None,
        prep_result_rx: Arc::new(Mutex::new(prep_result_rx)),
        recorder: Arc::new(Mutex::new(RecordingServices::new(
            std::env::temp_dir().join("octessera-sample-prep-recordings"),
            std::env::temp_dir().join("octessera-sample-prep-screen-recordings"),
        ))),
        recording_tap: Arc::new(RwLock::new(None)),
        recording_oled: Arc::new(RwLock::new(None)),
    };
    (service, prep_result_tx)
}

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) fn test_service_with_prep_worker() -> AudioService {
    let (control_tx, control_rx) = std::sync::mpsc::sync_channel(32);
    let (prep_result_tx, prep_result_rx) = std::sync::mpsc::channel();
    let service = AudioService {
        realtime_txs: Arc::new(Mutex::new(Vec::new())),
        replay_events: Arc::new(Mutex::new(ReplayCache::default())),
        attach_gate: crate::audio_sink_registry::new_attach_gate(),
        control_tx,
        config_revision: Arc::new(AtomicU64::new(0)),
        sample_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        sample_bank_signature: Arc::new(Mutex::new(String::new())),
        preview_generation: Arc::new(AtomicU64::new(0)),
        momentary_fx_types: Arc::new(Mutex::new(std::collections::BTreeMap::new())),
        next_sequence: Arc::new(AtomicU64::new(0)),
        latest_full_sequence: Arc::new(AtomicU64::new(0)),
        generations: Arc::new(Mutex::new(AudioGenerationState::default())),
        route_registry: crate::audio_route::new_registry(AudioOutputSet::jack()),
        audio_outputs: AudioOutputSet::jack(),
        required_jack_health: None,
        prep_result_rx: Arc::new(Mutex::new(prep_result_rx)),
        recorder: Arc::new(Mutex::new(RecordingServices::new(
            std::env::temp_dir().join("octessera-sample-prep-recordings"),
            std::env::temp_dir().join("octessera-sample-prep-screen-recordings"),
        ))),
        recording_tap: Arc::new(RwLock::new(None)),
        recording_oled: Arc::new(RwLock::new(None)),
    };
    crate::host_audio_prep::spawn_audio_control_worker(control_rx, service.clone(), prep_result_tx);
    service
}

#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn test_service_with_prep_sender() -> (
    AudioService,
    Receiver<AudioControlRequest>,
    EngineEventReceiver,
    Sender<HostMessage>,
) {
    test_service_with_recording_dir(
        std::env::temp_dir().join("octessera-orange-sample-prep-recordings"),
    )
}

#[cfg(all(test, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn test_service_with_outputs(outputs: AudioOutputSet) -> AudioService {
    let (mut service, _, _, _) = test_service_with_recording_dir(
        std::env::temp_dir().join("octessera-orange-gate-recordings"),
    );
    service.audio_outputs = outputs;
    service
}

#[cfg(test)]
pub(crate) fn test_service_with_recording_dir(
    recording_dir: std::path::PathBuf,
) -> (
    AudioService,
    Receiver<AudioControlRequest>,
    EngineEventReceiver,
    Sender<HostMessage>,
) {
    let (event_tx, event_rx) = event_queue();
    let (control_tx, control_rx) = std::sync::mpsc::sync_channel(32);
    let (prep_result_tx, prep_result_rx) = std::sync::mpsc::channel();
    let service = AudioService {
        realtime_txs: Arc::new(Mutex::new(vec![test_sink_sender(event_tx)])),
        replay_events: Arc::new(Mutex::new(ReplayCache::default())),
        attach_gate: crate::audio_sink_registry::new_attach_gate(),
        control_tx,
        config_revision: Arc::new(AtomicU64::new(0)),
        sample_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        sample_bank_signature: Arc::new(Mutex::new(String::new())),
        preview_generation: Arc::new(AtomicU64::new(0)),
        momentary_fx_types: Arc::new(Mutex::new(std::collections::BTreeMap::new())),
        next_sequence: Arc::new(AtomicU64::new(0)),
        latest_full_sequence: Arc::new(AtomicU64::new(0)),
        generations: Arc::new(Mutex::new(AudioGenerationState::default())),
        route_registry: crate::audio_route::new_registry(AudioOutputSet::jack()),
        audio_outputs: AudioOutputSet::jack(),
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        required_jack_health: None,
        prep_result_rx: Arc::new(Mutex::new(prep_result_rx)),
        recorder: Arc::new(Mutex::new(RecordingServices::new(
            recording_dir.clone(),
            recording_dir,
        ))),
        recording_tap: Arc::new(RwLock::new(None)),
        recording_oled: Arc::new(RwLock::new(None)),
    };
    (service, control_rx, event_rx, prep_result_tx)
}

#[test]
fn restore_preflight_finalizes_active_recording() {
    let service = test_service_for_sample_prep();
    service.start_recording(1).unwrap();
    assert!(service.is_recording().unwrap());
    service.prepare_restore().unwrap();
    assert!(!service.is_recording().unwrap());
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn raspberry_optional_route_fault_does_not_block_jack_readiness() {
    let mut service = test_service_for_sample_prep();
    service.audio_outputs = AudioOutputSet::from_flags(true, true, false).unwrap();
    crate::audio_route::set_status(
        &service.route_registry,
        AudioSink::Jack,
        crate::audio_route::AudioRouteStatus::Active,
    );
    crate::audio_route::set_status(
        &service.route_registry,
        AudioSink::Usb,
        crate::audio_route::AudioRouteStatus::Faulted,
    );

    assert!(service.ensure_route_readiness().is_ok());
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn raspberry_required_jack_worker_terminal_health_reaches_runtime_fault_path() {
    for reason in [
        realtime_engine::synth::SourceWorkerHealth::DispatchFailed,
        realtime_engine::synth::SourceWorkerHealth::CompletionFailed,
        realtime_engine::synth::SourceWorkerHealth::WorkerExited,
    ] {
        let mut service = test_service_for_sample_prep();
        let health = AudioStreamHealth::new("Jack".into());
        health.mark_worker_health(reason);
        service.required_jack_health = Some(health);

        assert!(service.required_jack_failed());
    }
}
