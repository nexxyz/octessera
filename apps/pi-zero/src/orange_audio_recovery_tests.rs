use super::audio_output_open::OpenedAudioSink;
use super::audio_profile::OrangeAudioProfile;
use super::orange_audio_recovery::{
    OrangeRecoveryClock, OrangeRecoveryController, OrangeRecoveryDependencies, OrangeRecoveryOpener,
};
use super::AudioSink;
use crate::audio_replay::ReplayCache;
use crate::audio_route::RouteOpenError;
use crate::audio_sink_registry::{has_sink, new_attach_gate, register_sink};
use crate::audio_stream_health::{AudioStreamHealth, AudioStreamStatus};
use realtime_engine::synth::{
    default_synth_config, prepare_instrument_slot_config, InstrumentSlotConfig, SampleBankConfig,
    SampleBankParamId, SourceWorkerHealth,
};
use rodio_engine_source::{event_queue, EngineEvent, EngineEventReceiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn opener_with_calls() -> (OrangeRecoveryOpener, Arc<Mutex<usize>>) {
    let calls = Arc::new(Mutex::new(0));
    let call_count = calls.clone();
    let opener: OrangeRecoveryOpener = Arc::new(move |_, _, _, _, _, _, _| {
        *call_count.lock().unwrap() += 1;
        Err(RouteOpenError::Fault(
            "unexpected recovery opener call".into(),
        ))
    });
    (opener, calls)
}

fn clock() -> OrangeRecoveryClock {
    Arc::new(Instant::now)
}

fn opened(
    engine_tx: rodio_engine_source::EngineEventSender,
    health: AudioStreamHealth,
) -> OpenedAudioSink {
    OpenedAudioSink {
        engine_tx: Some(engine_tx),
        _stream: None,
        health,
        _test_engine_rx: None,
    }
}

#[test]
fn worker_terminal_stays_separate_from_orange_route_recovery() {
    let (jack_tx, _jack_rx) = event_queue();
    let jack_sinks = Arc::new(Mutex::new(Vec::new()));
    register_sink(&jack_sinks, AudioSink::Jack, jack_tx.clone());
    let jack_health = AudioStreamHealth::new("Jack".into());
    let (jack_opener, jack_calls) = opener_with_calls();
    let mut jack_controller = OrangeRecoveryController::new_initial_with_dependencies(
        AudioSink::Jack,
        true,
        opened(jack_tx, jack_health.clone()),
        OrangeRecoveryDependencies {
            profile: OrangeAudioProfile::from_optimization(
                playback_runtime::AudioOptimization::Capacity,
            ),
            realtime_txs: jack_sinks.clone(),
            replay_events: Arc::new(Mutex::new(ReplayCache::default())),
            attach_gate: new_attach_gate(),
            recording_tap: None,
            mirror_producer: None,
            mirror_producers: [None, None],
            opener: jack_opener,
            clock: clock(),
        },
    )
    .unwrap();

    jack_health.mark_worker_health(SourceWorkerHealth::DeadlineMiss);
    jack_controller.recover_if_due();

    assert_eq!(*jack_calls.lock().unwrap(), 0);
    assert_eq!(jack_controller.device_status(), AudioStreamStatus::Healthy);
    assert_eq!(jack_controller.runtime_status(), AudioStreamStatus::Healthy);
    assert_eq!(jack_health.external_status(), AudioStreamStatus::Healthy);
    assert_eq!(jack_health.runtime_status(), AudioStreamStatus::Healthy);
    assert_eq!(jack_health.worker_health(), SourceWorkerHealth::Healthy);
    assert!(has_sink(&jack_sinks, AudioSink::Jack));

    jack_health.mark_worker_health(SourceWorkerHealth::CompletionFailed);
    jack_controller.recover_if_due();

    assert_eq!(*jack_calls.lock().unwrap(), 0);
    assert_eq!(
        jack_controller.runtime_status(),
        AudioStreamStatus::Terminal
    );
    assert!(has_sink(&jack_sinks, AudioSink::Jack));

    let usb_health = AudioStreamHealth::optional("USB".into());
    let (usb_opener, usb_calls) = opener_with_calls();
    let usb_mirror = rodio_engine_source::new_pcm_mirror();
    let mut usb_controller = OrangeRecoveryController::new_initial_with_dependencies(
        AudioSink::Usb,
        false,
        OpenedAudioSink {
            engine_tx: None,
            _stream: None,
            health: usb_health.clone(),
            _test_engine_rx: None,
        },
        OrangeRecoveryDependencies {
            profile: OrangeAudioProfile::from_optimization(
                playback_runtime::AudioOptimization::Capacity,
            ),
            realtime_txs: Arc::new(Mutex::new(Vec::new())),
            replay_events: Arc::new(Mutex::new(ReplayCache::default())),
            attach_gate: new_attach_gate(),
            recording_tap: None,
            mirror_producer: Some(usb_mirror.producer),
            mirror_producers: [None, None],
            opener: usb_opener,
            clock: clock(),
        },
    )
    .unwrap();

    usb_health.mark_worker_health(SourceWorkerHealth::WorkerExited);
    usb_controller.recover_if_due();

    assert_eq!(*usb_calls.lock().unwrap(), 0);
    assert_eq!(usb_controller.device_status(), AudioStreamStatus::Healthy);
    assert_eq!(usb_controller.runtime_status(), AudioStreamStatus::Terminal);
    assert_eq!(usb_health.external_status(), AudioStreamStatus::Healthy);
    assert_eq!(usb_health.runtime_status(), AudioStreamStatus::Terminal);
    assert_eq!(usb_health.worker_health(), SourceWorkerHealth::WorkerExited);
}

#[test]
fn jack_reopen_drains_stale_load_status_before_passing_fresh_sender() {
    let (load_tx, load_rx) = rodio_engine_source::audio_load_status_channel();
    load_tx.try_send(load_status(0.95));
    let health = AudioStreamHealth::optional("Jack".into());
    let (initial_tx, _initial_rx) = event_queue();
    let initial = opened(initial_tx, health.clone());
    let seen_sender = Arc::new(Mutex::new(false));
    let seen_sender_for_opener = seen_sender.clone();
    let opener: OrangeRecoveryOpener = Arc::new(move |_, _sink, health, _, sender, _, _| {
        *seen_sender_for_opener.lock().unwrap() = sender.is_some();
        sender.unwrap().try_send(load_status(0.7));
        let (engine_tx, engine_rx) = event_queue();
        Ok(OpenedAudioSink {
            engine_tx: Some(engine_tx),
            _stream: None,
            health,
            _test_engine_rx: Some(Arc::new(Mutex::new(engine_rx))),
        })
    });
    let clock = clock();
    let mut controller = OrangeRecoveryController::new_initial_with_dependencies(
        AudioSink::Jack,
        true,
        initial,
        OrangeRecoveryDependencies {
            profile: OrangeAudioProfile::from_optimization(
                playback_runtime::AudioOptimization::Capacity,
            ),
            realtime_txs: Arc::new(Mutex::new(Vec::new())),
            replay_events: Arc::new(Mutex::new(ReplayCache::default())),
            attach_gate: new_attach_gate(),
            recording_tap: None,
            mirror_producer: None,
            mirror_producers: [None, None],
            opener,
            clock,
        },
    )
    .unwrap();
    health.log(cpal::StreamError::DeviceNotAvailable);
    controller.recover_if_due();
    controller.recover_if_due_with(|| while load_rx.try_recv().is_ok() {}, Some(load_tx));

    assert!(*seen_sender.lock().unwrap());
    assert_eq!(load_rx.try_recv().unwrap().worker_utilization, Some(0.7));
}

#[test]
fn optional_initial_mirror_does_not_require_an_event_sink_registration() {
    let health = AudioStreamHealth::optional("USB".into());
    let (opener, calls) = opener_with_calls();
    let pair = rodio_engine_source::new_pcm_mirror();
    let initial = OpenedAudioSink {
        engine_tx: None,
        _stream: None,
        health: health.clone(),
        _test_engine_rx: None,
    };
    let mut controller = OrangeRecoveryController::new_initial_with_dependencies(
        AudioSink::Usb,
        false,
        initial,
        OrangeRecoveryDependencies {
            profile: OrangeAudioProfile::from_optimization(
                playback_runtime::AudioOptimization::Capacity,
            ),
            realtime_txs: Arc::new(Mutex::new(Vec::new())),
            replay_events: Arc::new(Mutex::new(ReplayCache::default())),
            attach_gate: new_attach_gate(),
            recording_tap: None,
            mirror_producer: Some(pair.producer),
            mirror_producers: [None, None],
            opener,
            clock: clock(),
        },
    )
    .unwrap();

    controller.recover_if_due();

    assert_eq!(controller.device_status(), AudioStreamStatus::Healthy);
    assert_eq!(controller.runtime_status(), AudioStreamStatus::Healthy);
    assert_eq!(health.external_status(), AudioStreamStatus::Healthy);
    assert_eq!(*calls.lock().unwrap(), 0);
}

#[test]
fn required_recovery_replays_preserved_sample_bank_before_synth_owner() {
    let (initial_tx, _initial_rx) = event_queue();
    let health = AudioStreamHealth::optional("Jack".into());
    let config = prepare_instrument_slot_config(InstrumentSlotConfig {
        fm: None,
        kind: "synth".into(),
        synth: default_synth_config(),
        mixer: None,
    });
    let mut replay = ReplayCache::default();
    replay.remember(&EngineEvent::SetPreparedInstrumentOwner {
        instrument_slot: 0,
        generation: 1,
        config: config.clone(),
        sample_bank: Some(SampleBankConfig {
            gain_pct: 42.0,
            ..Default::default()
        }),
    });
    replay.remember(&EngineEvent::SetSampleBankParam {
        instrument_slot: 0,
        generation: 1,
        param: SampleBankParamId::TuneSemis,
        value: 3.0,
    });
    replay.remember(&EngineEvent::SetPreparedInstrumentOwner {
        instrument_slot: 0,
        generation: 2,
        config,
        sample_bank: None,
    });
    let recovered_receiver = Arc::new(Mutex::new(None::<Arc<Mutex<EngineEventReceiver>>>));
    let receiver_for_opener = recovered_receiver.clone();
    let opener: OrangeRecoveryOpener = Arc::new(move |_, _, health, _, _, _, _| {
        let (engine_tx, engine_rx) = event_queue();
        *receiver_for_opener.lock().unwrap() = Some(Arc::new(Mutex::new(engine_rx)));
        Ok(OpenedAudioSink {
            engine_tx: Some(engine_tx),
            _stream: None,
            health,
            _test_engine_rx: None,
        })
    });
    let now = Arc::new(Mutex::new(Instant::now()));
    let clock_now = now.clone();
    let clock: OrangeRecoveryClock = Arc::new(move || *clock_now.lock().unwrap());
    let realtime_txs = Arc::new(Mutex::new(Vec::new()));
    let initial = opened(initial_tx, health.clone());
    let mut controller = OrangeRecoveryController::new_initial_with_dependencies(
        AudioSink::Jack,
        true,
        initial,
        OrangeRecoveryDependencies {
            profile: OrangeAudioProfile::from_optimization(
                playback_runtime::AudioOptimization::Capacity,
            ),
            realtime_txs,
            replay_events: Arc::new(Mutex::new(replay)),
            attach_gate: new_attach_gate(),
            recording_tap: None,
            mirror_producer: None,
            mirror_producers: [None, None],
            opener,
            clock,
        },
    )
    .unwrap();

    health.log(cpal::StreamError::DeviceNotAvailable);
    controller.recover_if_due();
    controller.recover_if_due();
    *now.lock().unwrap() += Duration::from_millis(250);
    controller.recover_if_due();

    let receiver = recovered_receiver.lock().unwrap().take().unwrap();
    let mut receiver = receiver.lock().unwrap();
    let mut events = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        events.push(event);
    }
    let bank_index = events
        .iter()
        .position(|event| {
            matches!(
                event,
                EngineEvent::SetPreparedSampleBank {
                    generation: 1,
                    bank,
                    ..
                } if bank.gain_pct == 42.0
            )
        })
        .unwrap();
    let owner_index = events
        .iter()
        .position(|event| {
            matches!(
                event,
                EngineEvent::SetPreparedInstrumentOwner {
                    generation: 2,
                    sample_bank: None,
                    ..
                }
            )
        })
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, EngineEvent::SetPreparedSampleBank { .. }))
            .count(),
        1
    );
    assert!(bank_index < owner_index);
}

fn load_status(worker_utilization: f32) -> realtime_engine::synth::AudioLoadStatus {
    realtime_engine::synth::AudioLoadStatus {
        ratio: 0.2,
        voice_steal: false,
        worker_utilization: Some(worker_utilization),
        high_cpu_steady: worker_utilization >= 0.85,
        missed_quantum_flash: false,
        block_ratio_p95: 0.2,
        block_ratio_max: 0.2,
        blocks: 1,
        control_events: 0,
        config_events: 0,
        rendered_quantums: 0,
        repeated_quantums: 0,
        dropped_quantums: 0,
        deadline_misses: 0,
        deadline_recoveries: 0,
    }
}
