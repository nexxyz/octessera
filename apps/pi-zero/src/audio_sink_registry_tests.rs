use super::*;
use realtime_engine::synth::{prepare_audio_config, DEFAULT_AUDIO_SAMPLE_RATE};
use rodio_engine_source::{event_queue, EngineEvent, QueueKind, QueueSendError};
use std::sync::{Arc, Mutex};

#[test]
fn latest_full_is_reported_without_emergency_or_removal() {
    let mut error = None;
    let mut disconnected = Vec::new();
    let mut emergency_sent = false;

    record_send_result(
        AudioSink::Jack,
        Err(QueueSendError::Full {
            queue: QueueKind::Latest,
        }),
        || {
            emergency_sent = true;
            Ok(())
        },
        &mut error,
        &mut disconnected,
    );

    assert!(matches!(
        error,
        Some(AudioBroadcastError::Queue(QueueSendError::Full {
            queue: QueueKind::Latest
        }))
    ));
    assert!(!emergency_sent);
    assert!(disconnected.is_empty());
}

#[test]
fn mixed_latest_full_continues_to_healthy_sink_and_retains_both() {
    let mut error = None;
    let mut disconnected = Vec::new();

    record_send_result(
        AudioSink::Jack,
        Err(QueueSendError::Full {
            queue: QueueKind::Latest,
        }),
        || panic!("latest full must not publish emergency silence"),
        &mut error,
        &mut disconnected,
    );
    record_send_result(
        AudioSink::Usb,
        Ok(()),
        || panic!("healthy sink must not publish emergency silence"),
        &mut error,
        &mut disconnected,
    );

    assert!(error.is_some());
    assert!(disconnected.is_empty());
}

#[test]
fn all_latest_full_sinks_report_once_without_emergency_or_removal() {
    let mut error = None;
    let mut disconnected = Vec::new();
    let mut emergency_count = 0;

    for sink in [AudioSink::Jack, AudioSink::Usb] {
        record_send_result(
            sink,
            Err(QueueSendError::Full {
                queue: QueueKind::Latest,
            }),
            || {
                emergency_count += 1;
                Ok(())
            },
            &mut error,
            &mut disconnected,
        );
    }

    assert!(error.is_some());
    assert_eq!(emergency_count, 0);
    assert!(disconnected.is_empty());
}

#[test]
fn later_latest_retry_is_admitted_after_a_full_result() {
    let mut first_error = None;
    let mut disconnected = Vec::new();
    record_send_result(
        AudioSink::Jack,
        Err(QueueSendError::Full {
            queue: QueueKind::Latest,
        }),
        || panic!("latest full must not publish emergency silence"),
        &mut first_error,
        &mut disconnected,
    );
    assert!(first_error.is_some());

    let mut retry_error = None;
    record_send_result(
        AudioSink::Jack,
        Ok(()),
        || panic!("successful retry must not publish emergency silence"),
        &mut retry_error,
        &mut disconnected,
    );
    assert!(retry_error.is_none());
    assert!(disconnected.is_empty());
}

#[test]
fn musical_full_publishes_emergency_silence_and_retains_sink() {
    let (tx, mut rx) = event_queue();
    let txs = Arc::new(Mutex::new(Vec::new()));
    register_sink(&txs, AudioSink::Jack, tx);
    let config = prepare_audio_config(
        crate::audio::default_pi_instruments(),
        None,
        None,
        DEFAULT_AUDIO_SAMPLE_RATE,
    );
    for generation in 0..64 {
        txs.lock()
            .unwrap()
            .first()
            .unwrap()
            .tx
            .send(EngineEvent::SetPreparedAudioConfig {
                generation: generation as u64,
                config: config.clone(),
            })
            .unwrap();
    }

    let result = broadcast_event(
        &txs,
        EngineEvent::SetPreparedAudioConfig {
            generation: 99,
            config,
        },
    );

    assert!(matches!(
        result,
        Err(AudioBroadcastError::Queue(QueueSendError::Full {
            queue: QueueKind::Structural
        }))
    ));
    assert!(matches!(rx.try_recv().unwrap(), EngineEvent::AllNotesOff));
    assert_eq!(txs.lock().unwrap().len(), 1);
}

#[test]
fn disconnected_sink_is_removed_and_preserves_queue_kind() {
    let (tx, rx) = event_queue();
    let txs = Arc::new(Mutex::new(Vec::new()));
    register_sink(&txs, AudioSink::Jack, tx);
    drop(rx);

    let result = broadcast_event(
        &txs,
        EngineEvent::SetMasterVolume {
            generation: 1,
            volume_pct: 70.0,
        },
    );

    assert!(matches!(
        result,
        Err(AudioBroadcastError::Queue(QueueSendError::Disconnected {
            queue: QueueKind::Latest
        }))
    ));
    assert!(txs.lock().unwrap().is_empty());
}
