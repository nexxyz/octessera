use super::audio_output_open::OpenedAudioSink;
use super::orange_audio_recovery::{
    OrangeRecoveryClock, OrangeRecoveryController, OrangeRecoveryDependencies, OrangeRecoveryOpener,
};
use super::AudioSink;
use crate::audio_replay::ReplayCache;
use crate::audio_route::RouteOpenError;
use crate::audio_sink_registry::{has_sink, new_attach_gate, register_sink};
use crate::audio_stream_health::{AudioStreamHealth, AudioStreamStatus};
use realtime_engine::synth::SourceWorkerHealth;
use rodio_engine_source::event_queue;
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn opener_with_calls() -> (OrangeRecoveryOpener, Arc<Mutex<usize>>) {
    let calls = Arc::new(Mutex::new(0));
    let call_count = calls.clone();
    let opener: OrangeRecoveryOpener = Arc::new(move |_, _, _, _| {
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
        engine_tx,
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
            output_buffer_frames: None,
            realtime_txs: jack_sinks.clone(),
            replay_events: Arc::new(Mutex::new(ReplayCache::default())),
            attach_gate: new_attach_gate(),
            recording_tap: None,
            opener: jack_opener,
            clock: clock(),
        },
    )
    .unwrap();

    jack_health.mark_worker_health(SourceWorkerHealth::DeadlineMiss);
    jack_controller.recover_if_due();

    assert_eq!(*jack_calls.lock().unwrap(), 0);
    assert_eq!(jack_controller.device_status(), AudioStreamStatus::Healthy);
    assert_eq!(
        jack_controller.runtime_status(),
        AudioStreamStatus::Terminal
    );
    assert_eq!(jack_health.external_status(), AudioStreamStatus::Healthy);
    assert_eq!(jack_health.runtime_status(), AudioStreamStatus::Terminal);
    assert_eq!(
        jack_health.worker_health(),
        SourceWorkerHealth::DeadlineMiss
    );
    assert!(has_sink(&jack_sinks, AudioSink::Jack));

    let (usb_tx, _usb_rx) = event_queue();
    let usb_sinks = Arc::new(Mutex::new(Vec::new()));
    register_sink(&usb_sinks, AudioSink::Usb, usb_tx.clone());
    let usb_health = AudioStreamHealth::optional("USB".into());
    let (usb_opener, usb_calls) = opener_with_calls();
    let mut usb_controller = OrangeRecoveryController::new_initial_with_dependencies(
        AudioSink::Usb,
        false,
        opened(usb_tx, usb_health.clone()),
        OrangeRecoveryDependencies {
            output_buffer_frames: None,
            realtime_txs: usb_sinks.clone(),
            replay_events: Arc::new(Mutex::new(ReplayCache::default())),
            attach_gate: new_attach_gate(),
            recording_tap: None,
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
    assert!(has_sink(&usb_sinks, AudioSink::Usb));
}
