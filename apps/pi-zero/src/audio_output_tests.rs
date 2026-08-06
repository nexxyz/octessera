use super::cpal_audio_output::resolve_output_buffer_frames;

#[cfg(feature = "hardware-orange-pi-zero-2w")]
use crate::audio_hotplug::{has_sink, register_sink, ReplayCache};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use rodio_engine_source::{event_queue, EngineEvent, EngineEventReceiver};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "hardware-orange-pi-zero-2w")]
use std::time::{Duration, Instant};

#[test]
fn raspberry_direct_cpal_default_buffer_remains_256_frames() {
    assert_eq!(resolve_output_buffer_frames(None, None, 256), 256);
    assert_eq!(resolve_output_buffer_frames(None, Some(512), 256), 512);
}

#[test]
fn output_buffer_override_is_parsed_and_clamped() {
    assert_eq!(resolve_output_buffer_frames(Some("1024"), None, 256), 1024);
    assert_eq!(
        resolve_output_buffer_frames(Some("invalid"), Some(512), 256),
        512
    );
    assert_eq!(resolve_output_buffer_frames(Some("1"), None, 256), 32);
    assert_eq!(resolve_output_buffer_frames(Some("4096"), None, 256), 2048);
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[test]
fn raspberry_scheduler_labels_preserve_sink_identity() {
    assert_eq!(super::AudioSink::Jack.scheduler_label(), "Jack");
    assert_eq!(super::AudioSink::Usb.scheduler_label(), "USB");
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_direct_cpal_default_buffer_is_256_frames() {
    assert_eq!(
        resolve_output_buffer_frames(
            None,
            None,
            super::cpal_audio_output::ORANGE_DEFAULT_OUTPUT_BUFFER_FRAMES,
        ),
        256
    );
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_explicit_buffer_overrides_remain_available() {
    assert_eq!(
        resolve_output_buffer_frames(
            None,
            Some(512),
            super::cpal_audio_output::ORANGE_DEFAULT_OUTPUT_BUFFER_FRAMES,
        ),
        512
    );
    assert_eq!(
        resolve_output_buffer_frames(
            Some("1024"),
            None,
            super::cpal_audio_output::ORANGE_DEFAULT_OUTPUT_BUFFER_FRAMES,
        ),
        1024
    );
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_buffer_qualification_stages_remain_explicit() {
    assert_eq!(
        super::cpal_audio_output::ORANGE_BUFFER_QUALIFICATION_STAGES,
        &[1024, 512, 256]
    );
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_engine_blocks_follow_one_quarter_of_output_buffer() {
    for (output_buffer, engine_block) in [(256, 64), (512, 128), (1024, 256)] {
        assert_eq!(
            super::cpal_audio_output::orange_engine_block_frames(output_buffer),
            engine_block
        );
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_audio_out_policy_keeps_dac_required() {
    use crate::usb_config::UsbAudioOut;

    assert_eq!(
        super::orange_audio_sinks(UsbAudioOut::Jack),
        Ok(vec![super::AudioSink::InternalDac])
    );
    assert_eq!(
        super::orange_audio_sinks(UsbAudioOut::Both),
        Ok(vec![super::AudioSink::InternalDac, super::AudioSink::Usb])
    );
    assert_eq!(
        super::orange_audio_sinks(UsbAudioOut::Usb),
        Err(super::OrangeAudioInitError::UsbUnavailable)
    );
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_scheduler_labels_identify_dac_and_uac2() {
    assert_eq!(super::AudioSink::InternalDac.scheduler_label(), "DAC");
    assert_eq!(super::AudioSink::Usb.scheduler_label(), "UAC2");
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_recovery_stops_after_maximum_recoverable_failures() {
    let health = super::AudioStreamHealth::new("InternalDac".into());
    let mut attempts = Vec::new();

    let decision = super::orange_audio_recovery::run_bounded_orange_recovery(&health, |attempt| {
        attempts.push(attempt);
        super::orange_audio_recovery::OrangeRecoveryAttempt::RecoverableFailure
    });

    assert_eq!(
        decision,
        super::orange_audio_recovery::OrangeRecoveryDecision::Terminal
    );
    assert_eq!(attempts, vec![1, 2, 3]);
    assert_eq!(
        attempts.len(),
        super::orange_audio_recovery::ORANGE_RECOVERY_MAX_ATTEMPTS
    );
    assert!(health.is_terminal());
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_recovery_accepts_a_stable_attempt_before_the_limit() {
    let health = super::AudioStreamHealth::new("InternalDac".into());
    let mut outcomes = [
        super::orange_audio_recovery::OrangeRecoveryAttempt::RecoverableFailure,
        super::orange_audio_recovery::OrangeRecoveryAttempt::Stable,
    ]
    .into_iter();

    let decision = super::orange_audio_recovery::run_bounded_orange_recovery(&health, |_| {
        outcomes.next().unwrap()
    });

    assert_eq!(
        decision,
        super::orange_audio_recovery::OrangeRecoveryDecision::Recovered
    );
    assert!(!health.is_terminal());
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_optional_gadget_recovery_retries_after_absence_and_can_recover() {
    let health = super::AudioStreamHealth::optional("UAC2Gadget".into());
    let mut attempts = Vec::new();

    let decision =
        super::orange_audio_recovery::run_bounded_optional_recovery(&health, |attempt| {
            attempts.push(attempt);
            super::orange_audio_recovery::OrangeRecoveryAttempt::RecoverableFailure
        });

    assert_eq!(
        decision,
        super::orange_audio_recovery::OrangeRecoveryDecision::Retrying
    );
    assert_eq!(
        attempts,
        vec![1, 2, 3],
        "gadget absence is bounded per recovery cycle, then remains retryable"
    );
    let decision = super::orange_audio_recovery::run_bounded_optional_recovery(&health, |_| {
        super::orange_audio_recovery::OrangeRecoveryAttempt::Stable
    });
    assert_eq!(
        decision,
        super::orange_audio_recovery::OrangeRecoveryDecision::Recovered
    );
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_controller_reopens_optional_uac2_once_and_keeps_dac_registered() {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let replay_receiver = Arc::new(Mutex::new(None::<EngineEventReceiver>));
    let opener: super::orange_audio_recovery::OrangeRecoveryOpener = {
        let attempts = attempts.clone();
        let replay_receiver = replay_receiver.clone();
        Arc::new(move |_, sink, health| {
            attempts.lock().unwrap().push(sink);
            if attempts.lock().unwrap().len() < 4 {
                return Err("UAC2Gadget is absent".into());
            }
            let (tx, rx) = event_queue();
            *replay_receiver.lock().unwrap() = Some(rx);
            Ok(super::OpenedAudioSink {
                engine_tx: tx,
                _stream: None,
                health,
            })
        })
    };
    let now = Arc::new(Mutex::new(Instant::now()));
    let clock_now = now.clone();
    let clock: super::orange_audio_recovery::OrangeRecoveryClock =
        Arc::new(move || *clock_now.lock().unwrap());
    let (dac_tx, _dac_rx) = event_queue();
    let sinks = Arc::new(Mutex::new(Vec::new()));
    register_sink(&sinks, super::AudioSink::InternalDac, dac_tx);
    let mut replay = ReplayCache::default();
    replay.remember(&EngineEvent::SetMasterVolume { volume_pct: 72.0 });
    let replay_events = Arc::new(Mutex::new(replay));
    let mut controller = super::orange_audio_recovery::OrangeRecoveryController::
        new_optional_missing_with_dependencies(
            None,
            sinks.clone(),
            replay_events,
            opener,
            clock,
        );

    for _ in 0..3 {
        controller.recover_if_due();
        *now.lock().unwrap() += Duration::from_millis(50);
    }
    *now.lock().unwrap() += Duration::from_secs(2);
    controller.recover_if_due();
    *now.lock().unwrap() += Duration::from_millis(250);
    controller.recover_if_due();

    assert_eq!(
        attempts.lock().unwrap().as_slice(),
        &[super::AudioSink::Usb; 4]
    );
    let registered = sinks.lock().unwrap();
    assert!(registered
        .iter()
        .any(|entry| entry.sink == super::AudioSink::InternalDac));
    assert!(registered
        .iter()
        .any(|entry| entry.sink == super::AudioSink::Usb));
    drop(registered);
    let mut receiver = replay_receiver.lock().unwrap().take().unwrap();
    assert!(matches!(
        receiver.try_recv(),
        Ok(EngineEvent::SetPreparedInstruments(_))
    ));
    assert!(matches!(
        receiver.try_recv(),
        Ok(EngineEvent::SetMasterVolume { volume_pct }) if volume_pct == 72.0
    ));
    assert!(receiver.try_recv().is_err());
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_device_loss_is_terminal_without_recovery_retry() {
    let health = super::AudioStreamHealth::new("InternalDac".into());
    let mut attempts = 0;

    let decision = super::orange_audio_recovery::run_bounded_orange_recovery(&health, |_| {
        attempts += 1;
        super::orange_audio_recovery::OrangeRecoveryAttempt::DeviceNotAvailable
    });

    assert_eq!(
        decision,
        super::orange_audio_recovery::OrangeRecoveryDecision::Terminal
    );
    assert_eq!(attempts, 1);
    assert!(health.is_terminal());
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn orange_required_controller_detaches_after_device_loss_without_opening_hardware() {
    let (tx, _rx) = event_queue();
    let sinks = Arc::new(Mutex::new(Vec::new()));
    register_sink(&sinks, super::AudioSink::InternalDac, tx.clone());
    let health = super::AudioStreamHealth::new("InternalDac".into());
    let initial = super::OpenedAudioSink {
        engine_tx: tx,
        _stream: None,
        health: health.clone(),
    };
    let replay_events = Arc::new(Mutex::new(ReplayCache::default()));
    let mut controller = super::orange_audio_recovery::OrangeRecoveryController::new_required(
        initial,
        None,
        sinks.clone(),
        replay_events,
    );

    health.mark_terminal();
    controller.recover_if_due();

    assert_eq!(controller.status(), super::OrangeDacStatus::Terminal);
    assert!(!has_sink(&sinks, super::AudioSink::InternalDac));
}
