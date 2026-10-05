use super::*;
use realtime_engine::synth::{prepare_audio_config, DEFAULT_AUDIO_SAMPLE_RATE};
use rodio_engine_source::{event_queue, EngineEvent, QueueKind, QueueSendError};

fn master_volume(generation: u64, volume_pct: f32) -> EngineEvent {
    EngineEvent::SetMasterVolume {
        generation,
        volume_pct,
    }
}

#[test]
fn latest_full_is_reported_without_emergency_or_detach() {
    let (result, disconnected) = send_result(
        Err(QueueSendError::Full {
            queue: QueueKind::Latest,
        }),
        || panic!("latest full must not publish emergency silence"),
    );

    assert!(matches!(
        result,
        Err(AudioSendError::Queue(QueueSendError::Full {
            queue: QueueKind::Latest
        }))
    ));
    assert!(!disconnected);
}

#[test]
fn musical_full_publishes_emergency_silence_and_keeps_owner() {
    let (tx, mut rx) = event_queue();
    let config = prepare_audio_config(
        crate::audio::default_pi_instruments(),
        None,
        None,
        DEFAULT_AUDIO_SAMPLE_RATE,
    );
    for generation in 0..64 {
        tx.send(EngineEvent::SetPreparedAudioConfig {
            generation,
            config: config.clone(),
        })
        .unwrap();
    }
    let owner = AudioEngineOwner::attached(tx);

    let result = owner.send(EngineEvent::SetPreparedAudioConfig {
        generation: 99,
        config,
    });

    assert!(matches!(
        result,
        Err(AudioSendError::Queue(QueueSendError::Full {
            queue: QueueKind::Structural
        }))
    ));
    assert!(matches!(rx.try_recv().unwrap(), EngineEvent::AllNotesOff));
    assert!(owner.is_attached());
}

#[test]
fn disconnected_engine_is_detached_and_preserves_queue_kind() {
    let (tx, rx) = event_queue();
    let owner = AudioEngineOwner::attached(tx);
    drop(rx);

    let result = owner.send(master_volume(1, 70.0));

    assert!(matches!(
        result,
        Err(AudioSendError::Queue(QueueSendError::Disconnected {
            queue: QueueKind::Latest
        }))
    ));
    assert!(!owner.is_attached());
}

#[test]
fn sends_without_an_engine_are_remembered_and_replayed_on_attach() {
    let owner = AudioEngineOwner::new(ReplayCache::default());
    owner.send(master_volume(1, 64.0)).unwrap();
    assert!(!owner.is_attached());
    assert!(owner
        .replay_events()
        .iter()
        .any(|event| matches!(event, EngineEvent::SetMasterVolume { generation: 1, .. })));

    let (tx, mut rx) = event_queue();
    owner.attach(tx).unwrap();

    assert!(owner.is_attached());
    assert!(matches!(
        rx.try_recv().unwrap(),
        EngineEvent::SetPreparedInstruments { .. }
    ));
}
